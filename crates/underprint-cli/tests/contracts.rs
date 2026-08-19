use std::process::Command;

#[test]
fn version_json_matches_the_committed_contract_fixture() {
    let output = Command::new(env!("CARGO_BIN_EXE_underprint"))
        .args(["--json", "version"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../schemas/fixtures/version.json")).unwrap();
    assert_eq!(actual, expected);
    assert!(!output.stdout.contains(&0x1b));
}

#[test]
fn runtime_json_errors_are_stable_and_use_stderr() {
    let output = Command::new(env!("CARGO_BIN_EXE_underprint"))
        .args([
            "--json",
            "detect",
            "missing.png",
            "--models",
            "definitely-missing-models",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    let actual: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/fixtures/error-invalid-input.json"
    ))
    .unwrap();
    assert_eq!(actual, expected);
    assert!(!output.stderr.contains(&0x1b));
}
