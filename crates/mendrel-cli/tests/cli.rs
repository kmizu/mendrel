use std::fs;
use std::process::Command;

#[test]
fn mendrel_frontend_reports_its_own_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_mendrel"))
        .arg("--version")
        .output()
        .expect("run mendrel");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8 stdout"),
        "mendrel 0.0.1\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn generated_check_uses_the_invocation_workspace() {
    let directory = std::env::temp_dir().join(format!("mendrel-cwd-test-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("create isolated current directory");

    let output = Command::new(env!("CARGO_BIN_EXE_mendrel"))
        .current_dir(&directory)
        .args(["xtask", "generated", "--check"])
        .output()
        .expect("run mendrel generated check outside a workspace");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .expect("UTF-8 stderr")
            .contains("generated syntax inventory error")
    );
    fs::remove_dir_all(directory).expect("remove isolated current directory");
}
