//! The MIR as an S-expression (specs/ir-pipeline/10 §6): relations by
//! their operator, columns and tables by name, CTE references with their
//! body inline. `annotate` lets an analysis append its own items to each
//! relation's list — `(class linear)` — without a printer of its own.

use crate::common::{BinaryOp, JoinKind, Lit, SetOpKind, UnaryOp};
use crate::mir::{ColId, Mir, RelId, RelOp, ScalarId, ScalarKind, SortKey, SubqueryKind, TableRef};
use crate::print_util::MAX_PRINT_DEPTH;
use crate::sexp::{atom, list, string, Sexp};

/// The plan from its root, unannotated.
pub fn mir_sexp(mir: &Mir) -> Sexp {
    rel_sexp(mir, mir.root, &|_| Vec::new())
}

/// The plan below `id`, each relation followed by what `annotate` says.
pub fn rel_sexp(mir: &Mir, id: RelId, annotate: &dyn Fn(RelId) -> Vec<Sexp>) -> Sexp {
    rel(mir, id, annotate, 0)
}

fn rel(mir: &Mir, id: RelId, annotate: &dyn Fn(RelId) -> Vec<Sexp>, depth: usize) -> Sexp {
    if depth > MAX_PRINT_DEPTH {
        return atom("...");
    }
    let Some(node) = mir.rels.get(id) else {
        return atom("<bad-rel>");
    };
    let sub = |r: RelId| rel(mir, r, annotate, depth + 1);
    let s = |x: ScalarId| scalar(mir, x, annotate, depth + 1);
    let mut items: Vec<Sexp> = match &node.op {
        RelOp::Scan { table, open, .. } => {
            let mut items = vec![atom("scan"), atom(table_name(mir, *table))];
            if *open {
                items.push(atom("open"));
            }
            items
        }
        RelOp::Filter { input, conjuncts } => {
            vec![
                atom("filter"),
                conjunction(conjuncts.iter().map(|c| s(*c)).collect()),
                sub(*input),
            ]
        }
        RelOp::Project { input, items } => vec![
            atom("project"),
            list("items", items.iter().map(|(c, e)| named(mir, *c, *e, &s))),
            sub(*input),
        ],
        RelOp::Join { kind, l, r, on } => vec![
            atom("join"),
            atom(join_kind(*kind)),
            list("on", [conjunction(on.iter().map(|c| s(*c)).collect())]),
            sub(*l),
            sub(*r),
        ],
        RelOp::Aggregate {
            input,
            keys,
            aggs,
            sets,
        } => {
            let mut items = vec![
                atom("aggregate"),
                list("keys", keys.iter().map(|(c, e)| named(mir, *c, *e, &s))),
                list(
                    "aggs",
                    aggs.iter().map(|(c, call)| {
                        let mut parts = vec![atom(mir.syms.text(call.func))];
                        if call.distinct {
                            parts.push(atom("distinct"));
                        }
                        parts.extend(call.args.iter().map(|a| s(*a)));
                        parts.extend(name_of(mir, *c).map(atom));
                        Sexp::List(parts)
                    }),
                ),
            ];
            if sets.is_some() {
                items.push(atom("grouping-sets"));
            }
            items.push(sub(*input));
            items
        }
        RelOp::Window { input, funcs } => {
            let mut items = vec![atom("window")];
            for (c, call) in funcs {
                items.push(list(
                    "fn",
                    [
                        atom(mir.syms.text(call.func)),
                        list("partition", call.partition_by.iter().map(|p| s(*p))),
                        list("order", call.order_by.iter().map(|k| sort_key(k, &s))),
                    ]
                    .into_iter()
                    .chain(name_of(mir, *c).map(atom)),
                ));
            }
            items.push(sub(*input));
            items
        }
        RelOp::Distinct(input) => vec![atom("distinct"), sub(*input)],
        RelOp::SetOp {
            op, all, inputs, ..
        } => {
            let head = match (op, all) {
                (SetOpKind::Union, true) => "union-all",
                (SetOpKind::Union, false) => "union",
                (SetOpKind::Intersect, _) => "intersect",
                (SetOpKind::Except, _) => "except",
            };
            std::iter::once(atom(head))
                .chain(inputs.iter().map(|i| sub(*i)))
                .collect()
        }
        RelOp::Sort { input, keys } => {
            let mut items = vec![atom("sort")];
            items.extend(keys.iter().map(|k| sort_key(k, &s)));
            items.push(sub(*input));
            items
        }
        RelOp::Limit {
            input,
            limit,
            offset,
        } => {
            let mut items = vec![atom("limit"), limit.map_or(atom("all"), s)];
            if let Some(o) = offset {
                items.push(list("offset", [s(*o)]));
            }
            items.push(sub(*input));
            items
        }
        RelOp::Unnest { input, exprs, .. } => {
            let mut items = vec![atom("unnest")];
            items.extend(exprs.iter().map(|e| s(*e)));
            items.push(sub(*input));
            items
        }
        RelOp::CteRef { cte, .. } => {
            let def = mir.ctes.get(*cte);
            let name = def.and_then(|d| d.name).map_or("?", |n| mir.syms.text(n));
            let mut items = vec![atom("cte-ref"), atom(name)];
            if let Some(def) = def {
                items.push(sub(def.body));
            }
            items
        }
        RelOp::Opaque { .. } => vec![atom("opaque")],
    };
    items.extend(annotate(id));
    Sexp::List(items)
}

/// One conjunct as itself, several as `(and ...)`.
fn conjunction(mut items: Vec<Sexp>) -> Sexp {
    match items.len() {
        1 => items.pop().unwrap_or(atom("true")),
        0 => atom("true"),
        _ => list("and", items),
    }
}

/// A passed-through column as its name; anything else as `(name expr)`.
fn named(mir: &Mir, col: ColId, expr: ScalarId, s: &dyn Fn(ScalarId) -> Sexp) -> Sexp {
    let value = s(expr);
    let Some(name) = name_of(mir, col) else {
        return value;
    };
    match &value {
        // the column itself, or an internal one the name now gives a name
        Sexp::Atom(a) if *a == name || a == UNNAMED => atom(name),
        _ => Sexp::List(vec![atom(name), value]),
    }
}

/// How an internal column without a name prints, where it has to.
const UNNAMED: &str = "_";

fn name_of(mir: &Mir, id: ColId) -> Option<String> {
    mir.cols
        .get(id)
        .and_then(|d| d.name)
        .map(|n| mir.syms.text(n).to_string())
}

fn sort_key(key: &SortKey, s: &dyn Fn(ScalarId) -> Sexp) -> Sexp {
    let dir = if key.desc == Some(true) {
        "desc"
    } else {
        "asc"
    };
    list(dir, [s(key.expr)])
}

fn col_name(mir: &Mir, id: ColId) -> String {
    name_of(mir, id).unwrap_or_else(|| UNNAMED.to_string())
}

fn table_name(mir: &Mir, id: crate::mir::TableId) -> String {
    let Some(table) = mir.tables.get(id) else {
        return "<bad-table>".into();
    };
    match &table.target {
        TableRef::Model { package, name, .. } => match package {
            Some(p) => format!("{}.{}", mir.syms.text(*p), mir.syms.text(*name)),
            None => mir.syms.text(*name).to_string(),
        },
        TableRef::Source { source, table } => {
            format!("{}.{}", mir.syms.text(*source), mir.syms.text(*table))
        }
        TableRef::This => "this".into(),
        TableRef::Physical(path) => path
            .iter()
            .map(|p| mir.syms.text(*p))
            .collect::<Vec<_>>()
            .join("."),
        TableRef::Jinja => "jinja".into(),
    }
}

fn join_kind(kind: JoinKind) -> &'static str {
    match kind {
        JoinKind::Inner => "inner",
        JoinKind::Left => "left",
        JoinKind::Right => "right",
        JoinKind::Full => "full",
        JoinKind::Cross => "cross",
        JoinKind::Semi => "semi",
        JoinKind::Anti => "anti",
    }
}

fn scalar(mir: &Mir, id: ScalarId, annotate: &dyn Fn(RelId) -> Vec<Sexp>, depth: usize) -> Sexp {
    if depth > MAX_PRINT_DEPTH {
        return atom("...");
    }
    let Some(x) = mir.scalars.get(id) else {
        return atom("<bad>");
    };
    let s = |e: ScalarId| scalar(mir, e, annotate, depth + 1);
    match &x.kind {
        ScalarKind::Col(c) => atom(col_name(mir, *c)),
        ScalarKind::Outer(c) => list("outer", [atom(col_name(mir, *c))]),
        ScalarKind::Lit(l) => literal(mir, l),
        ScalarKind::Unary { op, e } => {
            let head = match op {
                UnaryOp::Not => "not",
                UnaryOp::Neg => "neg",
                UnaryOp::IsNull => "is-null",
                UnaryOp::IsNotNull => "is-not-null",
                UnaryOp::IsTrue => "is-true",
                UnaryOp::IsFalse => "is-false",
                UnaryOp::Other(name) => mir.syms.text(*name),
            };
            list(head, [s(*e)])
        }
        ScalarKind::Binary { op, l, r } => list(&binary(mir, op), [s(*l), s(*r)]),
        ScalarKind::Call {
            name,
            args,
            distinct,
        } => {
            let mut items = vec![atom(mir.syms.text(*name))];
            if *distinct {
                items.push(atom("distinct"));
            }
            items.extend(args.iter().map(|a| s(*a)));
            Sexp::List(items)
        }
        ScalarKind::Case {
            operand,
            arms,
            else_,
        } => {
            let mut items = vec![atom("case")];
            items.extend(operand.iter().map(|o| s(*o)));
            items.extend(arms.iter().map(|(w, t)| list("when", [s(*w), s(*t)])));
            items.extend(else_.iter().map(|e| list("else", [s(*e)])));
            Sexp::List(items)
        }
        ScalarKind::Cast { e, ty } => list("cast", [s(*e), atom(format!("{ty:?}").to_lowercase())]),
        ScalarKind::InList {
            e,
            list: xs,
            negated,
        } => {
            let head = if *negated { "not-in" } else { "in" };
            list(head, std::iter::once(s(*e)).chain(xs.iter().map(|x| s(*x))))
        }
        ScalarKind::Subquery { kind, rel: r, .. } => {
            let body = rel(mir, *r, annotate, depth + 1);
            match kind {
                SubqueryKind::Exists => list("exists", [body]),
                SubqueryKind::Scalar => list("scalar", [body]),
                SubqueryKind::In { e, negated } => {
                    list(if *negated { "not-in" } else { "in" }, [s(*e), body])
                }
            }
        }
        ScalarKind::Opaque { .. } => atom("opaque"),
    }
}

fn binary(mir: &Mir, op: &BinaryOp) -> String {
    match op {
        BinaryOp::Eq => "=",
        BinaryOp::NotEq => "<>",
        BinaryOp::Lt => "<",
        BinaryOp::LtEq => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::GtEq => ">=",
        BinaryOp::And => "and",
        BinaryOp::Or => "or",
        BinaryOp::Plus => "+",
        BinaryOp::Minus => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::Concat => "||",
        BinaryOp::Like => "like",
        BinaryOp::NotLike => "not-like",
        BinaryOp::ILike => "ilike",
        BinaryOp::NotILike => "not-ilike",
        BinaryOp::Other(name) => return mir.syms.text(*name).to_string(),
    }
    .to_string()
}

fn literal(mir: &Mir, l: &Lit) -> Sexp {
    match l {
        Lit::Null => atom("null"),
        Lit::Bool(b) => atom(b.to_string()),
        Lit::Number(n) => atom(mir.syms.text(*n)),
        Lit::Str(s) => string(mir.syms.text(*s)),
        Lit::Typed { ty, value } => list(mir.syms.text(*ty), [string(mir.syms.text(*value))]),
        Lit::Interval(v) => list("interval", [string(mir.syms.text(*v))]),
    }
}
