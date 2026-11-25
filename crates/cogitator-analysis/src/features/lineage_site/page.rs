//! The site as one self-contained HTML page: the stylesheet, the scripts
//! and the data inline, nothing fetched (a Content-Security-Policy says
//! so to the browser). The assets are the same ones the VS Code webview
//! loads through `dbt.lineageSite`.

use super::LineageSite;

const PAGE: &str = include_str!("assets/page.html");
const STYLE: &str = include_str!("assets/site.css");
const CORE: &str = include_str!("assets/core.js");
const VIEW: &str = include_str!("assets/view.js");

/// The page's stylesheet.
pub fn page_style() -> &'static str {
    STYLE
}

/// The page's script: the pure core, then the view.
pub fn page_script() -> String {
    format!("{CORE}\n{VIEW}")
}

/// `site` as an HTML page that works from `file://`.
pub fn page_html(site: &LineageSite) -> String {
    let data = serde_json::to_string(site).unwrap_or_else(|_| "null".to_string());
    let title = format!("{} \u{2014} lineage", site.project);
    let script = page_script();
    let values = [
        ("GENERATOR", escape_html(&site.generator)),
        ("TITLE", escape_html(&title)),
        ("STYLE", STYLE.to_string()),
        ("DATA", script_safe(&data)),
        ("SCRIPT", script),
    ];
    fill(PAGE, &values)
}

/// `{{NAME}}` placeholders replaced in one pass, so inserted text is never
/// searched for placeholders itself.
fn fill(template: &str, values: &[(&str, String)]) -> String {
    let mut out =
        String::with_capacity(template.len() + values.iter().map(|(_, v)| v.len()).sum::<usize>());
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let value = after.find("}}").and_then(|close| {
            let name = &after[..close];
            values
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| (v, close))
        });
        match value {
            Some((v, close)) => {
                out.push_str(v);
                rest = &after[close + 2..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// JSON that cannot close or comment its `<script>` element: `<`, `>`
/// and `&` only occur inside JSON strings, where the escapes mean the same.
fn script_safe(json: &str) -> String {
    json.replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
