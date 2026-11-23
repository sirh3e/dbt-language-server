//! A tiny context of scanned macros for the dispatch and call checks.

use std::path::PathBuf;

use cogitator_dialects::Dialect;
use cogitator_parser::jinja_scan::extract_macros;

use crate::context::MacroDetails;
use crate::features::DbtContext;
use crate::{Document, Documents};

/// `files` are (package, file name, macro source); the first package is
/// the root project.
pub(crate) fn ctx(adapter: &str, files: &[(&str, &str, &str)]) -> DbtContext {
    let mut ctx = DbtContext {
        dialect: Dialect::new(adapter),
        ..Default::default()
    };
    ctx.project.name = files.first().map(|f| f.0).unwrap_or("root").to_string();
    for (package, file, text) in files {
        let path = PathBuf::from(format!("/p/{package}/macros/{file}"));
        for raw in extract_macros(text) {
            let details = MacroDetails {
                name: raw.name.clone(),
                project_name: package.to_string(),
                description: raw.signature.clone(),
                params: raw.params.clone(),
                body: raw.body.clone(),
                uri: path.clone(),
                range: Default::default(),
            };
            ctx.macro_detail_map
                .entry(package.to_string())
                .or_default()
                .insert(raw.name, details);
        }
    }
    ctx
}

pub(crate) fn doc(text: &str) -> Document {
    let mut docs = Documents::default();
    let uri = "file:///p/root/models/m.sql".to_string();
    docs.open(uri.clone(), text, &Dialect::default());
    let doc = docs.get(&uri).unwrap();
    Document {
        uri: doc.uri.clone(),
        text: doc.text.clone(),
        parsed: doc.parsed.clone(),
        generation: doc.generation,
    }
}
