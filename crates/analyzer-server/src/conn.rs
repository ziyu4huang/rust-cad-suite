//! 連線處理(ANALYZER-SERVICE §5.4-§5.7):握手、幀循環、ops 分派、
//! 錯誤幀、catch_unwind、優雅停機觸發。單連線內請求串行(幀序=響應序)。

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use analyzer_core::proto::frame;
use analyzer_core::proto::json::{CodecError, Json};
use analyzer_core::report;
use analyzer_core::{analyze, SourceInput, SvInput, RtlilInput};

use crate::ServerState;

pub(crate) const OPS: [&str; 4] = ["hello", "ping", "analyze", "shutdown"];

/// 一次幀讀取的結果。
enum ReadOutcome {
    /// 合法 JSON Obj 負載。
    Frame(Json),
    /// 協議違約:回錯誤幀(code, message)後關閉(§5.6)。
    Violation(&'static str, String),
    /// 對端關閉/連線斷裂,靜默退出。
    Eof,
}

pub(crate) fn serve(mut stream: UnixStream, st: Arc<ServerState>) {
    // 握手(§5.4):首幀必須 hello 且 args.proto==1,否則 E_VERSION 後關閉。
    match read_request(&mut stream, st.max_frame) {
        ReadOutcome::Eof => return,
        ReadOutcome::Violation(code, msg) => {
            let _ = write_frame_json(&mut stream, &err_envelope(0, code, &msg));
            return;
        }
        ReadOutcome::Frame(v) => {
            match handshake(&v) {
                Ok(id) => {
                    let result = Json::Obj(vec![
                        ("proto".to_string(), Json::Int(1)),
                        ("server".to_string(), Json::Str("analyzerd".to_string())),
                        (
                            "version".to_string(),
                            Json::Str(env!("CARGO_PKG_VERSION").to_string()),
                        ),
                        (
                            "ops".to_string(),
                            Json::Arr(OPS.iter().map(|o| Json::Str(o.to_string())).collect()),
                        ),
                    ]);
                    if write_frame_json(&mut stream, &ok_envelope(id, result)).is_err() {
                        return;
                    }
                }
                Err((id, msg)) => {
                    let _ = write_frame_json(&mut stream, &err_envelope(id, "E_VERSION", &msg));
                    return;
                }
            }
        }
    }

    // 請求循環:幀序=響應序;E_PROTO/E_CODEC 類違約=關閉,E_OP/E_ARGS/E_SHUTDOWN
    // 類=連線保持(§5.6)。
    loop {
        let v = match read_request(&mut stream, st.max_frame) {
            ReadOutcome::Eof => return,
            ReadOutcome::Violation(code, msg) => {
                let _ = write_frame_json(&mut stream, &err_envelope(0, code, &msg));
                return;
            }
            ReadOutcome::Frame(v) => v,
        };

        let (id, op) = envelope_parts(&v);
        // 停機 drain:既有連線僅 shutdown/ping 豁免,其餘 E_SHUTDOWN(§5.6)。
        if st.shutdown.load(Ordering::SeqCst) && op.as_deref() != Some("shutdown") && op.as_deref() != Some("ping")
        {
            let _ = write_frame_json(
                &mut stream,
                &err_envelope(id, "E_SHUTDOWN", "server is shutting down"),
            );
            continue;
        }

        let response = std::panic::catch_unwind(AssertUnwindSafe(|| dispatch(&v, &st)))
            .unwrap_or_else(|_| (false, err_envelope(id, "E_INTERNAL", "request handler panicked")));

        if write_frame_json(&mut stream, &response.1).is_err() {
            return;
        }
        if response.0 {
            // shutdown op:ack 已 flush(§5.7①),接 ②unlink ③置位。
            let _ = std::fs::remove_file(&st.sock_path);
            st.shutdown.store(true, Ordering::SeqCst);
            return;
        }
    }
}

/// 讀一幀:前綴即檢查(先於分配,§5.1)→ 讀負載 → UTF-8 → 嚴格 JSON 子集
/// → 須為 Obj(§5.6 E_PROTO)。
fn read_request(stream: &mut UnixStream, max_frame: u32) -> ReadOutcome {
    let mut header = [0u8; frame::HEADER_LEN];
    if let Err(e) = stream.read_exact(&mut header) {
        let _ = e; // UnexpectedEof/中斷都按對端關閉處理
        return ReadOutcome::Eof;
    }
    let len = frame::header_len(&header);
    if let Err(e) = frame::check_len(len, max_frame) {
        // 超限幀不讀負載不分配(§5.1)。
        return ReadOutcome::Violation("E_PROTO", e.to_string());
    }
    let mut payload = vec![0u8; len as usize]; // check_len 之後才分配
    if stream.read_exact(&mut payload).is_err() {
        return ReadOutcome::Eof;
    }
    let text = match String::from_utf8(payload) {
        Ok(t) => t,
        Err(_) => return ReadOutcome::Violation("E_PROTO", "payload is not valid UTF-8".to_string()),
    };
    match Json::parse(&text) {
        Ok(v @ Json::Obj(_)) => ReadOutcome::Frame(v),
        Ok(_) => ReadOutcome::Violation("E_PROTO", "payload is not a JSON object".to_string()),
        Err(CodecError { msg }) => ReadOutcome::Violation("E_CODEC", msg),
    }
}

/// 握手核驗:首幀 op=hello 且 args.proto==1 → Ok(id);否則 Err((id, message))。
fn handshake(v: &Json) -> Result<i64, (i64, String)> {
    let Json::Obj(entries) = v else { unreachable!("read_request guarantees Obj") };
    let id = envelope_id(entries);
    let op = entries.iter().find(|(k, _)| k == "op").and_then(|(_, v)| match v {
        Json::Str(s) => Some(s.as_str()),
        _ => None,
    });
    if op != Some("hello") {
        return Err((id, "first frame on a connection must be op=hello".to_string()));
    }
    let proto = entries
        .iter()
        .find(|(k, _)| k == "args")
        .and_then(|(_, v)| match v {
            Json::Obj(a) => a.iter().find(|(k, _)| k == "proto").map(|(_, pv)| pv),
            _ => None,
        });
    match proto {
        Some(Json::Int(1)) => Ok(id),
        Some(Json::Int(n)) => Err((id, format!("unsupported protocol version {n} (want 1)"))),
        _ => Err((id, "hello requires args.proto == 1".to_string())),
    }
}

/// (id, op):id=非零 Int 否則 0;op=Str 否則 None。未知/壞信封走 E_ARGS
/// (連線保持,裁定記錄於 §17 S3)。
fn envelope_parts(v: &Json) -> (i64, Option<String>) {
    let Json::Obj(entries) = v else { unreachable!("read_request guarantees Obj") };
    (envelope_id(entries), op_of(entries))
}

fn envelope_id(entries: &[(String, Json)]) -> i64 {
    match entries.iter().find(|(k, _)| k == "id") {
        Some((_, Json::Int(n))) if *n != 0 => *n,
        _ => 0,
    }
}

fn op_of(entries: &[(String, Json)]) -> Option<String> {
    match entries.iter().find(|(k, _)| k == "op") {
        Some((_, Json::Str(s))) => Some(s.clone()),
        _ => None,
    }
}

fn args_of(entries: &[(String, Json)]) -> Option<&Vec<(String, Json)>> {
    match entries.iter().find(|(k, _)| k == "args") {
        Some((_, Json::Obj(a))) => Some(a),
        _ => None,
    }
}

/// ops 分派(§5.5):返回 (is_shutdown, 響應幀)。未知 op=E_OP(連線不斷)。
fn dispatch(v: &Json, st: &Arc<ServerState>) -> (bool, Json) {
    let (id, op) = envelope_parts(v);
    let Json::Obj(entries) = v else { unreachable!("read_request guarantees Obj") };
    let Some(op) = op else {
        return (false, err_envelope(id, "E_ARGS", "request requires a string `op`"));
    };
    let Some(args) = args_of(entries) else {
        return (false, err_envelope(id, "E_ARGS", "request requires an object `args`"));
    };
    match op.as_str() {
        "hello" => (
            false,
            ok_envelope(
                id,
                Json::Obj(vec![
                    ("proto".to_string(), Json::Int(1)),
                    ("server".to_string(), Json::Str("analyzerd".to_string())),
                    (
                        "version".to_string(),
                        Json::Str(env!("CARGO_PKG_VERSION").to_string()),
                    ),
                    (
                        "ops".to_string(),
                        Json::Arr(OPS.iter().map(|o| Json::Str(o.to_string())).collect()),
                    ),
                ]),
            ),
        ),
        "ping" => (
            false,
            ok_envelope(
                id,
                Json::Obj(vec![
                    ("pong".to_string(), Json::Bool(true)),
                    (
                        "uptime_ms".to_string(),
                        Json::Int(st.start.elapsed().as_millis().min(i64::MAX as u128) as i64),
                    ),
                ]),
            ),
        ),
        "analyze" => match op_analyze(args) {
            Ok(result) => (false, ok_envelope(id, result)),
            Err((code, msg)) => (false, err_envelope(id, &code, &msg)),
        },
        "shutdown" => (true, ok_envelope(id, Json::Obj(vec![("bye".to_string(), Json::Bool(true))]))),
        _ => (
            false,
            err_envelope(id, "E_OP", &format!("unknown op `{op}` (ops: {})", OPS.join(","))),
        ),
    }
}

/// analyze op(§5.5/§13):SV/RTLIL 雙入口;include_dirs/defines 僅 SV 受理。
fn op_analyze(args: &[(String, Json)]) -> Result<Json, (String, String)> {
    let eargs = |msg: String| ("E_ARGS".to_string(), msg);
    let get_str = |key: &str| -> Result<String, (String, String)> {
        match args.iter().find(|(k, _)| k == key) {
            Some((_, Json::Str(s))) => Ok(s.clone()),
            Some((_, other)) => Err(eargs(format!("`{key}` must be a string, got {other:?}"))),
            None => Err(eargs(format!("analyze requires `{key}` (string)"))),
        }
    };
    let get_str_arr = |key: &str| -> Result<Option<Vec<String>>, (String, String)> {
        match args.iter().find(|(k, _)| k == key) {
            None => Ok(None),
            Some((_, Json::Arr(items))) => {
                let mut out = Vec::with_capacity(items.len());
                for it in items {
                    match it {
                        Json::Str(s) => out.push(s.clone()),
                        other => {
                            return Err(eargs(format!(
                                "`{key}` entries must be strings, got {other:?}"
                            )))
                        }
                    }
                }
                Ok(Some(out))
            }
            Some((_, other)) => Err(eargs(format!("`{key}` must be an array, got {other:?}"))),
        }
    };

    let kind = get_str("kind")?;
    let name = get_str("name")?;
    let source = get_str("source")?;
    let include_dirs = get_str_arr("include_dirs")?;
    let defines = get_str_arr("defines")?;
    let analysis = match kind.as_str() {
        "sv" => {
            let input = SvInput {
                name,
                source,
                include_dirs: include_dirs.unwrap_or_default().into_iter().map(std::path::PathBuf::from).collect(),
                defines: defines.unwrap_or_default(),
            };
            analyze(&SourceInput::Sv(input))
        }
        "rtlil" => {
            if include_dirs.is_some() || defines.is_some() {
                return Err(eargs(
                    "include_dirs/defines apply to kind=sv only (rtlil carries none)".to_string(),
                ));
            }
            let input = RtlilInput { name, source };
            analyze(&SourceInput::Rtlil(input))
        }
        other => {
            return Err(eargs(format!(
                "`kind` must be \"sv\" or \"rtlil\", got \"{other}\""
            )))
        }
    };
    Ok(report::analysis_result(&analysis))
}

fn ok_envelope(id: i64, result: Json) -> Json {
    Json::Obj(vec![
        ("id".to_string(), Json::Int(id)),
        ("ok".to_string(), Json::Bool(true)),
        ("result".to_string(), result),
    ])
}

fn err_envelope(id: i64, code: &str, message: &str) -> Json {
    Json::Obj(vec![
        ("id".to_string(), Json::Int(id)),
        ("ok".to_string(), Json::Bool(false)),
        (
            "error".to_string(),
            Json::Obj(vec![
                ("code".to_string(), Json::Str(code.to_string())),
                ("message".to_string(), Json::Str(message.to_string())),
            ]),
        ),
    ])
}

/// 編碼並寫出一幀(單次 write_all;§6.2 對稱律的 server 側)。
fn write_frame_json(stream: &mut UnixStream, v: &Json) -> std::io::Result<()> {
    let mut buf = Vec::new();
    frame::write_frame(v.render().as_bytes(), &mut buf);
    stream.write_all(&buf)?;
    stream.flush()
}
