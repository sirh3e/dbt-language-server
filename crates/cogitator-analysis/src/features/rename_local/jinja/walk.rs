//! The scope walk over minijinja's own AST: every binding (`set`, block
//! `set`, `for` targets, macro and `call(…)` parameters, `with`, imports)
//! and every use (`Var`), each use resolved to the binding visible at its
//! position, in source order, as Jinja scopes them:
//!
//! - a `for` body is a scope: its targets, and a `set` inside it, do not
//!   leak out (the `else` body is outside);
//! - a macro (and a `{% call(args) %}` body) is a scope holding its
//!   parameters; defaults are evaluated outside it;
//! - `if` makes no scope; a use resolves to the innermost scope that bound
//!   the name *before* the use, so `{% set x = x + 1 %}` reads the outer
//!   `x` and a later `set` of the same name in one scope is the same var.

use std::collections::HashMap;

use cogitator_ir::Span;
use minijinja::machinery::ast::{CallArg, Expr, Stmt};

use super::{Analysis, CallRef, Local, LocalKind, ScopeInfo};

pub(super) struct Walker<'s> {
    pub out: Analysis,
    maps: Vec<HashMap<&'s str, usize>>,
    stack: Vec<usize>,
}

fn span_of(s: minijinja::machinery::Span) -> Span {
    Span::new(s.start_offset, s.end_offset)
}

pub(super) fn expr_span(e: &Expr<'_>) -> Span {
    span_of(match e {
        Expr::Var(x) => x.span(),
        Expr::Const(x) => x.span(),
        Expr::Slice(x) => x.span(),
        Expr::UnaryOp(x) => x.span(),
        Expr::BinOp(x) => x.span(),
        Expr::Compare(x) => x.span(),
        Expr::IfExpr(x) => x.span(),
        Expr::Filter(x) => x.span(),
        Expr::Test(x) => x.span(),
        Expr::GetAttr(x) => x.span(),
        Expr::GetItem(x) => x.span(),
        Expr::Call(x) => x.span(),
        Expr::List(x) => x.span(),
        Expr::Tuple(x) => x.span(),
        Expr::Map(x) => x.span(),
    })
}

impl<'s> Walker<'s> {
    pub fn new(len: usize) -> Self {
        let root = ScopeInfo {
            parent: None,
            span: Span::new(0, len as u32),
        };
        Walker {
            out: Analysis {
                scopes: vec![root],
                ..Analysis::default()
            },
            maps: vec![HashMap::new()],
            stack: vec![0],
        }
    }

    fn current(&self) -> usize {
        *self.stack.last().unwrap_or(&0)
    }

    fn push(&mut self, span: Span) {
        let parent = self.current();
        self.out.scopes.push(ScopeInfo {
            parent: Some(parent),
            span,
        });
        self.maps.push(HashMap::new());
        self.stack.push(self.out.scopes.len() - 1);
    }

    fn pop(&mut self) {
        self.stack.pop();
    }

    fn bind_name(&mut self, name: &'s str, span: Span, kind: LocalKind, owner: Option<&str>) {
        let scope = self.current();
        if let Some(&idx) = self.maps[scope].get(name) {
            self.out.locals[idx].bindings.push(span);
            return;
        }
        self.maps[scope].insert(name, self.out.locals.len());
        self.out.locals.push(Local {
            name: name.to_string(), // boundary: the analysis outlives the AST
            kind,
            scope,
            bindings: vec![span],
            uses: Vec::new(),
            macro_name: owner.map(str::to_string),
        });
    }

    /// A binding target: a name, or a tuple/list of them; anything else
    /// (`ns.x`) is read, not bound.
    fn bind(&mut self, target: &Expr<'s>, kind: LocalKind, owner: Option<&str>) {
        match target {
            Expr::Var(v) => self.bind_name(v.id, span_of(v.span()), kind, owner),
            Expr::Tuple(t) => t.items.iter().for_each(|e| self.bind(e, kind, owner)),
            Expr::List(l) => l.items.iter().for_each(|e| self.bind(e, kind, owner)),
            other => self.expr(other),
        }
    }

    fn use_name(&mut self, name: &'s str, span: Span) {
        for &scope in self.stack.iter().rev() {
            if let Some(&idx) = self.maps[scope].get(name) {
                self.out.locals[idx].uses.push(span);
                return;
            }
        }
        self.out.free.push((name.to_string(), span)); // boundary: as above
    }

    pub fn stmts(&mut self, body: &[Stmt<'s>]) {
        body.iter().for_each(|s| self.stmt(s));
    }

    pub fn stmt(&mut self, stmt: &Stmt<'s>) {
        match stmt {
            Stmt::Template(t) => self.stmts(&t.children),
            Stmt::EmitExpr(e) => self.expr(&e.expr),
            Stmt::ForLoop(f) => {
                self.expr(&f.iter);
                self.push(span_of(f.span()));
                self.bind(&f.target, LocalKind::Loop, None);
                if let Some(filter) = &f.filter_expr {
                    self.expr(filter);
                }
                self.stmts(&f.body);
                self.pop();
                self.stmts(&f.else_body);
            }
            Stmt::IfCond(i) => {
                self.expr(&i.expr);
                self.stmts(&i.true_body);
                self.stmts(&i.false_body);
            }
            Stmt::WithBlock(w) => {
                w.assignments.iter().for_each(|(_, v)| self.expr(v));
                self.push(span_of(w.span()));
                for (target, _) in &w.assignments {
                    self.bind(target, LocalKind::With, None);
                }
                self.stmts(&w.body);
                self.pop();
            }
            Stmt::Set(s) => {
                self.expr(&s.expr);
                self.bind(&s.target, LocalKind::Set, None);
            }
            Stmt::SetBlock(s) => {
                if let Some(filter) = &s.filter {
                    self.expr(filter);
                }
                self.stmts(&s.body);
                self.bind(&s.target, LocalKind::Set, None);
            }
            Stmt::AutoEscape(a) => {
                self.expr(&a.enabled);
                self.stmts(&a.body);
            }
            Stmt::FilterBlock(f) => {
                self.expr(&f.filter);
                self.stmts(&f.body);
            }
            Stmt::Import(i) => {
                self.expr(&i.expr);
                self.bind(&i.name, LocalKind::Import, None);
            }
            Stmt::FromImport(f) => {
                self.expr(&f.expr);
                for (name, alias) in &f.names {
                    self.bind(alias.as_ref().unwrap_or(name), LocalKind::Import, None);
                }
            }
            Stmt::Macro(m) => {
                m.defaults.iter().for_each(|d| self.expr(d));
                self.push(span_of(m.span()));
                for arg in &m.args {
                    self.bind(arg, LocalKind::Param, Some(m.name));
                }
                self.stmts(&m.body);
                self.pop();
            }
            Stmt::CallBlock(c) => {
                self.call(&c.call.expr, &c.call.args);
                let decl = &c.macro_decl;
                decl.defaults.iter().for_each(|d| self.expr(d));
                self.push(span_of(c.span()));
                for arg in &decl.args {
                    self.bind(arg, LocalKind::CallerArg, None);
                }
                self.stmts(&decl.body);
                self.pop();
            }
            Stmt::Do(d) => self.call(&d.call.expr, &d.call.args),
            _ => {}
        }
    }

    fn call(&mut self, callee: &Expr<'s>, args: &[CallArg<'s>]) {
        let name = match callee {
            Expr::Var(v) => Some(v.id),
            Expr::GetAttr(g) => Some(g.name),
            _ => None,
        };
        if let Some(name) = name {
            self.out.calls.push(CallRef {
                name: name.to_string(), // boundary: as above
                after_callee: expr_span(callee).end,
            });
        }
        self.expr(callee);
        self.args(args);
    }

    pub fn expr(&mut self, e: &Expr<'s>) {
        match e {
            Expr::Var(v) => self.use_name(v.id, span_of(v.span())),
            Expr::Const(_) => {}
            Expr::Slice(s) => {
                self.expr(&s.expr);
                for part in [&s.start, &s.stop, &s.step].into_iter().flatten() {
                    self.expr(part);
                }
            }
            Expr::UnaryOp(u) => self.expr(&u.expr),
            Expr::BinOp(b) => {
                self.expr(&b.left);
                self.expr(&b.right);
            }
            Expr::Compare(c) => {
                self.expr(&c.expr);
                c.ops.iter().for_each(|op| self.expr(&op.expr));
            }
            Expr::IfExpr(i) => {
                self.expr(&i.test_expr);
                self.expr(&i.true_expr);
                if let Some(f) = &i.false_expr {
                    self.expr(f);
                }
            }
            Expr::Filter(f) => {
                if let Some(inner) = &f.expr {
                    self.expr(inner);
                }
                self.args(&f.args);
            }
            Expr::Test(t) => {
                self.expr(&t.expr);
                self.args(&t.args);
            }
            Expr::GetAttr(g) => self.expr(&g.expr),
            Expr::GetItem(g) => {
                self.expr(&g.expr);
                self.expr(&g.subscript_expr);
            }
            Expr::Call(c) => self.call(&c.expr, &c.args),
            Expr::List(l) => l.items.iter().for_each(|i| self.expr(i)),
            Expr::Tuple(t) => t.items.iter().for_each(|i| self.expr(i)),
            Expr::Map(m) => {
                m.keys.iter().for_each(|k| self.expr(k));
                m.values.iter().for_each(|v| self.expr(v));
            }
        }
    }

    fn args(&mut self, args: &[CallArg<'s>]) {
        for arg in args {
            match arg {
                CallArg::Pos(e)
                | CallArg::Kwarg(_, e)
                | CallArg::PosSplat(e)
                | CallArg::KwargSplat(e) => self.expr(e),
            }
        }
    }
}
