use std::env;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, ExitCode};

use mendrel_syntax::generate::{check_inventory, write_inventory};

const USAGE: &str = "usage: xtask <generated [--check]|verify>";

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [command, check] if command == "generated" && check == "--check" => {
            report_generation(check_inventory(repository_root()), "is current")
        }
        [command] if command == "generated" => {
            report_generation(write_inventory(repository_root()), "updated")
        }
        [command] if command == "verify" => verify(),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn verify() -> ExitCode {
    if let Err(error) = check_inventory(repository_root()) {
        eprintln!("verify failed: generated syntax inventory: {error}");
        return ExitCode::FAILURE;
    }
    println!("verify: generated syntax inventory is current");

    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let commands: [(&OsStr, &[&str]); 3] = [
        (cargo.as_os_str(), &["fmt", "--all", "--", "--check"]),
        (
            cargo.as_os_str(),
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        (cargo.as_os_str(), &["test", "--workspace"]),
    ];
    for (program, arguments) in commands {
        if !run_child(program, arguments) {
            return ExitCode::FAILURE;
        }
    }
    if !run_child(
        OsStr::new("python"),
        &["scripts/validate_pack.py", "--strict-schema"],
    ) {
        return ExitCode::FAILURE;
    }
    println!("verify: all checks passed");
    ExitCode::SUCCESS
}

fn run_child(program: &OsStr, arguments: &[&str]) -> bool {
    let rendered = std::iter::once(program.to_string_lossy().into_owned())
        .chain(arguments.iter().map(|argument| (*argument).to_owned()))
        .collect::<Vec<_>>()
        .join(" ");
    println!("verify: {rendered}");
    match Command::new(program)
        .args(arguments)
        .current_dir(repository_root())
        .status()
    {
        Ok(status) if status.success() => true,
        Ok(status) => {
            eprintln!("verify failed: `{rendered}` exited with {status}");
            false
        }
        Err(error) => {
            eprintln!("verify failed: could not run `{rendered}`: {error}");
            false
        }
    }
}

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives directly below the repository root")
}

fn report_generation(
    result: Result<(), mendrel_syntax::generate::GenerateError>,
    success: &str,
) -> ExitCode {
    match result {
        Ok(()) => {
            println!("generated syntax inventory {success}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("generated syntax inventory error: {error}");
            ExitCode::FAILURE
        }
    }
}
