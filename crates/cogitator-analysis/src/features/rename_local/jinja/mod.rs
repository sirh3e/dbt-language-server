//! The Jinja side of local renames: one file's bindings and uses, read
//! off minijinja's parser (`machinery::parse`) and scoped by [`walk`].
//! Pure: the text in, an [`Analysis`] out.

mod kwargs;
mod source;
mod walk;

#[cfg(test)]
mod tests;

pub(crate) use kwargs::keyword_args;

use cogitator_ir::Span;
use minijinja::machinery::parse;
use minijinja::syntax::SyntaxConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalKind {
    Set,
    Loop,
    Param,
    CallerArg,
    With,
    Import,
}

/// One local name: every binding site of it in one scope, and every use
/// that resolves to it.
#[derive(Debug, Clone)]
pub(crate) struct Local {
    pub name: String,
    pub kind: LocalKind,
    pub scope: usize,
    pub bindings: Vec<Span>,
    pub uses: Vec<Span>,
    /// A macro parameter's macro.
    pub macro_name: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ScopeInfo {
    pub parent: Option<usize>,
    pub span: Span,
}

/// A call in the file: the callee's last name, and where its argument
/// list starts (right after the callee).
#[derive(Debug, Clone)]
pub(crate) struct CallRef {
    pub name: String,
    pub after_callee: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Analysis {
    pub locals: Vec<Local>,
    pub scopes: Vec<ScopeInfo>,
    /// Uses no binding of the file decides (context names, globals).
    pub free: Vec<(String, Span)>,
    pub calls: Vec<CallRef>,
}

/// The file's locals; `None` when minijinja cannot parse it.
pub(crate) fn analyze(text: &str) -> Option<Analysis> {
    let source = source::parseable(text)?;
    let ast = parse(&source, "<local>", SyntaxConfig::default()).ok()?;
    let mut walker = walk::Walker::new(source.len());
    walker.stmt(&ast);
    // the AST borrows `source`; the analysis owns its strings
    Some(walker.out)
}

fn touches(span: Span, offset: u32) -> bool {
    span.start <= offset && offset <= span.end
}

impl Analysis {
    /// The local whose binding or use is at `offset`.
    pub fn at(&self, offset: u32) -> Option<usize> {
        self.locals.iter().position(|l| {
            l.bindings
                .iter()
                .chain(&l.uses)
                .any(|s| touches(*s, offset))
        })
    }

    fn within(&self, mut scope: usize, ancestor: usize) -> bool {
        loop {
            if scope == ancestor {
                return true;
            }
            match self.scopes[scope].parent {
                Some(p) => scope = p,
                None => return false,
            }
        }
    }

    /// Why renaming local `idx` to `new` would change what the template
    /// means, if it would: `new` is bound in the local's scope or one
    /// inside it (two names would merge or one would be shadowed), or a
    /// use of `new` already sits in that scope (it would be captured).
    pub fn collision(&self, idx: usize, new: &str) -> Option<String> {
        let local = &self.locals[idx];
        let scope = local.scope;
        let span = self.scopes[scope].span;
        let bound = self
            .locals
            .iter()
            .any(|l| l.name == new && self.within(l.scope, scope));
        let used = self
            .locals
            .iter()
            .filter(|l| l.name == new)
            .flat_map(|l| &l.uses)
            .chain(self.free.iter().filter(|(n, _)| n == new).map(|(_, s)| s))
            .any(|s| span.contains(*s));
        (bound || used).then(|| {
            format!(
                "`{new}` is already {} where `{}` is visible.",
                if bound { "bound" } else { "used" },
                local.name
            )
        })
    }
}
