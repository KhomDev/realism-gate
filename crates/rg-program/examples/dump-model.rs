//! Print the entry-point authority model for a target directory.
//!
//! This is a debugging tool over `rg-program`; it reads only the directory it
//! is given and never runs a rule. The same document is available from
//! `realism-gate check --dump-program <dir>`.
//!
//! Usage: `cargo run -p rg-program --example dump-model -- <target-dir>`

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let Some(target) = arguments.next() else {
        eprintln!("usage: dump-model <target-dir>");
        return ExitCode::from(2);
    };
    if arguments.next().is_some() {
        eprintln!("usage: dump-model <target-dir>");
        return ExitCode::from(2);
    }
    match rg_program::ProgramModel::resolve(&PathBuf::from(target)) {
        Ok(model) => {
            print!("{}", model.to_json_pretty());
            println!();
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("rg-program: {error}");
            ExitCode::from(2)
        }
    }
}
