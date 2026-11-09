//! Same-length masking: every Jinja region is overwritten byte for byte,
//! newlines preserved, so offsets and sqlparser line/columns agree with
//! the template. Placeholders are named by offset, not by their text.

use std::ops::Range;

use cogitator_ir::hir::JinjaRole;
use cogitator_ir::origin::TemplateVariant;
use cogitator_ir::Span;
use cogitator_parser::backend::tokenize_template;
use cogitator_parser::{JinjaKind, LineTable, Token, TokenKind};

use crate::jinja::{classify, classify_tag, config_kwargs, Class, ExprKindTag, Raw, Tag};

/// The HIR `JinjaKind` of a region, before its macro name is interned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RegionKind {
    Ref,
    Source,
    This,
    Var,
    Macro(String),
    IsIncremental,
    Config,
    Expression,
    Statement,
    Comment,
}

#[derive(Debug, Clone)]
pub(crate) struct Region {
    pub span: Span,
    /// The placeholder identifier's bytes; `None` when the region is blanked.
    pub word: Option<Span>,
    pub class: Class,
    pub kind: RegionKind,
    pub clause: bool,
}

impl Region {
    /// The role before position refines it. A region that never finds a
    /// consumer stays Blank, the role for regions inside holes and strings.
    pub fn initial_role(&self) -> JinjaRole {
        if self.clause {
            JinjaRole::ClausePosition
        } else {
            JinjaRole::Blank
        }
    }
}

#[derive(Debug)]
pub(crate) struct Scanned {
    pub mask: String,
    pub regions: Vec<Region>,
    pub variant: TemplateVariant,
    pub configs: Vec<Vec<(String, Raw)>>,
}

#[derive(Debug)]
pub(crate) enum Stop {
    /// Control flow outside the `is_incremental()` shape.
    ControlFlow { detail: String, span: Span },
    /// minijinja could not tokenize the template.
    Malformed { detail: String },
}

/// A block whose extent matters: `{% if is_incremental() %}` or a block set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenBlock {
    Incremental,
    Set,
}

pub(crate) fn scan(text: &str, variant: TemplateVariant) -> Result<Scanned, Stop> {
    let table = LineTable::new(text);
    let (mut tokens, mut segments) = (Vec::new(), Vec::new());
    tokenize_template(text, 0, &table, &mut tokens, &mut segments)
        .map_err(|e| Stop::Malformed { detail: e.to_string() })?;
    let walked = walk_regions(text, &tokens)?;
    let variant = effective_variant(variant, &walked.incremental);
    let mut regions = walked.regions;
    let mut blank_ranges = walked.set_blocks;
    if variant == TemplateVariant::FullRefresh {
        blank_ranges.extend(walked.incremental.iter().copied());
    }
    blank_inner_regions(&mut regions, &blank_ranges);
    let mut configs = Vec::new();
    for region in &regions {
        if region.kind == RegionKind::Config && region.class == Class::Blank {
            if let Some(inner) = text.get(region.span.start as usize..region.span.end as usize) {
                configs.push(config_kwargs(strip_expr_markers(inner)));
            }
        }
    }
    add_comments(text, &segments, &mut regions);
    let mask = render_mask(text, &segments, &regions, &blank_ranges)?;
    Ok(Scanned {
        mask,
        regions,
        variant,
        configs,
    })
}

/// A template with incremental blocks has no `Plain` variant: it is the
/// variant rules run on, `Incremental`.
fn effective_variant(requested: TemplateVariant, incremental: &[Span]) -> TemplateVariant {
    if requested == TemplateVariant::Plain && !incremental.is_empty() {
        TemplateVariant::Incremental
    } else {
        requested
    }
}

struct Walked {
    regions: Vec<Region>,
    /// `{% if is_incremental() %}` … `{% endif %}`, tags included.
    incremental: Vec<Span>,
    set_blocks: Vec<Span>,
}

fn walk_regions(text: &str, tokens: &[Token]) -> Result<Walked, Stop> {
    let mut out = Walked {
        regions: Vec::new(),
        incremental: Vec::new(),
        set_blocks: Vec::new(),
    };
    let mut open: Vec<(OpenBlock, u32)> = Vec::new();
    let mut i = 0;
    while let Some(token) = tokens.get(i) {
        let TokenKind::JinjaOpen(kind) = token.kind else {
            i += 1;
            continue;
        };
        let close_at = tokens[i + 1..]
            .iter()
            .position(|t| t.kind == TokenKind::JinjaClose(kind))
            .map(|p| i + 1 + p);
        let Some(close_at) = close_at else {
            return Err(Stop::Malformed {
                detail: "unterminated Jinja region".to_string(),
            });
        };
        let close = &tokens[close_at];
        let span = Span::new(token.start, close.end);
        let inner = text.get(token.end as usize..close.start as usize).unwrap_or("");
        let region = match kind {
            JinjaKind::Expr => expression_region(text, span, inner),
            JinjaKind::Stmt => statement_region(span, inner, &mut open, &mut out)?,
        };
        out.regions.push(region);
        i = close_at + 1;
    }
    if let Some((_, start)) = open.last() {
        return Err(Stop::ControlFlow {
            detail: "unbalanced control block".to_string(),
            span: Span::new(*start, *start),
        });
    }
    Ok(out)
}

fn expression_region(text: &str, span: Span, inner: &str) -> Region {
    let classified = classify(inner);
    let kind = match (&classified.class, classified.tag) {
        (Class::Relation(crate::jinja::RelInfo::Model { .. }), _) => RegionKind::Ref,
        (Class::Relation(crate::jinja::RelInfo::Source { .. }), _) => RegionKind::Source,
        (Class::Relation(crate::jinja::RelInfo::This), _) => RegionKind::This,
        (_, ExprKindTag::Var) => RegionKind::Var,
        (_, ExprKindTag::Config) => RegionKind::Config,
        (_, ExprKindTag::IsIncremental) => RegionKind::IsIncremental,
        (Class::Macro(name), _) if !name.is_empty() => RegionKind::Macro(name.clone()),
        _ => RegionKind::Expression,
    };
    let word = (classified.class != Class::Blank).then(|| first_line(text, span));
    Region {
        span,
        word,
        class: classified.class,
        kind,
        clause: false,
    }
}

/// The placeholder covers the region's first line: an identifier cannot
/// span a newline, and every newline must stay where the template has it.
fn first_line(text: &str, span: Span) -> Span {
    let line_len = text
        .get(span.start as usize..span.end as usize)
        .and_then(|region| region.find(['\n', '\r']))
        .unwrap_or(span.len() as usize);
    // `{{` is on the first line, so the word is never empty.
    Span::new(span.start, span.start + line_len.max(1) as u32)
}

fn statement_region(
    span: Span,
    inner: &str,
    open: &mut Vec<(OpenBlock, u32)>,
    out: &mut Walked,
) -> Result<Region, Stop> {
    let control = |detail: String| Stop::ControlFlow { detail, span };
    let tag = classify_tag(inner);
    let kind = match &tag {
        Tag::IfIncremental | Tag::EndIf => RegionKind::IsIncremental,
        _ => RegionKind::Statement,
    };
    match tag {
        Tag::IfIncremental => {
            if !open.is_empty() {
                return Err(control("nested is_incremental block".to_string()));
            }
            open.push((OpenBlock::Incremental, span.start));
        }
        Tag::EndIf => match open.pop() {
            Some((OpenBlock::Incremental, start)) => {
                out.incremental.push(Span::new(start, span.end));
            }
            _ => return Err(control("unbalanced endif".to_string())),
        },
        Tag::SetBlock => open.push((OpenBlock::Set, span.start)),
        Tag::EndSet => match open.pop() {
            Some((OpenBlock::Set, start)) => out.set_blocks.push(Span::new(start, span.end)),
            _ => return Err(control("unbalanced endset".to_string())),
        },
        Tag::SetInline | Tag::Do => {}
        Tag::Other(name) => {
            let what = if name.is_empty() { "statement" } else { name.as_str() };
            return Err(control(format!("control flow `{what}` needs the compiled fallback")));
        }
    }
    Ok(Region {
        span,
        word: None,
        class: Class::Blank,
        kind,
        clause: false,
    })
}

fn blank_inner_regions(regions: &mut [Region], ranges: &[Span]) {
    for region in regions {
        if ranges.iter().any(|r| r.contains(region.span)) {
            region.word = None;
            region.class = Class::Blank;
        }
    }
}

/// `{{ config(a=1) }}` → `config(a=1)` for the config parser.
fn strip_expr_markers(region: &str) -> &str {
    let inner = region.strip_prefix("{{").unwrap_or(region);
    let inner = inner.strip_prefix(['-', '+']).unwrap_or(inner);
    let inner = inner.strip_suffix("}}").unwrap_or(inner);
    inner.strip_suffix(['-', '+']).unwrap_or(inner)
}

/// Everything not covered by a segment or a region is a Jinja comment
/// or whitespace minijinja trimmed; comments become Blank nodes.
fn add_comments(text: &str, segments: &[Range<usize>], regions: &mut Vec<Region>) {
    let mut covered: Vec<Range<usize>> = segments.to_vec();
    covered.extend(regions.iter().map(|r| r.span.start as usize..r.span.end as usize));
    covered.sort_by_key(|r| r.start);
    let mut at = 0usize;
    let mut gaps = Vec::new();
    for range in covered.iter().chain(std::iter::once(&(text.len()..text.len()))) {
        if range.start > at {
            gaps.push(at..range.start);
        }
        at = at.max(range.end);
    }
    for gap in gaps {
        if text.get(gap.clone()).is_some_and(|g| g.contains("{#")) {
            regions.push(Region {
                span: Span::new(gap.start as u32, gap.end as u32),
                word: None,
                class: Class::Blank,
                kind: RegionKind::Comment,
                clause: false,
            });
        }
    }
    regions.sort_by_key(|r| r.span.start);
}

fn render_mask(
    text: &str,
    segments: &[Range<usize>],
    regions: &[Region],
    blank_ranges: &[Span],
) -> Result<String, Stop> {
    // Start blank so comments and trimmed whitespace vanish; segments and
    // regions then restore or overwrite their own bytes.
    let source = text.as_bytes();
    let mut out: Vec<u8> = source
        .iter()
        .map(|b| if *b == b'\n' { b'\n' } else { b' ' })
        .collect();
    for segment in segments {
        if let (Some(dst), Some(src)) = (out.get_mut(segment.clone()), source.get(segment.clone())) {
            dst.copy_from_slice(src);
        }
    }
    for region in regions {
        blank(&mut out, region.span);
        if let Some(word) = region.word {
            write_placeholder(&mut out, word);
        }
    }
    for range in blank_ranges {
        blank(&mut out, *range);
    }
    String::from_utf8(out).map_err(|e| Stop::Malformed {
        detail: format!("mask is not valid UTF-8: {e}"),
    })
}

fn blank(out: &mut [u8], span: Span) {
    if let Some(bytes) = out.get_mut(span.start as usize..span.end as usize) {
        for b in bytes.iter_mut().filter(|b| **b != b'\n') {
            *b = b' ';
        }
    }
}

/// `_jjj` over the word; the region's later lines stay blank.
fn write_placeholder(out: &mut [u8], word: Span) {
    if let Some(bytes) = out.get_mut(word.start as usize..word.end as usize) {
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = if i == 0 { b'_' } else { b'j' };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str) -> Scanned {
        scan(text, TemplateVariant::Plain).expect("scan")
    }

    #[test]
    fn mask_is_same_length_and_keeps_newlines() {
        let text = "select {{\n ref('é')\n}} from t -- x\n";
        let s = run(text);
        assert_eq!(s.mask.len(), text.len());
        assert_eq!(
            s.mask.as_bytes().iter().filter(|b| **b == b'\n').count(),
            text.as_bytes().iter().filter(|b| **b == b'\n').count()
        );
        assert!(s.mask.contains("from t"));
        assert!(s.mask.starts_with("select _j\n"));
    }

    #[test]
    fn regions_are_sorted_with_kinds() {
        let s = run("{# c #}select {{ var('a') }}, {{ m() }} {{ config(x=1) }} from {{ ref('b') }}");
        let kinds: Vec<_> = s.regions.iter().map(|r| r.kind.clone()).collect();
        assert_eq!(
            kinds,
            vec![
                RegionKind::Comment,
                RegionKind::Var,
                RegionKind::Macro("m".into()),
                RegionKind::Config,
                RegionKind::Ref,
            ]
        );
        assert_eq!(s.configs.len(), 1);
    }

    #[test]
    fn incremental_variants() {
        let text = "select * from t {% if is_incremental() %}where a > 1{% endif %}";
        let inc = scan(text, TemplateVariant::Incremental).expect("scan");
        assert!(inc.mask.contains("where a > 1"));
        assert_eq!(inc.variant, TemplateVariant::Incremental);
        let full = scan(text, TemplateVariant::FullRefresh).expect("scan");
        assert!(!full.mask.contains("where"));
        // Plain on an incremental model behaves as Incremental
        let plain = scan(text, TemplateVariant::Plain).expect("scan");
        assert!(plain.mask.contains("where a > 1"));
        assert_eq!(plain.variant, TemplateVariant::Incremental);
    }

    #[test]
    fn other_control_flow_stops() {
        for text in [
            "{% for x in y %}{{ x }}{% endfor %}",
            "{% if x %}a{% endif %}",
            "{% if is_incremental() %}a{% else %}b{% endif %}",
            "{% if is_incremental() %}{% if is_incremental() %}a{% endif %}{% endif %}",
            "{% macro m() %}x{% endmacro %}",
        ] {
            let stopped = matches!(
                scan(text, TemplateVariant::Plain),
                Err(Stop::ControlFlow { .. })
            );
            assert!(stopped, "{text}");
        }
    }

    #[test]
    fn set_blocks_blank_their_body() {
        let s = run("{% set a %}select 'x'{% endset %}select 1");
        assert!(!s.mask.contains("'x'"));
        assert!(s.mask.contains("select 1"));
    }

    #[test]
    fn inline_set_is_blanked_but_allowed() {
        let s = run("{% set p = ['a'] %}select 1");
        assert!(s.mask.trim_start().starts_with("select 1"));
    }

    #[test]
    fn malformed_jinja_is_reported() {
        assert!(matches!(
            scan("select {{ a", TemplateVariant::Plain),
            Err(Stop::Malformed { .. })
        ));
    }
}
