use std::env;
use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let current_directory = match env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("mendrelc: cannot determine current directory: {error}");
            return ExitCode::FAILURE;
        }
    };
    ExitCode::from(mendrelc::run(
        "mendrelc",
        env::args_os().skip(1),
        &current_directory,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ))
}
