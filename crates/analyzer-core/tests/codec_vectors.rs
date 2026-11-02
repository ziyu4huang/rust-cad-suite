//! TS↔Rust 編解碼一致性(ANALYZER-SERVICE §6.3,D7):TS 側
//! `clients/analyzer-ts/test/codec.test.ts` 與本測試讀**同一**向量文件,
//! 對正例斷言 canonical 寫出逐字節相等+回讀相等,對敵意例雙端同判「拒」。
//!
//! 向量文件是自建測試向量(新基礎設施無 oracle,非虛構 golden):由本測試
//! 內的生成器(`regen_fixtures`,#[ignore] 手動跑)以 canonical 寫律首錄,
//! 雙端斷言維持。TS 側實現=`src/codec.ts`(與 Rust 同律,禁 JSON.parse/
//! stringify/非 fatal UTF-8,§6.2)。

use analyzer_core::proto::frame::{self, HEADER_LEN, MAX_FRAME_DEFAULT};
use analyzer_core::proto::json::Json;

/// 跨目錄 include:manifest-dir 形(§6.3;CARGO_MANIFEST_DIR=crates/analyzer-core,
/// ../..=repo 根)。
const VECTORS_PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../clients/analyzer-ts/fixtures/codec-vectors.json");
const VECTORS_TEXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../clients/analyzer-ts/fixtures/codec-vectors.json"
));

/// analyze 響應向量對應的源(生成器與穩定性測試共用)。
const ANALYZE_VECTOR_SOURCE: &str = "module t(input a);\n  assign a = 1'b0;\nendmodule\n";

fn hex_decode(h: &str) -> Vec<u8> {
    let clean: String = h.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(clean.len() % 2, 0, "hex must be even-length");
    (0..clean.len() / 2)
        .map(|i| u8::from_str_radix(&clean[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

fn hex_encode(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn frame_hex_of(payload: &str) -> String {
    let mut buf = Vec::new();
    frame::write_frame(payload.as_bytes(), &mut buf);
    hex_encode(&buf)
}

fn frame_hex_of_bytes(payload: &[u8]) -> String {
    let mut buf = Vec::new();
    frame::write_frame(payload, &mut buf);
    hex_encode(&buf)
}

/// 讀取向量清單(用自家嚴格 reader 解析 fixtures 本身=dogfood)。
fn load_vectors() -> Vec<(String, String, String, String)> {
    let root = Json::parse(VECTORS_TEXT).expect("fixtures file must parse with own strict reader");
    let Json::Obj(entries) = &root else {
        panic!("fixtures root must be an object");
    };
    let Some((_, Json::Arr(items))) = entries.iter().find(|(k, _)| k == "vectors") else {
        panic!("vectors key missing or not an array");
    };
    items
        .iter()
        .map(|v| {
            let Json::Obj(fields) = v else { panic!("vector entry must be object") };
            let get = |key: &str| match fields.iter().find(|(k, _)| k == key) {
                Some((_, Json::Str(s))) => s.clone(),
                other => panic!("field {key} must be string, got {other:?}"),
            };
            (get("name"), get("kind"), get("frame_hex"), get("note"))
        })
        .collect()
}

#[test]
fn vectors_rust_side_byte_exact() {
    let vectors = load_vectors();
    assert!(
        vectors.len() >= 12,
        "§6.3 要求向量 ≥12 條,實際 {}",
        vectors.len()
    );
    for (name, kind, frame_hex, _note) in &vectors {
        let bytes = hex_decode(frame_hex);
        assert!(bytes.len() >= HEADER_LEN, "vector {name}: frame shorter than header");
        let (head, rest) = bytes.split_first_chunk::<4>().unwrap();
        let len = frame::header_len(head);
        match kind.as_str() {
            "canonical" => {
                frame::check_len(len, MAX_FRAME_DEFAULT)
                    .unwrap_or_else(|e| panic!("vector {name}: {e}"));
                assert_eq!(len as usize, rest.len(), "vector {name}: len prefix mismatch");
                let text = std::str::from_utf8(rest)
                    .unwrap_or_else(|e| panic!("vector {name}: payload not utf8: {e}"));
                let parsed =
                    Json::parse(text).unwrap_or_else(|e| panic!("vector {name}: parse: {e}"));
                // 寫出逐字節相等(canonical 形回放)。
                assert_eq!(parsed.render(), text, "vector {name}: canonical write mismatch");
                // 整幀重編碼逐字節相等。
                assert_eq!(frame_hex_of(text), *frame_hex, "vector {name}: frame re-encode mismatch");
            }
            "reject_codec" => {
                frame::check_len(len, MAX_FRAME_DEFAULT)
                    .unwrap_or_else(|e| panic!("vector {name}: {e}"));
                let text = std::str::from_utf8(rest)
                    .unwrap_or_else(|e| panic!("vector {name}: payload not utf8: {e}"));
                assert!(
                    Json::parse(text).is_err(),
                    "vector {name}: reader must reject payload {text:?}"
                );
            }
            "reject_frame" => {
                assert!(
                    frame::check_len(len, MAX_FRAME_DEFAULT).is_err(),
                    "vector {name}: frame-level reject expected (len={len})"
                );
            }
            "reject_utf8" => {
                frame::check_len(len, MAX_FRAME_DEFAULT)
                    .unwrap_or_else(|e| panic!("vector {name}: {e}"));
                assert!(
                    std::str::from_utf8(rest).is_err(),
                    "vector {name}: non-UTF-8 payload must be rejected"
                );
            }
            other => panic!("vector {name}: unknown kind {other}"),
        }
    }
}

#[test]
fn vectors_cover_required_surface() {
    let names: Vec<String> = load_vectors().into_iter().map(|(n, ..)| n).collect();
    // §6.3 清單逐項在場(按名字核對)。
    for required in [
        "hello_request",
        "analyze_request_nested",
        "str_all_seven_escapes",
        "ctrl_char_lowercase_u",
        "cjk_astral_raw",
        "i64_min",
        "i64_max",
        "error_frame",
        "oversize_len_prefix",
        "empty_len_frame",
        "float_payload_rejected",
        "duplicate_key_rejected",
        "non_utf8_payload_rejected",
    ] {
        assert!(names.iter().any(|n| n == required), "missing vector: {required}");
    }
}

/// i64 邊界斷言(R1-2):decode 出的 Int 是 i64::MIN 原值。
#[test]
fn i64_boundary_value_preserved() {
    let vectors = load_vectors();
    for (name, kind, frame_hex, _n) in &vectors {
        if name != "i64_min" || kind != "canonical" {
            continue;
        }
        let bytes = hex_decode(frame_hex);
        let text = std::str::from_utf8(&bytes[HEADER_LEN..]).unwrap();
        let parsed = Json::parse(text).unwrap();
        let Json::Obj(entries) = parsed else { panic!("obj expected") };
        let Some((_, Json::Int(n))) = entries.iter().find(|(k, _)| k == "n") else {
            panic!("n key missing in i64_min vector");
        };
        assert_eq!(*n, i64::MIN);
        assert_eq!(&bytes[HEADER_LEN..], b"{\"n\":-9223372036854775808}".as_slice());
        return;
    }
    panic!("i64_min canonical vector missing");
}

/// analyze 響應向量=真編碼器產出的收據:report::analysis_result 一動=向量
/// stale,必須同輪重錄(生成器 regen_fixtures)並讀 diff——chase-ledger 同律。
#[test]
fn vectors_analyze_response_matches_encoder() {
    let a = analyzer_core::analyze(&analyzer_core::SourceInput::Sv(analyzer_core::SvInput {
        name: "t.v".into(),
        source: ANALYZE_VECTOR_SOURCE.into(),
        ..Default::default()
    }));
    let response = Json::Obj(vec![
        ("id".into(), Json::Int(2)),
        ("ok".into(), Json::Bool(true)),
        ("result".into(), analyzer_core::report::analysis_result(&a)),
    ]);
    let vectors = load_vectors();
    let entry = vectors
        .iter()
        .find(|(n, k, _, _)| n == "analyze_response_nested" && k == "canonical")
        .unwrap_or_else(|| panic!("analyze_response_nested canonical vector missing"));
    assert_eq!(entry.2, frame_hex_of(&response.render()), "向量 stale:重跑生成器");
}

/// 生成器(首錄;改寫律時手動 `cargo test -p analyzer-core --test
/// codec_vectors regen_fixtures -- --ignored` 重錄,然後雙端測試對帳)。
#[test]
#[ignore]
fn regen_fixtures() {
    let obj = |entries: Vec<(&str, Json)>| {
        Json::Obj(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    };
    let s = |v: &str| Json::Str(v.to_string());

    let mut vectors: Vec<Json> = Vec::new();
    let push = |name: &str, kind: &str, hex: String, note: &str, out: &mut Vec<Json>| {
        out.push(obj(vec![
            ("name", s(name)),
            ("kind", s(kind)),
            ("frame_hex", Json::Str(hex)),
            ("note", s(note)),
        ]));
    };

    // ---- canonical:完整幀(prefix+payload),雙端逐字節對齊 ----
    let hello_request = obj(vec![
        ("id", Json::Int(1)),
        ("op", s("hello")),
        ("args", obj(vec![("proto", Json::Int(1)), ("client", obj(vec![("name", s("smoke")), ("version", s("0.1.0"))]))])),
    ]);
    push("hello_request", "canonical", frame_hex_of(&hello_request.render()),
        "握手請求(§5.4)", &mut vectors);

    let hello_response = obj(vec![
        ("id", Json::Int(1)),
        ("ok", Json::Bool(true)),
        ("result", obj(vec![
            ("proto", Json::Int(1)),
            ("server", s("analyzerd")),
            ("version", s("0.1.0")),
            ("ops", Json::Arr(vec![s("hello"), s("ping"), s("analyze"), s("shutdown")])),
        ])),
    ]);
    push("hello_response", "canonical", frame_hex_of(&hello_response.render()),
        "握手響應(§5.4)", &mut vectors);

    let analyze_request = obj(vec![
        ("id", Json::Int(2)),
        ("op", s("analyze")),
        ("args", obj(vec![
            ("kind", s("sv")),
            ("name", s("t.v")),
            ("source", s("module t(input a);\n  assign a = 1'b0;\nendmodule\n")),
            ("include_dirs", Json::Arr(vec![s("/tmp/inc"), s("/tmp/inc2")])),
            ("defines", Json::Arr(vec![s("-DX"), s("-DY=1")])),
        ])),
    ]);
    push("analyze_request_nested", "canonical", frame_hex_of(&analyze_request.render()),
        "analyze 請求(嵌套 Arr/Obj,§5.5)", &mut vectors);

    // analyze 響應向量=**真編碼器產出**(report::analysis_result 對真實
    // analyze() 結果),非手拼樹;穩定性由 vectors_analyze_response_matches_encoder 釘死。
    let analysis = analyzer_core::analyze(&analyzer_core::SourceInput::Sv(analyzer_core::SvInput {
        name: "t.v".into(),
        source: ANALYZE_VECTOR_SOURCE.into(),
        ..Default::default()
    }));
    let analyze_response = obj(vec![
        ("id", Json::Int(2)),
        ("ok", Json::Bool(true)),
        ("result", analyzer_core::report::analysis_result(&analysis)),
    ]);
    push("analyze_response_nested", "canonical", frame_hex_of(&analyze_response.render()),
        "analyze 響應(真編碼器產出,§13)", &mut vectors);

    let seven_escapes = obj(vec![("s", Json::Str("a\"b\\c\u{8}d\u{c}e\nf\rg\th".into()))]);
    push("str_all_seven_escapes", "canonical", frame_hex_of(&seven_escapes.render()),
        "全部七種轉義 \\\" \\\\ \\b \\f \\n \\r \\t(§5.3 律 3)", &mut vectors);

    let ctrl = obj(vec![("s", Json::Str("\u{f}".into()))]);
    push("ctrl_char_lowercase_u", "canonical", frame_hex_of(&ctrl.render()),
        "控制字符 U+000F → \\u000f 小寫角(§5.3 律 3)", &mut vectors);

    let cjk = obj(vec![("s", Json::Str("中文😀🔒".into()))]);
    push("cjk_astral_raw", "canonical", frame_hex_of(&cjk.render()),
        "CJK+astral 字符原樣 UTF-8 角(不拆代理對)", &mut vectors);

    let i64min = obj(vec![("n", Json::Int(i64::MIN))]);
    push("i64_min", "canonical", frame_hex_of(&i64min.render()),
        "i64 下界 -9223372036854775808(TS 側 bigint 路線,R1-2)", &mut vectors);

    let i64max = obj(vec![("n", Json::Int(i64::MAX))]);
    push("i64_max", "canonical", frame_hex_of(&i64max.render()),
        "i64 上界 9223372036854775807", &mut vectors);

    let err_frame = obj(vec![
        ("id", Json::Int(3)),
        ("ok", Json::Bool(false)),
        ("error", obj(vec![("code", s("E_OP")), ("message", s("unknown op `frobnicate`"))])),
    ]);
    push("error_frame", "canonical", frame_hex_of(&err_frame.render()),
        "錯誤幀形(§5.4/§5.6)", &mut vectors);

    let ping_request = obj(vec![
        ("id", Json::Int(4)),
        ("op", s("ping")),
        ("args", Json::Obj(vec![])),
    ]);
    push("ping_request_empty_obj", "canonical", frame_hex_of(&ping_request.render()),
        "空對象 args(ping,§5.5)", &mut vectors);

    // ---- reject_codec:合法幀頭+合法 UTF-8 負載,reader 必拒(§5.2) ----
    let push_reject = |name: &str, payload: String, note: &str, out: &mut Vec<Json>| {
        push(name, "reject_codec", frame_hex_of(&payload), note, out);
    };
    push_reject("float_payload_rejected", "{\"x\":1.5}".into(), "浮點禁項", &mut vectors);
    push_reject("exponent_payload_rejected", "{\"x\":1e5}".into(), "指數禁項", &mut vectors);
    push_reject("duplicate_key_rejected", "{\"a\":1,\"a\":2}".into(), "對象重複鍵禁項", &mut vectors);
    push_reject("leading_zero_rejected", "{\"x\":01}".into(), "前導零禁項", &mut vectors);
    push_reject("trailing_comma_rejected", "{\"a\":[1,2,]}".into(), "尾逗號禁項", &mut vectors);
    push_reject("nan_literal_rejected", "{\"x\":NaN}".into(), "NaN/字面量禁項", &mut vectors);
    push_reject("comment_rejected", "{\"a\":1}//c".into(), "註釋禁項", &mut vectors);
    push_reject(
        "raw_control_char_rejected",
        format!("{{\"s\":\"{}\"}}", '\u{1}'),
        "字符串內原始控制字節禁項",
        &mut vectors,
    );
    push_reject("int_overflow_pos_rejected", "{\"n\":9223372036854775809}".into(), "越 i64 上界禁項", &mut vectors);
    push_reject("int_overflow_neg_rejected", "{\"n\":-9223372036854775809}".into(), "越 i64 下界禁項", &mut vectors);
    push_reject("lone_surrogate_rejected", "{\"s\":\"\\ud800\"}".into(), "孤高代理轉義禁項", &mut vectors);

    // ---- reject_frame:幀頭級違約(先檢查後分配,§5.1) ----
    push("oversize_len_prefix", "reject_frame", "ffffffffdeadbeef".into(),
        "len=0xFFFFFFFF 超限,不讀負載不分配", &mut vectors);
    push("empty_len_frame", "reject_frame", "00000000".into(),
        "len=0 空幀違約", &mut vectors);

    // ---- reject_utf8:合法幀頭+非法 UTF-8 負載(§5.6 E_PROTO) ----
    let mut bad_payload = Vec::new();
    bad_payload.extend_from_slice(b"{\"s\":\"");
    bad_payload.extend_from_slice(&[0xFF, 0xFE]);
    bad_payload.extend_from_slice(b"\"}");
    push("non_utf8_payload_rejected", "reject_utf8", frame_hex_of_bytes(&bad_payload),
        "負載非 UTF-8(0xFF 0xFE)", &mut vectors);

    let root = obj(vec![("vectors", Json::Arr(vectors))]);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../clients/analyzer-ts/fixtures/codec-vectors.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, root.render() + "\n").unwrap();
    println!("wrote {}", VECTORS_PATH);
}
