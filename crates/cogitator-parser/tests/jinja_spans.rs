//! Where Jinja comments and raw blocks are, including on text minijinja
//! rejects.

use cogitator_parser::jinja_scan::{extract_docs_blocks, extract_macros};
use cogitator_parser::jinja_spans::{blanked, jinja_spans, JinjaSpanKind as K, NOT_CODE, NOT_SQL};

fn kinds(text: &str) -> Vec<(K, &str)> {
    jinja_spans(text)
        .into_iter()
        .map(|s| (s.kind, &text[s.range]))
        .collect()
}

#[test]
fn comment_ends_at_the_first_closer() {
    assert_eq!(
        kinds("{## a ##} x {# b #}"),
        [(K::Comment, "{## a ##}"), (K::Comment, "{# b #}")]
    );
    assert_eq!(kinds("{#} x #}"), [(K::Comment, "{#} x #}")]);
}

#[test]
fn raw_block_splits_into_tags_and_body() {
    assert_eq!(
        kinds("{%- raw -%}{{ a }}{# b #}{% endraw %}"),
        [
            (K::RawTag, "{%- raw -%}"),
            (K::RawBody, "{{ a }}{# b #}"),
            (K::RawTag, "{% endraw %}"),
        ]
    );
}

#[test]
fn raw_inside_a_comment_is_comment() {
    assert_eq!(
        kinds("{# {% raw %} #} x"),
        [(K::Comment, "{# {% raw %} #}")]
    );
}

#[test]
fn comment_delimiter_in_a_jinja_string_opens_nothing() {
    assert_eq!(
        kinds(r#"{{ "{#" }} a {% set x = '#}' %}"#),
        [(K::Code, r#"{{ "{#" }}"#), (K::Code, "{% set x = '#}' %}"),]
    );
}

#[test]
fn unterminated_constructs_run_to_the_end() {
    assert_eq!(kinds("a {# b"), [(K::Comment, "{# b")]);
    assert_eq!(
        kinds("{% raw %} b"),
        [(K::RawTag, "{% raw %}"), (K::RawBody, " b")]
    );
}

#[test]
fn blanking_keeps_length_and_newlines() {
    let text = "a {# é\nb #} {% raw %}{{ c }}{% endraw %} d";
    let not_sql = blanked(text, NOT_SQL);
    assert_eq!(not_sql.len(), text.len());
    assert_eq!(not_sql.matches('\n').count(), 1);
    assert!(not_sql.contains("{{ c }}") && !not_sql.contains('#'));
    assert!(!blanked(text, NOT_CODE).contains("{{"));
}

#[test]
fn regex_fallbacks_ignore_commented_out_blocks() {
    // The stray `{{ '` makes minijinja fail, forcing the regex fallback.
    let text = concat!(
        "{# {% macro dead(a) %}{% endmacro %} #}\n",
        "{% macro live(b) %}{% endmacro %}\n",
        "{{ 'broken\n"
    );
    let names: Vec<String> = extract_macros(text).into_iter().map(|m| m.name).collect();
    assert_eq!(names, ["live"]);

    let text = concat!(
        "{# {% docs dead %}x{% enddocs %} #}\n",
        "{% docs live %}kept # text{% enddocs %}\n",
        "{{ 'broken\n"
    );
    let docs = extract_docs_blocks(text);
    assert_eq!(docs.len(), 1);
    assert_eq!(
        (docs[0].name.as_str(), docs[0].content.as_str()),
        ("live", "kept # text")
    );
}
