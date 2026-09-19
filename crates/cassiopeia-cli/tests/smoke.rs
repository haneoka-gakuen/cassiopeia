use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[test]
fn emits_the_versioned_fixture_summary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO_BIN_EXE_cassiopeia-cli"))
        .args([
            "conformance",
            root.join("fixtures/conformance/basic.ccf.json")
                .to_str()
                .unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let expected = std::fs::read_to_string(root.join("fixtures/conformance/basic.summary.json"))
        .unwrap()
        .replace("\r\n", "\n");
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn rejects_an_invalid_chart_without_emitting_a_summary() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cassiopeia-cli"))
        .args(["conformance", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            br#"{
                "format":"org.haneoka.cassiopeia.chart",
                "version":2,
                "header":{"title":"","artist":"","author":"","ppq":480,"laneCount":8}
            }"#,
        )
        .unwrap();

    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("invalid chart: unsupported chart version: 2")
    );
}
