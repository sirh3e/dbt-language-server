//! Expressions. Anything not modelled becomes `Opaque { children }`, so
//! the column references inside it are still seen.

use cogitator_ir::common::{BinaryOp, Lit, UnaryOp};
use cogitator_ir::hir::{ColRef, ExprId, ExprKind, Ident, JinjaRole, Resolution, ScopeKind};
use cogitator_ir::{DegradationKind, Span};
use sqlparser::ast::{self, Expr};

use super::Lowerer;
use crate::caps::MAX_EXPR_DEPTH;
use crate::types::to_sql_type;

impl Lowerer<'_> {
    /// Lowers one expression. The depth check is a backstop: the
    /// pre-parse bound normally stops these inputs from reaching here.
    pub(super) fn expr(&mut self, e: &Expr) -> ExprId {
        if self.depth >= MAX_EXPR_DEPTH {
            return self.depth_hole();
        }
        if self.over_budget() {
            return self.alloc_expr(Span::default(), ExprKind::Hole);
        }
        self.depth += 1;
        let id = self.expr_inner(e);
        self.depth -= 1;
        id
    }

    fn depth_hole(&mut self) -> ExprId {
        if !self.depth_reported {
            self.depth_reported = true;
            self.degrade(
                DegradationKind::CapExceeded,
                Span::default(),
                format!("expression depth exceeds {MAX_EXPR_DEPTH}"),
            );
        }
        self.alloc_expr(Span::default(), ExprKind::Hole)
    }

    fn expr_inner(&mut self, e: &Expr) -> ExprId {
        match e {
            Expr::Identifier(ident) => self.column(std::slice::from_ref(ident)),
            Expr::CompoundIdentifier(path) => self.column(path),
            Expr::Value(value) => self.literal(value),
            Expr::BinaryOp { left, op, right } => self.binary_op(left, op, right),
            Expr::UnaryOp { op, expr } => self.unary_op(op, expr),
            Expr::Nested(inner) => self.expr(inner),
            Expr::IsNull(inner) => self.unary(UnaryOp::IsNull, inner),
            Expr::IsNotNull(inner) => self.unary(UnaryOp::IsNotNull, inner),
            Expr::IsTrue(inner) => self.unary(UnaryOp::IsTrue, inner),
            Expr::IsFalse(inner) => self.unary(UnaryOp::IsFalse, inner),
            Expr::IsNotTrue(inner) => self.unary_named("is not true", inner),
            Expr::IsNotFalse(inner) => self.unary_named("is not false", inner),
            Expr::IsUnknown(inner) => self.unary_named("is unknown", inner),
            Expr::IsNotUnknown(inner) => self.unary_named("is not unknown", inner),
            Expr::IsDistinctFrom(a, b) => self.binary_named("is distinct from", a, b),
            Expr::IsNotDistinctFrom(a, b) => self.binary_named("is not distinct from", a, b),
            Expr::InList {
                expr,
                list,
                negated,
            } => self.in_list(expr, list, *negated),
            Expr::InSubquery {
                expr,
                subquery,
                negated,
            } => self.in_subquery(expr, subquery, *negated),
            Expr::Between {
                expr,
                negated,
                low,
                high,
            } => self.between(expr, low, high, *negated),
            Expr::Like {
                negated,
                expr,
                pattern,
                escape_char,
                ..
            } => self.like(*negated, false, expr, pattern, escape_char.as_deref()),
            Expr::ILike {
                negated,
                expr,
                pattern,
                escape_char,
                ..
            } => self.like(*negated, true, expr, pattern, escape_char.as_deref()),
            Expr::RLike {
                negated,
                expr,
                pattern,
                ..
            } => {
                let name = if *negated { "not rlike" } else { "rlike" };
                self.binary_named(name, expr, pattern)
            }
            Expr::SimilarTo {
                negated,
                expr,
                pattern,
                ..
            } => {
                let name = if *negated { "not similar to" } else { "similar to" };
                self.binary_named(name, expr, pattern)
            }
            Expr::AnyOp {
                left,
                compare_op,
                right,
                ..
            }
            | Expr::AllOp {
                left,
                compare_op,
                right,
            } => self.quantified(left, compare_op, right),
            Expr::Cast { expr, data_type, .. } => self.cast(expr, data_type),
            Expr::Case {
                operand,
                conditions,
                else_result,
                ..
            } => self.case(operand.as_deref(), conditions, else_result.as_deref()),
            Expr::Function(f) => self.function(f),
            Expr::Exists { subquery, negated } => self.exists(subquery, *negated),
            Expr::Subquery(q) => self.scalar_subquery(q),
            Expr::TypedString(ts) => self.typed_string(ts),
            Expr::Interval(interval) => self.interval(interval),
            Expr::Named { expr, .. } | Expr::Prefixed { value: expr, .. } | Expr::Collate { expr, .. } => {
                self.expr(expr)
            }
            Expr::AtTimeZone {
                timestamp,
                time_zone,
            } => {
                let children = [timestamp.as_ref(), time_zone.as_ref()];
                self.call_like("at_time_zone", &children)
            }
            other => self.opaque_expr(other),
        }
    }

    pub(super) fn column(&mut self, path: &[ast::Ident]) -> ExprId {
        if let Some(region) = self.region_in_path(path) {
            return self.jinja_expr(region, JinjaRole::Scalar);
        }
        let span = self.path_span(path);
        let Some((name, quals)) = path.split_last() else {
            return self.alloc_expr(span, ExprKind::Hole);
        };
        let qualifier: Vec<Ident> = quals.iter().map(|q| self.ident(q)).collect();
        let name_ident = self.ident(name);
        let lambda_ref = qualifier.is_empty() && self.lambda.contains(&name_ident.folded);
        if lambda_ref {
            return self.alloc_expr(span, ExprKind::Opaque { children: Vec::new() });
        }
        let syms: Vec<_> = qualifier.iter().map(|q| q.folded).collect();
        let res = self.resolve(&syms, name_ident.folded);
        if res == Resolution::Unresolved {
            if let Some(field) = self.struct_field(path, span, &qualifier) {
                return field;
            }
        }
        self.col_ref(span, qualifier, name_ident, Some(res))
    }

    /// `a.b` where `a` is a column, not a relation: a struct field.
    fn struct_field(&mut self, path: &[ast::Ident], span: Span, qualifier: &[Ident]) -> Option<ExprId> {
        let first = *qualifier.first()?;
        let head = self.resolve(&[], first.folded);
        if head == Resolution::Unresolved {
            return None;
        }
        let name = self.ident(path.first()?);
        let col = self.col_ref(self.ident_span(path.first()?), Vec::new(), name, Some(head));
        Some(self.alloc_expr(span, ExprKind::Opaque { children: vec![col] }))
    }

    fn col_ref(&mut self, span: Span, qualifier: Vec<Ident>, name: Ident, res: Option<Resolution>) -> ExprId {
        let col = self.hir.cols.alloc(ColRef {
            span,
            qualifier,
            name,
            res,
        });
        self.alloc_expr(span, ExprKind::Col(col))
    }

    pub(super) fn jinja_expr(&mut self, region: usize, role: JinjaRole) -> ExprId {
        let span = self.region_span(region);
        match self.take_region(region, role) {
            Some(id) => self.alloc_expr(span, ExprKind::Jinja(id)),
            None => self.alloc_expr(span, ExprKind::Hole),
        }
    }

    fn literal(&mut self, value: &ast::ValueWithSpan) -> ExprId {
        let span = self.sql_span(value.span).unwrap_or_default();
        let lit = match &value.value {
            ast::Value::Number(text, _) => Lit::Number(self.intern(&text.to_string())),
            ast::Value::Boolean(b) => Lit::Bool(*b),
            ast::Value::Null => Lit::Null,
            ast::Value::Placeholder(_) => {
                return self.alloc_expr(span, ExprKind::Opaque { children: Vec::new() });
            }
            other => match string_literal(other) {
                Some(text) => Lit::Str(self.intern(&text)),
                None => {
                    return self.alloc_expr(span, ExprKind::Opaque { children: Vec::new() });
                }
            },
        };
        self.alloc_expr(span, ExprKind::Lit(lit))
    }

    fn typed_string(&mut self, ts: &ast::TypedString) -> ExprId {
        let span = self.sql_span(ts.value.span).unwrap_or_default();
        let ty = self.intern(&ts.data_type.to_string());
        let text = string_literal(&ts.value.value).unwrap_or_default();
        let value = self.intern(&text);
        self.alloc_expr(span, ExprKind::Lit(Lit::Typed { ty, value }))
    }

    fn interval(&mut self, interval: &ast::Interval) -> ExprId {
        if let Expr::Value(v) = interval.value.as_ref() {
            let span = self.sql_span(v.span).unwrap_or_default();
            let text = interval.to_string();
            let sym = self.intern(&text);
            return self.alloc_expr(span, ExprKind::Lit(Lit::Interval(sym)));
        }
        let child = self.expr(&interval.value);
        let span = self.union_spans(&[child]);
        self.alloc_expr(
            span,
            ExprKind::Opaque {
                children: vec![child],
            },
        )
    }

    fn binary_op(&mut self, left: &Expr, op: &ast::BinaryOperator, right: &Expr) -> ExprId {
        use ast::BinaryOperator as B;
        let op = match op {
            B::Eq => BinaryOp::Eq,
            B::NotEq => BinaryOp::NotEq,
            B::Lt => BinaryOp::Lt,
            B::LtEq => BinaryOp::LtEq,
            B::Gt => BinaryOp::Gt,
            B::GtEq => BinaryOp::GtEq,
            B::And => BinaryOp::And,
            B::Or => BinaryOp::Or,
            B::Plus => BinaryOp::Plus,
            B::Minus => BinaryOp::Minus,
            B::Multiply => BinaryOp::Mul,
            B::Divide => BinaryOp::Div,
            B::Modulo => BinaryOp::Mod,
            B::StringConcat => BinaryOp::Concat,
            other => BinaryOp::Other(self.intern(&other.to_string())),
        };
        self.binary(op, left, right)
    }

    fn binary(&mut self, op: BinaryOp, left: &Expr, right: &Expr) -> ExprId {
        let l = self.expr(left);
        let r = self.expr(right);
        let span = self.union_spans(&[l, r]);
        self.alloc_expr(span, ExprKind::Binary { op, l, r })
    }

    fn binary_named(&mut self, name: &str, a: &Expr, b: &Expr) -> ExprId {
        let op = BinaryOp::Other(self.intern(name));
        self.binary(op, a, b)
    }

    fn quantified(&mut self, left: &Expr, op: &ast::BinaryOperator, right: &Expr) -> ExprId {
        let name = format!("{op} quantified");
        self.binary_named(&name, left, right)
    }

    fn unary_op(&mut self, op: &ast::UnaryOperator, inner: &Expr) -> ExprId {
        use ast::UnaryOperator as U;
        match op {
            U::Not => self.unary(UnaryOp::Not, inner),
            U::Minus => self.unary(UnaryOp::Neg, inner),
            U::Plus => self.expr(inner),
            other => self.unary_named(&other.to_string(), inner),
        }
    }

    fn unary(&mut self, op: UnaryOp, inner: &Expr) -> ExprId {
        let e = self.expr(inner);
        let span = self.union_spans(&[e]);
        self.alloc_expr(span, ExprKind::Unary { op, e })
    }

    fn unary_named(&mut self, name: &str, inner: &Expr) -> ExprId {
        let op = UnaryOp::Other(self.intern(name));
        self.unary(op, inner)
    }

    fn in_list(&mut self, expr: &Expr, list: &[Expr], negated: bool) -> ExprId {
        let e = self.expr(expr);
        let list: Vec<ExprId> = list.iter().map(|x| self.expr(x)).collect();
        let mut all = list.clone();
        all.push(e);
        let span = self.union_spans(&all);
        self.alloc_expr(span, ExprKind::InList { e, list, negated })
    }

    fn in_subquery(&mut self, expr: &Expr, subquery: &ast::Query, negated: bool) -> ExprId {
        let e = self.expr(expr);
        let q = self.query(subquery, ScopeKind::Subquery, false);
        let span = self.union_spans(&[e]);
        self.alloc_expr(span, ExprKind::InQuery { e, q, negated })
    }

    fn between(&mut self, expr: &Expr, low: &Expr, high: &Expr, negated: bool) -> ExprId {
        let (e, lo, hi) = (self.expr(expr), self.expr(low), self.expr(high));
        let span = self.union_spans(&[e, lo, hi]);
        self.alloc_expr(span, ExprKind::Between { e, lo, hi, negated })
    }

    fn like(&mut self, negated: bool, insensitive: bool, expr: &Expr, pattern: &Expr, escape: Option<&Expr>) -> ExprId {
        if escape.is_some() {
            let mut children = vec![expr, pattern];
            children.extend(escape);
            return self.opaque_of(&children);
        }
        let op = match (negated, insensitive) {
            (false, false) => BinaryOp::Like,
            (true, false) => BinaryOp::NotLike,
            (false, true) => BinaryOp::ILike,
            (true, true) => BinaryOp::NotILike,
        };
        self.binary(op, expr, pattern)
    }

    fn cast(&mut self, expr: &Expr, data_type: &ast::DataType) -> ExprId {
        let e = self.expr(expr);
        let ty = to_sql_type(data_type);
        let span = self.union_spans(&[e]);
        self.alloc_expr(span, ExprKind::Cast { e, ty })
    }

    fn case(
        &mut self,
        operand: Option<&Expr>,
        conditions: &[ast::CaseWhen],
        else_result: Option<&Expr>,
    ) -> ExprId {
        let operand = operand.map(|e| self.expr(e));
        let arms: Vec<(ExprId, ExprId)> = conditions
            .iter()
            .map(|c| (self.expr(&c.condition), self.expr(&c.result)))
            .collect();
        let else_ = else_result.map(|e| self.expr(e));
        let mut parts: Vec<ExprId> = arms.iter().flat_map(|(w, t)| [*w, *t]).collect();
        parts.extend(operand);
        parts.extend(else_);
        let span = self.union_spans(&parts);
        self.alloc_expr(
            span,
            ExprKind::Case {
                operand,
                arms,
                else_,
            },
        )
    }

    fn exists(&mut self, subquery: &ast::Query, negated: bool) -> ExprId {
        let q = self.query(subquery, ScopeKind::Subquery, false);
        let span = self.hir.queries.get(q).map_or(Span::default(), |x| x.span);
        let id = self.alloc_expr(span, ExprKind::Exists(q));
        if negated {
            self.alloc_expr(span, ExprKind::Unary { op: UnaryOp::Not, e: id })
        } else {
            id
        }
    }

    pub(super) fn scalar_subquery(&mut self, subquery: &ast::Query) -> ExprId {
        let q = self.query(subquery, ScopeKind::Subquery, false);
        let span = self.hir.queries.get(q).map_or(Span::default(), |x| x.span);
        self.alloc_expr(span, ExprKind::Scalar(q))
    }

    /// `name(children...)` for SQL's keyword-call forms.
    pub(super) fn call_like(&mut self, name: &str, children: &[&Expr]) -> ExprId {
        let args: Vec<ExprId> = children.iter().map(|e| self.expr(e)).collect();
        let span = self.union_spans(&args);
        let name = self.intern(name);
        self.alloc_expr(
            span,
            ExprKind::Call {
                name,
                args,
                distinct: false,
                filter: None,
                over: None,
            },
        )
    }

    /// `Opaque { children }`, never silently nothing.
    pub(super) fn opaque_of(&mut self, children: &[&Expr]) -> ExprId {
        let children: Vec<ExprId> = children.iter().map(|e| self.expr(e)).collect();
        let span = self.union_spans(&children);
        self.alloc_expr(span, ExprKind::Opaque { children })
    }
}

/// The text of a string-valued literal, without quotes.
fn string_literal(value: &ast::Value) -> Option<String> {
    use ast::Value as V;
    match value {
        V::SingleQuotedString(s)
        | V::DoubleQuotedString(s)
        | V::TripleSingleQuotedString(s)
        | V::TripleDoubleQuotedString(s)
        | V::EscapedStringLiteral(s)
        | V::UnicodeStringLiteral(s)
        | V::NationalStringLiteral(s)
        | V::HexStringLiteral(s)
        | V::SingleQuotedByteStringLiteral(s)
        | V::DoubleQuotedByteStringLiteral(s)
        | V::TripleSingleQuotedByteStringLiteral(s)
        | V::TripleDoubleQuotedByteStringLiteral(s)
        | V::SingleQuotedRawStringLiteral(s)
        | V::DoubleQuotedRawStringLiteral(s)
        | V::TripleSingleQuotedRawStringLiteral(s)
        | V::TripleDoubleQuotedRawStringLiteral(s) => Some(s.clone()), // boundary: interned next
        V::DollarQuotedString(d) => Some(d.value.clone()), // boundary: interned next
        _ => None,
    }
}
