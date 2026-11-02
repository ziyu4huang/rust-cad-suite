//! Rust 真回環集成測試(ANALYZER-SERVICE §8.1 loopback.rs 行):spawn
//! analyzerd → std UnixStream 客戶端 → hello/ping/analyze(SV+RTLIL)/E_OP/
//! oversize 斷連/stale socket 重綁/活實例衝突 exit 1/路徑超長 exit 1/
//! shutdown 後 socket 文件消失+進程 exit 0(child.wait() 實證,R1-5;
//! smoke.ts 拿不到 PID,該斷言移址至此)。
//! 每測自建 tmpdir+唯一短路徑(R1-13:cargo test 並行,固定路徑互踩)。

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use analyzer_core::proto::frame;
use analyzer_core::proto::json::Json;

// ---- 測試腳手架 ----

struct Guard(Child, PathBuf);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
        // tmpdir 清理(冪等;shutdown 測試自行 remove 後此處再調=無害)。
        let _ = std::fs::remove_dir_all(&self.1);
    }
}

/// tmpdir 下唯一短路徑(<100 字節斷言,R1-13/§5.8)。
fn tmp_sock(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!(
        "analyzerd-lb-{}-{tag}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("a.sock");
    assert!(
        p.to_string_lossy().len() < 100,
        "test socket path must stay under 100 bytes"
    );
    p
}

fn spawn_server(sock: &Path) -> Guard {
    // stderr 暫存 tmpdir(診斷用;測試斷言不消費)。
    let dir = sock.parent().unwrap().to_path_buf();
    let err_log = std::fs::File::create(dir.join("server.err.log")).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_analyzerd"))
        .arg("--socket")
        .arg(sock)
        .stdout(Stdio::null())
        .stderr(err_log)
        .spawn()
        .unwrap();
    Guard(child, dir)
}

fn wait_socket(sock: &Path) -> UnixStream {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(c) = UnixStream::connect(sock) {
            return c;
        }
        assert!(Instant::now() < deadline, "server socket never appeared");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn obj(pairs: Vec<(&str, Json)>) -> Json {
    Json::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn s(v: &str) -> Json {
    Json::Str(v.to_string())
}

fn send(stream: &mut UnixStream, v: &Json) {
    let mut buf = Vec::new();
    frame::write_frame(v.render().as_bytes(), &mut buf);
    stream.write_all(&buf).unwrap();
}

/// 讀一幀;對端關閉 → None。
fn recv(stream: &mut UnixStream) -> Option<Json> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).ok()?;
    let len = frame::header_len(&header);
    let mut payload = vec![0u8; len as usize];
    stream.read_exact(&mut payload).ok()?;
    Json::parse(&String::from_utf8(payload).ok()?).ok()
}

fn field<'a>(obj: &'a Json, key: &str) -> &'a Json {
    let Json::Obj(entries) = obj else { panic!("not an object: {obj:?}") };
    &entries.iter().find(|(k, _)| k == key).unwrap_or_else(|| panic!("missing `{key}` in {obj:?}")).1
}

fn error_code(v: &Json) -> String {
    let err = field(v, "error");
    let Json::Str(s) = field(err, "code") else { panic!("error.code not a string") };
    s.clone()
}

/// 非恐慌形:成功幀(None)或錯誤幀(Some(code))。
fn try_error_code(v: &Json) -> Option<String> {
    let Json::Obj(entries) = v else { return None };
    let err = &entries.iter().find(|(k, _)| k == "error")?.1;
    let Json::Obj(ee) = err else { return None };
    match ee.iter().find(|(k, _)| k == "code") {
        Some((_, Json::Str(s))) => Some(s.clone()),
        _ => None,
    }
}

fn run_cli(args: &[&str]) -> (ExitStatus, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_analyzerd"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status,
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

// ---- 回環測試 ----

#[test]
fn hello_ping_analyze_e_op_e_args() {
    let sock = tmp_sock("basic");
    let mut srv = spawn_server(&sock);
    let mut c = wait_socket(&sock);

    // hello(握手,§5.4)
    send(&mut c, &obj(vec![
        ("id", Json::Int(1)),
        ("op", s("hello")),
        ("args", obj(vec![("proto", Json::Int(1))])),
    ]));
    let resp = recv(&mut c).expect("hello response");
    assert_eq!(field(&resp, "id"), &Json::Int(1));
    assert_eq!(field(&resp, "ok"), &Json::Bool(true));
    let result = field(&resp, "result");
    assert_eq!(field(result, "proto"), &Json::Int(1));
    assert_eq!(field(result, "server"), &s("analyzerd"));
    let Json::Arr(ops) = field(result, "ops") else { panic!("ops not array") };
    assert_eq!(ops.len(), 4);

    // ping(§5.5)
    send(&mut c, &obj(vec![("id", Json::Int(2)), ("op", s("ping")), ("args", Json::Obj(vec![]))]));
    let resp = recv(&mut c).expect("pong");
    let result = field(&resp, "result");
    assert_eq!(field(result, "pong"), &Json::Bool(true));
    assert!(matches!(field(result, "uptime_ms"), Json::Int(_)));

    // analyze SV(§5.5)
    send(&mut c, &obj(vec![
        ("id", Json::Int(3)),
        ("op", s("analyze")),
        ("args", obj(vec![
            ("kind", s("sv")),
            ("name", s("t.v")),
            ("source", s("module t(input a, b);\nassign a = b;\nendmodule\n")),
        ])),
    ]));
    let resp = recv(&mut c).expect("analyze sv");
    let result = field(&resp, "result");
    assert_eq!(field(result, "has_errors"), &Json::Bool(false));
    let Json::Arr(modules) = field(result, "modules") else { panic!("modules not array") };
    assert_eq!(modules.len(), 1);
    assert_eq!(field(&modules[0], "name"), &s("t"));
    let Json::Arr(ports) = field(&modules[0], "ports") else { panic!("ports not array") };
    assert_eq!(field(&ports[0], "name"), &s("a"));
    assert_eq!(field(&ports[0], "dir"), &s("input"));
    assert_eq!(field(&ports[0], "width"), &Json::Int(1));

    // analyze RTLIL
    send(&mut c, &obj(vec![
        ("id", Json::Int(4)),
        ("op", s("analyze")),
        ("args", obj(vec![
            ("kind", s("rtlil")),
            ("name", s("top.il")),
            ("source", s("module \\top\n  wire width 1 \\a\n  connect \\a \\a\nend\n")),
        ])),
    ]));
    let resp = recv(&mut c).expect("analyze rtlil");
    let result = field(&resp, "result");
    assert_eq!(field(result, "has_errors"), &Json::Bool(false));
    let Json::Arr(modules) = field(result, "modules") else { panic!("modules not array") };
    assert_eq!(field(&modules[0], "name"), &s("\\top"));

    // 未知 op → E_OP,連線保持(§5.6)
    send(&mut c, &obj(vec![("id", Json::Int(5)), ("op", s("frobnicate")), ("args", Json::Obj(vec![]))]));
    let resp = recv(&mut c).expect("E_OP frame");
    assert_eq!(field(&resp, "ok"), &Json::Bool(false));
    assert_eq!(error_code(&resp), "E_OP");
    // 連線可續用
    send(&mut c, &obj(vec![("id", Json::Int(6)), ("op", s("ping")), ("args", Json::Obj(vec![]))]));
    assert_eq!(field(&recv(&mut c).expect("pong after E_OP"), "ok"), &Json::Bool(true));

    // analyze 缺 source → E_ARGS;RTLIL 帶 defines → E_ARGS
    send(&mut c, &obj(vec![
        ("id", Json::Int(7)), ("op", s("analyze")),
        ("args", obj(vec![("kind", s("sv")), ("name", s("x"))])),
    ]));
    assert_eq!(error_code(&recv(&mut c).expect("E_ARGS missing source")), "E_ARGS");
    send(&mut c, &obj(vec![
        ("id", Json::Int(8)), ("op", s("analyze")),
        ("args", obj(vec![
            ("kind", s("rtlil")), ("name", s("x")), ("source", s("")), ("defines", Json::Arr(vec![])),
        ])),
    ]));
    assert_eq!(error_code(&recv(&mut c).expect("E_ARGS rtlil defines")), "E_ARGS");

    drop(c);
    drop(srv); // Guard Drop=kill+wait(無 shutdown 觸發的 server 不會自行退出)
}

#[test]
fn oversize_frame_disconnects_and_server_survives() {
    let sock = tmp_sock("oversize");
    let mut srv = spawn_server(&sock);
    let mut c = wait_socket(&sock);
    send(&mut c, &obj(vec![("id", Json::Int(1)), ("op", s("hello")), ("args", obj(vec![("proto", Json::Int(1))]))]));
    assert_eq!(field(&recv(&mut c).unwrap(), "ok"), &Json::Bool(true));

    // 超限前綴(不讀負載不分配,§5.1)→ E_PROTO 錯誤幀後斷連
    send_raw_frame(&mut c, &[0xff, 0xff, 0xff, 0xff], &[0xde, 0xad, 0xbe, 0xef]);
    let resp = recv(&mut c).expect("E_PROTO frame for oversize");
    assert_eq!(error_code(&resp), "E_PROTO");
    assert!(recv(&mut c).is_none(), "connection closed after violation");

    // server 存活,新連線續接
    drop(c);
    let mut c2 = wait_socket(&sock);
    send(&mut c2, &obj(vec![("id", Json::Int(9)), ("op", s("hello")), ("args", obj(vec![("proto", Json::Int(1))]))]));
    assert_eq!(field(&recv(&mut c2).unwrap(), "ok"), &Json::Bool(true));
    drop(c2);
    drop(srv); // Guard Drop=kill+wait
}

fn send_raw_frame(c: &mut UnixStream, header: &[u8], payload: &[u8]) {
    let mut bytes = header.to_vec();
    bytes.extend_from_slice(payload);
    c.write_all(&bytes).unwrap();
}

#[test]
fn stale_socket_rebind() {
    let sock = tmp_sock("stale");
    {
        let mut srv = spawn_server(&sock);
        let mut c = wait_socket(&sock);
        send(&mut c, &obj(vec![
            ("id", Json::Int(1)), ("op", s("hello")), ("args", obj(vec![("proto", Json::Int(1))])),
        ]));
        assert_eq!(field(&recv(&mut c).unwrap(), "ok"), &Json::Bool(true));
        // SIGKILL 硬殺:socket 文件殘留(§5.7 SIGTERM 邊界/§5.8 啟動清理兜底)。
        srv.0.kill().unwrap();
        srv.0.wait().unwrap();
        drop(c);
        assert!(sock.exists(), "killed server leaves the socket file behind");
        // 殘留檔案正是本測試的受試物,不讓 Guard 的 tmpdir 清理收走它。
        std::mem::forget(srv);
    }
    let mut srv2 = spawn_server(&sock);
    let mut c = wait_socket(&sock);
    send(&mut c, &obj(vec![("id", Json::Int(1)), ("op", s("hello")), ("args", obj(vec![("proto", Json::Int(1))]))]));
    assert_eq!(field(&recv(&mut c).unwrap(), "ok"), &Json::Bool(true), "stale socket re-bound");
    drop(c);
    drop(srv2); // Guard Drop=kill+wait
}

#[test]
fn live_instance_conflict_exit_1() {
    let sock = tmp_sock("conflict");
    let mut srv = spawn_server(&sock);
    let mut c = wait_socket(&sock); // 等就緒
    send(&mut c, &obj(vec![
        ("id", Json::Int(1)), ("op", s("hello")), ("args", obj(vec![("proto", Json::Int(1))])),
    ]));
    assert_eq!(field(&recv(&mut c).unwrap(), "ok"), &Json::Bool(true));
    drop(c);

    let second = Command::new(env!("CARGO_BIN_EXE_analyzerd"))
        .arg("--socket")
        .arg(&sock)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    assert_eq!(second.code(), Some(1), "second instance on live socket must exit 1");
    drop(srv);
}

#[test]
fn socket_path_too_long_exit_1() {
    let long = std::env::temp_dir().join(format!("{}{}", "p".repeat(60), "analyzerd.sock"));
    assert!(long.to_string_lossy().len() >= 100);
    let (status, _out, err_out) = run_cli(&["--socket", &long.to_string_lossy()]);
    assert_eq!(status.code(), Some(1), "path >=100 bytes must exit 1");
    assert!(err_out.contains("too long"), "stderr: {err_out}");
}

#[test]
fn shutdown_socket_file_gone_and_exit_0() {
    let sock = tmp_sock("shutdown");
    let mut srv = spawn_server(&sock);
    let mut c = wait_socket(&sock);
    send(&mut c, &obj(vec![("id", Json::Int(1)), ("op", s("hello")), ("args", obj(vec![("proto", Json::Int(1))]))]));
    assert_eq!(field(&recv(&mut c).unwrap(), "ok"), &Json::Bool(true));

    send(&mut c, &obj(vec![("id", Json::Int(2)), ("op", s("shutdown")), ("args", Json::Obj(vec![]))]));
    let resp = recv(&mut c).expect("bye ack");
    let result = field(&resp, "result");
    assert_eq!(field(result, "bye"), &Json::Bool(true));
    // ack 後 socket 文件消失(§6.5-7;進程退出碼在此處斷言,R1-5)。
    let deadline = Instant::now() + Duration::from_secs(5);
    while sock.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!sock.exists(), "socket file must be unlinked during shutdown");

    // shutdown 幀 ack 後服務端關閉連線;進程 exit 0(child.wait() 實證)。
    drop(c);
    let status = srv.0.wait().unwrap();
    assert_eq!(status.code(), Some(0), "graceful shutdown must exit 0");
    let _ = std::fs::remove_dir_all(sock.parent().unwrap());
}

#[test]
fn first_frame_not_hello_is_e_version_and_close() {
    let sock = tmp_sock("eversion");
    let mut srv = spawn_server(&sock);
    let mut c = wait_socket(&sock);
    // 首幀 ping(非 hello)→ E_VERSION 後關閉(§5.4)
    send(&mut c, &obj(vec![("id", Json::Int(1)), ("op", s("ping")), ("args", Json::Obj(vec![]))]));
    let resp = recv(&mut c).expect("E_VERSION frame");
    assert_eq!(error_code(&resp), "E_VERSION");
    assert!(recv(&mut c).is_none(), "connection closed after E_VERSION");
    // proto 不支持同樣 E_VERSION
    let mut c2 = wait_socket(&sock);
    send(&mut c2, &obj(vec![
        ("id", Json::Int(1)), ("op", s("hello")), ("args", obj(vec![("proto", Json::Int(99))])),
    ]));
    assert_eq!(error_code(&recv(&mut c2).expect("E_VERSION proto").clone(), ), "E_VERSION");
    drop(c2);
    drop(c);
    drop(srv); // Guard Drop=kill+wait
}

#[test]
fn frame_violations_close_connection() {
    let sock = tmp_sock("violations");
    let mut srv = spawn_server(&sock);

    // (code, 原始幀字節):空 len / 非 JSON Obj / 浮點(§5.2)/ 非 UTF-8
    let payload_obj = |payload: &[u8]| {
        let mut b = (payload.len() as u32).to_be_bytes().to_vec();
        b.extend_from_slice(payload);
        b
    };
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("E_PROTO", 0u32.to_be_bytes().to_vec()), // len=0
        ("E_PROTO", payload_obj(b"[]")),          // 非 Obj
        ("E_CODEC", payload_obj(b"{\"x\":1.5}")), // 浮點禁項
        ("E_PROTO", {
            let mut p = b"{\"s\":\"".to_vec();
            p.push(0xff);
            p.push(0xfe);
            p.extend_from_slice(b"\"}");
            payload_obj(&p)
        }),
    ];
    for (code, bytes) in cases {
        let mut c = wait_socket(&sock);
        c.write_all(&bytes).unwrap();
        let resp = recv(&mut c).expect("violation error frame");
        assert_eq!(error_code(&resp), code);
        assert!(recv(&mut c).is_none(), "connection must close after {code}");
        drop(c);
    }
    drop(srv);
}

#[test]
fn shutdown_drain_e_shutdown_new_conn_eof_then_exit_0() {
    let sock = tmp_sock("drain");
    let mut srv = spawn_server(&sock);
    let mut a = wait_socket(&sock);
    let mut b = wait_socket(&sock);
    let hello = obj(vec![
        ("id", Json::Int(1)),
        ("op", s("hello")),
        ("args", obj(vec![("proto", Json::Int(1))])),
    ]);
    send(&mut a, &hello);
    assert_eq!(field(&recv(&mut a).unwrap(), "ok"), &Json::Bool(true));
    send(&mut b, &hello);
    assert_eq!(field(&recv(&mut b).unwrap(), "ok"), &Json::Bool(true));

    // B 觸發停機:ack 後 unlink+SHUTDOWN 置位(§5.7①②③)
    send(&mut b, &obj(vec![("id", Json::Int(2)), ("op", s("shutdown")), ("args", Json::Obj(vec![]))]));
    assert_eq!(field(&field(&recv(&mut b).unwrap(), "result"), "bye"), &Json::Bool(true));
    drop(b);

    // 既有連線 A:analyze 拒 E_SHUTDOWN(SHUTDOWN 置位在 bye flush 之後,
    // A 的首發可能趕在前面 → 輪詢重試直至 E_SHUTDOWN;§5.6)
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut got_e_shutdown = false;
    let mut req_id = 3;
    while Instant::now() < deadline {
        send(&mut a, &obj(vec![
            ("id", Json::Int(req_id)), ("op", s("analyze")),
            ("args", obj(vec![("kind", s("sv")), ("name", s("x")), ("source", s("module x();\nendmodule\n"))])),
        ]));
        let resp = recv(&mut a).expect("analyze response during drain");
        if try_error_code(&resp).as_deref() == Some("E_SHUTDOWN") {
            got_e_shutdown = true;
            break;
        }
        req_id += 1;
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(got_e_shutdown, "analyze during drain must eventually get E_SHUTDOWN");
    send(&mut a, &obj(vec![("id", Json::Int(4)), ("op", s("ping")), ("args", Json::Obj(vec![]))]));
    assert_eq!(field(&field(&recv(&mut a).unwrap(), "result"), "pong"), &Json::Bool(true));

    // drain 窗口新 client:§5.7② unlink 先於 ③ SHUTDOWN 置位,故新 connect
    // **立即失敗**(ENOENT/ECONNREFUSED=「服務終止」的機械觀測面);§5.6 的
    // 「accept 後立即 close 不發幀」只覆蓋 unlink 時已在 backlog 的競態連線
    // (不發幀語義兩路一致:連接失敗或 0 字節 EOF,皆無幀歧義)。
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut service_gone = false;
    while Instant::now() < deadline {
        match UnixStream::connect(&sock) {
            Err(_) => {
                service_gone = true;
                break;
            }
            Ok(mut c) => {
                // unlink 前瞬間排入 backlog 的競態連線:被 accept 後立即 close
                // → 0 字節 EOF;未進 SHUTDOWN 的活連線 → 讀超時,重試。
                let _ = c.set_read_timeout(Some(Duration::from_millis(150)));
                let mut buf = [0u8; 8];
                match c.read(&mut buf) {
                    Ok(0) => {
                        service_gone = true;
                        break;
                    }
                    Ok(_) => panic!("drain-window new connection must not receive frames"),
                    Err(_) => {}
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(service_gone, "new client in drain window must see service terminated (connect fail or 0-byte EOF)");

    // A 關閉 → CONN==0 → 主循環退出 → exit 0 + socket 文件消失
    drop(a);
    let status = srv.0.wait().unwrap();
    assert_eq!(status.code(), Some(0));
    assert!(!sock.exists());
    let _ = std::fs::remove_dir_all(sock.parent().unwrap());
}

#[test]
fn cli_flag_contracts() {
    // --version(§5.9)
    let (status, out, _e) = run_cli(&["--version"]);
    assert_eq!(status.code(), Some(0));
    assert_eq!(out.trim(), format!("analyzerd {} (ASP/1)", env!("CARGO_PKG_VERSION")));
    // --help
    let (status, out, _e) = run_cli(&["--help"]);
    assert_eq!(status.code(), Some(0));
    assert!(out.contains("--socket") && out.contains("--max-frame"));
    // 未知旗標=硬錯 exit 1
    let (status, _o, err) = run_cli(&["--frobnicate"]);
    assert_eq!(status.code(), Some(1));
    assert!(err.contains("unknown flag"), "stderr: {err}");
    // --max-frame 非整數=exit 1
    let (status, _o, err) = run_cli(&["--max-frame", "abc"]);
    assert_eq!(status.code(), Some(1));
    assert!(err.contains("expects an integer"), "stderr: {err}");
    // --socket 缺參=exit 1
    let (status, _o, err) = run_cli(&["--socket"]);
    assert_eq!(status.code(), Some(1));
    assert!(err.contains("requires a PATH"), "stderr: {err}");
    // 綁定 IO 錯=exit 2(父目錄不存在)
    let (status, _o, _e) = run_cli(&["--socket", "/nonexistent-analyzerd-dir/a.sock"]);
    assert_eq!(status.code(), Some(2), "bind IO error must exit 2");
}

/// 握手(首幀必 hello,§5.4)+ ping,斷言 ok=true。
fn handshake_ping(c: &mut UnixStream, what: &str) {
    send(c, &obj(vec![
        ("id", Json::Int(1)),
        ("op", s("hello")),
        ("args", obj(vec![("proto", Json::Int(1))])),
    ]));
    let resp = recv(c).expect("hello response");
    assert_eq!(field(&resp, "ok"), &Json::Bool(true));
    send(c, &obj(vec![("id", Json::Int(2)), ("op", s("ping")), ("args", Json::Obj(vec![]))]));
    let resp = recv(c).expect(what);
    assert_eq!(field(&resp, "ok"), &Json::Bool(true));
}

/// socket 路徑優先級(§5.8/§5.9;收平輪 ② 回歸錨):真二進制+真環境變量。
/// 單獨 `--max-frame`(無 --socket)時 server 必落 `ANALYZER_SOCK`——
/// 修復前 `--max-frame` 在 parse_args 硬編碼 DEFAULT_SOCKET,run() 的
/// unwrap_or_else 不再查 env,env 被繞過(實錄 bug)。
#[test]
fn max_frame_alone_respects_analyzer_sock_env() {
    let env_sock = tmp_sock("prio-env");
    let dir = env_sock.parent().unwrap().to_path_buf();
    let err_log = std::fs::File::create(dir.join("prio-a.err.log")).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_analyzerd"))
        .arg("--max-frame")
        .arg("1024")
        .env("ANALYZER_SOCK", &env_sock)
        .stdout(Stdio::null())
        .stderr(err_log)
        .spawn()
        .unwrap();
    let mut g = Guard(child, dir);
    {
        let mut c = wait_socket(&env_sock);
        handshake_ping(&mut c, "pong via env path");
    }
    let _ = g.0.kill();
    let _ = g.0.wait();
}

/// 優先級正向錨(§5.9):`--socket` 與 `ANALYZER_SOCK` 同給 → CLI 勝,
/// env 路徑零產物。
#[test]
fn cli_socket_flag_beats_analyzer_sock_env() {
    let env_sock = tmp_sock("prio-env2");
    let cli_sock = tmp_sock("prio-cli2");
    let dir = cli_sock.parent().unwrap().to_path_buf();
    let err_log = std::fs::File::create(dir.join("prio-b.err.log")).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_analyzerd"))
        .arg("--socket")
        .arg(&cli_sock)
        .env("ANALYZER_SOCK", &env_sock)
        .stdout(Stdio::null())
        .stderr(err_log)
        .spawn()
        .unwrap();
    let mut g = Guard(child, dir);
    {
        let mut c = wait_socket(&cli_sock);
        handshake_ping(&mut c, "pong via cli path");
    }
    assert!(!env_sock.exists(), "CLI 勝後 env 路徑不得被綁定");
    let _ = g.0.kill();
    let _ = g.0.wait();
}
