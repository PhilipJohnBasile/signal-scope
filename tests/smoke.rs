use assert_cmd::Command;

#[test]
fn cli_help_runs() {
    let mut cmd = Command::cargo_bin("rwe-assistant").expect("binary exists");
    cmd.arg("--help").assert().success();
}
