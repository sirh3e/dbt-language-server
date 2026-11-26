//! `cogitator-format`: the formatter on its own, without the language
//! server: format files in place, `--check` them, or format stdin.

mod cli;

fn main() -> std::process::ExitCode {
    cli::run()
}
