//! Compiles the vendored tree-sitter-jinja2 grammar (specs/rust-port/06 §3.2).
//! Like all C in this project, CI routes this through `zig cc` (07 §5).

fn main() {
    let src = std::path::Path::new("../../third_party/tree-sitter-jinja2/src");
    println!("cargo:rerun-if-changed={}", src.join("parser.c").display());
    cc::Build::new()
        .include(src)
        .file(src.join("parser.c"))
        .warnings(false)
        .compile("tree-sitter-jinja2");
}
