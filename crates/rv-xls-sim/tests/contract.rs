//! Contract tests for the XLS-facing adapter.
//!
//! These are the S2 acceptance gates from docs/plan/xls-shared-core.html:
//!   - the two harvested XLS samples reproduce their oracle-recorded stdout
//!     byte-for-byte through the adapter (text mode and argv mode);
//!   - macros actually change behaviour (not just accepted and ignored);
//!   - include directories resolve;
//!   - a syntax error lands on stderr with a nonzero exit;
//!   - `.sv` is refused explicitly (milestone, never a silent downgrade);
//!   - the JSON response round-trips, and no scratch path leaks into stdout.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rv_xls_sim::json::{self, Json};
use rv_xls_sim::{parse_argv, run, Request, Source};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn case_dir(name: &str) -> PathBuf {
    repo_root().join("corpus/e2e").join(name)
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// Design + testbench as one text, exactly how XLS hands its generated Verilog
/// to a `VerilogSimulator`.
fn case_text(name: &str, design: &str) -> String {
    let dir = case_dir(name);
    format!(
        "{}\n{}",
        read(&dir.join(design)),
        read(&dir.join("tb.v"))
    )
}

fn memory_request(text: &str) -> Request {
    Request {
        sources: vec![Source::memory("top.v", text)],
        ..Request::default()
    }
}

fn assert_oracle(case: &str, design: &str) {
    let request = memory_request(&case_text(case, design));
    let response = run(&request);
    let expected = read(&case_dir(case).join("expected.stdout"));
    assert!(
        response.ok,
        "{case}: adapter failed\nstdout:\n{}\nstderr:\n{}",
        response.stdout, response.stderr
    );
    assert_eq!(response.stdout, expected, "{case}: stdout must be byte-exact");
    assert!(
        !response.stdout.contains("rv-xls-sim-"),
        "{case}: scratch path leaked into stdout"
    );
}

#[test]
fn xls_add3_matches_the_recorded_expectation() {
    assert_oracle("57_xls_add3", "add3.v");
}

#[test]
fn xls_acc_pipe_matches_the_recorded_expectation() {
    assert_oracle("58_xls_acc_pipe", "acc_pipe.v");
}

#[test]
fn argv_mode_drives_the_same_core() {
    let dir = case_dir("57_xls_add3");
    let args = vec![
        dir.join("add3.v").display().to_string(),
        dir.join("tb.v").display().to_string(),
    ];
    let request = parse_argv(&args).expect("argv parses");
    let response = run(&request);
    assert!(response.ok, "argv run failed: {}", response.stderr);
    assert_eq!(response.stdout, read(&dir.join("expected.stdout")));
}

#[test]
fn top_module_can_be_forced() {
    let dir = case_dir("57_xls_add3");
    let args = vec![
        "--top-module".to_string(),
        "tb".to_string(),
        dir.join("add3.v").display().to_string(),
        dir.join("tb.v").display().to_string(),
    ];
    let response = run(&parse_argv(&args).expect("argv parses"));
    assert!(response.ok, "forced top failed: {}", response.stderr);
}

#[test]
fn macros_change_behaviour() {
    let text = "`ifdef BIG\n\
                module tb; initial begin $display(\"big\"); $finish; end endmodule\n\
                `else\n\
                module tb; initial begin $display(\"small\"); $finish; end endmodule\n\
                `endif\n";
    let plain = run(&memory_request(text));
    assert!(plain.ok, "plain run failed: {}", plain.stderr);
    assert_eq!(plain.stdout, "small\n");

    let mut defined = memory_request(text);
    defined.defines.push("BIG".to_string());
    let with_macro = run(&defined);
    assert!(with_macro.ok, "macro run failed: {}", with_macro.stderr);
    assert_eq!(with_macro.stdout, "big\n", "`ifdef must actually see -D BIG");
}

#[test]
fn include_dirs_resolve() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "rv-xls-sim-test-include-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("create include dir");
    fs::write(dir.join("defs.vh"), "`define VAL 7\n").expect("write include");

    let mut request = memory_request(
        "`include \"defs.vh\"\n\
         module tb; initial begin $display(\"val=%0d\", `VAL); $finish; end endmodule\n",
    );
    request.includes.push(dir.clone());
    let response = run(&request);
    let _ = fs::remove_dir_all(&dir);

    assert!(response.ok, "include run failed: {}", response.stderr);
    assert_eq!(response.stdout, "val=7\n");
}

#[test]
fn syntax_check_is_quiet_on_success() {
    let dir = case_dir("57_xls_add3");
    let mut request = parse_argv(&[
        "--syntax-check".to_string(),
        dir.join("add3.v").display().to_string(),
        dir.join("tb.v").display().to_string(),
    ])
    .expect("argv parses");
    let response = run(&request);
    assert!(response.ok, "syntax check failed: {}", response.stderr);
    assert_eq!(response.stdout, "", "syntax check must not print %Info to stdout");
    assert_eq!(response.stderr, "");

    request.sources = vec![Source::memory("bad.v", "module tb(\n")];
    let broken = run(&request);
    assert!(!broken.ok, "broken design must fail the syntax check");
    assert_eq!(broken.exit, 1);
    assert!(broken.stderr.contains("during parsing"), "{}", broken.stderr);
}

#[test]
fn parse_error_goes_to_stderr() {
    let response = run(&memory_request("module tb(\n  input wire a\n"));
    assert!(!response.ok);
    assert_eq!(response.exit, 1);
    assert_eq!(response.stdout, "", "diagnostics must not leak onto stdout");
    assert!(response.stderr.contains("during parsing"), "{}", response.stderr);
}

#[test]
fn systemverilog_is_refused_explicitly() {
    let mut request = memory_request("module tb; endmodule\n");
    request.file_type = ".sv".to_string();
    let response = run(&request);
    assert!(!response.ok, ".sv must not be accepted");
    assert_eq!(response.exit, 1);
    assert!(response.stderr.contains("SystemVerilog"), "{}", response.stderr);
    assert!(response.stderr.contains("milestone"), "{}", response.stderr);
}

#[test]
fn json_response_round_trips() {
    let request = Request::from_json(
        &json::parse(
            r#"{"text": "module tb; initial begin $display(\"q=%0d\", 8); $finish; end endmodule\n"}"#,
        )
        .expect("valid json"),
    )
    .expect("valid request");
    let response = run(&request);
    let encoded = response.to_json();
    let parsed = json::parse(&encoded).expect("response is valid json");
    assert_eq!(parsed.get("status").and_then(Json::as_str), Some("ok"));
    assert_eq!(parsed.get("stdout").and_then(Json::as_str), Some("q=8\n"));
    assert_eq!(parsed.get("stderr").and_then(Json::as_str), Some(""));
    assert_eq!(parsed.get("exit"), Some(&Json::Number(0.0)));
}

#[test]
fn failure_response_keeps_streams_separate() {
    let response = run(&memory_request("module tb(\n"));
    assert!(!response.ok);
    let encoded = response.to_json();
    let parsed = json::parse(&encoded).expect("failure response is valid json");
    assert_eq!(parsed.get("status").and_then(Json::as_str), Some("error"));
    assert_eq!(parsed.get("stdout").and_then(Json::as_str), Some(""));
    assert_eq!(parsed.get("exit"), Some(&Json::Number(1.0)));
}
