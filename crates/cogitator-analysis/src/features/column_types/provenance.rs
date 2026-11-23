//! Where each output column's type comes from, read off a lowered MIR.
//!
//! Pure over the plan: every output `ColId` is followed to the node that
//! defines it (a select item, an aggregate, a window call, a scanned
//! column, a set operation's arms, a CTE's output) and classified. The
//! upstream columns whose types feed the result are listed, so a caller
//! can decide how much those upstream types are worth.

use std::collections::{BTreeSet, HashMap};

use cogitator_ir::mir::{ColId, Mir, RelOp, ScalarId, ScalarKind, TableId, TableRef};
use cogitator_ir::{ColType, SqlType, Symbol};
use serde::Serialize;

/// Bounds the walk through pass-through columns and nested expressions.
const MAX_DEPTH: usize = 64;

/// A relation an output column reads its type from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Relation {
    Model {
        name: String,
    },
    Source {
        source: String,
        table: String,
    },
    /// `{{ this }}`: the model's own previous build.
    This,
    /// A hard-coded table or a relation built by Jinja.
    Other,
}

/// One upstream column a type was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct UpstreamColumn {
    pub relation: Relation,
    pub column: String,
}

/// What produces an output column, as far as its type is concerned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Provenance {
    /// `cast(x as t)` / `x::t`: the type is the cast's.
    Cast,
    Literal,
    /// A scalar function call.
    Function {
        name: String,
    },
    Aggregate {
        name: String,
    },
    Window {
        name: String,
    },
    /// A column read from another relation, passed through unchanged.
    Upstream {
        from: UpstreamColumn,
    },
    /// The arms of a `union` / `intersect` / `except`, widened.
    SetOp,
    /// An operator, `CASE` or another composite expression.
    Expression,
    /// Opaque SQL, a subquery or a Jinja hole: nothing is claimed.
    Unknown,
}

/// One output column with its inferred type and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawColumn {
    pub name: String,
    pub ty: ColType,
    pub provenance: Provenance,
    /// Upstream columns whose types the result depends on (sorted). Empty
    /// for casts, literals and `count`: their type stands on its own.
    pub feeds: Vec<UpstreamColumn>,
}

#[derive(Debug, Clone)]
enum Def {
    Scalar(ScalarId),
    Agg {
        func: Symbol,
        args: Vec<ScalarId>,
    },
    Win {
        func: Symbol,
        args: Vec<ScalarId>,
    },
    Scan {
        table: TableId,
        name: Option<Symbol>,
    },
    Arms(Vec<ColId>),
    Alias(ColId),
}

/// The root's named output columns, first occurrence of a name only.
pub fn raw_columns(mir: &Mir) -> Vec<RawColumn> {
    let defs = definitions(mir);
    let walker = Walker { mir, defs: &defs };
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for &col in mir.root_columns() {
        let Some(def) = mir.cols.get(col) else {
            continue;
        };
        let Some(name) = def.name else {
            continue;
        };
        let name = mir.syms.text(name);
        if !seen.insert(name.to_ascii_lowercase()) {
            continue;
        }
        let mut feeds = BTreeSet::new();
        let provenance = walker.col(col, 0, &mut feeds);
        out.push(RawColumn {
            name: name.to_string(), // boundary: the snapshot owns its names
            ty: def.ty.clone(),
            provenance,
            feeds: feeds.into_iter().collect(),
        });
    }
    out
}

/// Every column some relation defines, mapped to its definition.
fn definitions(mir: &Mir) -> HashMap<ColId, Def> {
    let mut defs = HashMap::new();
    for (_, rel) in mir.rels.iter() {
        match &rel.op {
            RelOp::Scan { table, cols, .. } => {
                for &c in cols {
                    let name = mir.cols.get(c).and_then(|d| d.name);
                    defs.insert(
                        c,
                        Def::Scan {
                            table: *table,
                            name,
                        },
                    );
                }
            }
            RelOp::Project { items, .. } => {
                for &(c, s) in items {
                    if !is_passthrough(mir, c, s) {
                        defs.insert(c, Def::Scalar(s));
                    }
                }
            }
            RelOp::Aggregate { keys, aggs, .. } => {
                for &(c, s) in keys {
                    if !is_passthrough(mir, c, s) {
                        defs.insert(c, Def::Scalar(s));
                    }
                }
                for (c, agg) in aggs {
                    let (func, args) = (agg.func, agg.args.clone());
                    defs.insert(*c, Def::Agg { func, args });
                }
            }
            RelOp::Window { funcs, .. } => {
                for (c, win) in funcs {
                    let (func, args) = (win.func, win.args.clone());
                    defs.insert(*c, Def::Win { func, args });
                }
            }
            RelOp::SetOp { inputs, out, .. } => {
                for (i, &c) in out.iter().enumerate() {
                    let arms = inputs
                        .iter()
                        .filter_map(|r| mir.rels.get(*r)?.out.get(i).copied())
                        .collect();
                    defs.insert(c, Def::Arms(arms));
                }
            }
            RelOp::CteRef { cte, out } => {
                let Some(body) = mir.ctes.get(*cte) else {
                    continue;
                };
                for (&c, &inner) in out.iter().zip(&body.out) {
                    if c != inner {
                        defs.insert(c, Def::Alias(inner));
                    }
                }
            }
            _ => {}
        }
    }
    defs
}

fn is_passthrough(mir: &Mir, c: ColId, s: ScalarId) -> bool {
    matches!(mir.scalars.get(s).map(|s| &s.kind), Some(ScalarKind::Col(x)) if *x == c)
}

struct Walker<'a> {
    mir: &'a Mir,
    defs: &'a HashMap<ColId, Def>,
}

impl Walker<'_> {
    fn col(&self, col: ColId, depth: usize, feeds: &mut BTreeSet<UpstreamColumn>) -> Provenance {
        if depth > MAX_DEPTH {
            return Provenance::Unknown;
        }
        match self.defs.get(&col) {
            Some(Def::Scalar(s)) => self.scalar(*s, depth + 1, feeds),
            Some(Def::Alias(inner)) => self.col(*inner, depth + 1, feeds),
            Some(Def::Agg { func, args }) => {
                let name = self.mir.syms.text(*func).to_ascii_lowercase();
                if !matches!(name.as_str(), "count" | "count_if" | "countif") {
                    self.feed_all(args, depth, feeds);
                }
                Provenance::Aggregate { name }
            }
            Some(Def::Win { func, args }) => {
                let name = self.mir.syms.text(*func).to_ascii_lowercase();
                self.feed_all(args, depth, feeds);
                Provenance::Window { name }
            }
            Some(Def::Scan { table, name }) => {
                let from = UpstreamColumn {
                    relation: self.relation(*table),
                    column: name
                        .map(|n| self.mir.syms.text(n).to_string())
                        .unwrap_or_default(),
                };
                feeds.insert(from.clone());
                Provenance::Upstream { from }
            }
            Some(Def::Arms(arms)) => {
                for &arm in arms {
                    self.col(arm, depth + 1, feeds);
                }
                Provenance::SetOp
            }
            None => Provenance::Unknown,
        }
    }

    fn scalar(
        &self,
        id: ScalarId,
        depth: usize,
        feeds: &mut BTreeSet<UpstreamColumn>,
    ) -> Provenance {
        let Some(scalar) = self.mir.scalars.get(id) else {
            return Provenance::Unknown;
        };
        if depth > MAX_DEPTH {
            return Provenance::Unknown;
        }
        match &scalar.kind {
            ScalarKind::Col(c) => self.col(*c, depth + 1, feeds),
            ScalarKind::Cast { .. } => Provenance::Cast,
            ScalarKind::Lit(_) => Provenance::Literal,
            ScalarKind::Call { name, args, .. } => {
                self.feed_all(args, depth, feeds);
                Provenance::Function {
                    name: self.mir.syms.text(*name).to_ascii_lowercase(),
                }
            }
            ScalarKind::Unary { .. }
            | ScalarKind::Binary { .. }
            | ScalarKind::Case { .. }
            | ScalarKind::InList { .. } => {
                self.feed_all(&scalar.kind.children(), depth, feeds);
                Provenance::Expression
            }
            ScalarKind::Outer(_) | ScalarKind::Subquery { .. } | ScalarKind::Opaque { .. } => {
                Provenance::Unknown
            }
        }
    }

    /// Records the upstream columns under `args`; the classification of the
    /// arguments themselves is not needed.
    fn feed_all(&self, args: &[ScalarId], depth: usize, feeds: &mut BTreeSet<UpstreamColumn>) {
        for &arg in args {
            self.scalar(arg, depth + 1, feeds);
        }
    }

    fn relation(&self, table: TableId) -> Relation {
        let text = |s: Symbol| self.mir.syms.text(s).to_string();
        match self.mir.tables.get(table).map(|t| &t.target) {
            Some(TableRef::Model { name, .. }) => Relation::Model { name: text(*name) },
            Some(TableRef::Source { source, table }) => Relation::Source {
                source: text(*source),
                table: text(*table),
            },
            Some(TableRef::This) => Relation::This,
            _ => Relation::Other,
        }
    }
}

/// True when the type says something: neither `Unknown` nor bare `Null`.
pub fn is_stated(ty: &SqlType) -> bool {
    !matches!(ty, SqlType::Unknown | SqlType::Null)
}
