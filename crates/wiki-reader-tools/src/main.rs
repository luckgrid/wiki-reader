use std::{path::Path, process::ExitCode};

fn main() -> ExitCode {
    // The manifest sits two levels below the repository root, so the result
    // does not depend on the directory the tool is run from.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match wiki_reader_tools::check(&root) {
        Ok(report) if report.errors.is_empty() => {
            println!("ok: {} markdown files", report.files);
            ExitCode::SUCCESS
        }
        Ok(report) => {
            eprintln!("link-check failed:");
            for error in &report.errors {
                eprintln!("  {error}");
            }
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("link-check: {error}");
            ExitCode::FAILURE
        }
    }
}
