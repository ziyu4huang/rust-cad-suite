//! cli — rtl-spec 原生 CLI(agent 唯一接口;D-S2;CLI-AGENT 憲法,ADR-0014 草案)。
//!
//! host-only(lib.rs cfg gate;wasm 構建只建 lib,bin 不入 wasm 面)。
//! 憲法① JSON-first:資料子命令 stdout=raw JSON(**無尾隨換行**,與 wasm
//! 導出 byte-equal——run-spec-cli-parity.sh 判據);人讀診斷一律 stderr。
//! 憲法② self-contain:`--help` 頂部攜完整工作流配方,每子命令 `--help`
//! 自解釋,錯誤自帶下一步可執行提示。exit code 四值:0=成功;1=域內失敗
//! (findings/被測紅/驗證不過);2=用法或環境缺;3=輸入檔不存在。默認零
//! FS 寫(tb --emit-v 需顯式 --out)。

use std::io::Write;

use crate::spec::{parse_spec_md, spec_json, spec_lint, spec_lint_json};
use crate::{analysis_json, extract_rules, gap_report_json, rulebook_json, analyze_summary,
    scan_gaps, tb_plan, tb_plan_frame, tb_template_v, SourceInput, SvInput, RtlilInput,
    DEFAULT_MAX_RULES, TbOpts};

const TOP_HELP: &str = "\
rtl-spec — RTL spec/規則書/TB 工具鏈,agent 唯一接口(rust-cad-suite;CLI-AGENT 憲法)

Usage:
  rtl-spec analyze <file.sv|.il>                      結構投影摘要(JSON)
  rtl-spec extract <file> [--lang sv|rtlil] [--max-rules N] [--format json|md]
                                                      行為規則書(默認 JSON)
  rtl-spec gaps <file.sv|.il>                         破口掃描(JSON)
  rtl-spec tb <file> --module M [--seed S] [--cycles C] [--flush F]
                                                      TB 幀 JSON;--emit-v --out <path> 生成模板 TB
  rtl-spec <cmd> --help                               每子命令自解釋

工作流配方(spec 撰寫→盲寫 RTL→lint→extract 對拍→tb 生成→conformance 評測):
  1. 依格式撰寫 <design>.spec.md(plugin/rtl2spec/fixtures/d-spec/ 有三份正樣可抄形)
  2. rtl-spec spec parse <design>.spec.md   驗解析(結構 JSON)
  3. rtl-spec spec lint <design>.spec.md    七查(exit 0=clean / 1=findings)
  4. 盲寫 top.v:只讀 spec,不讀原 DUT;頂模組名==spec module、端口逐字合 contract
  5. rtl-spec extract <top.v>               規則書 JSON,與 spec 逐條對讀
  6. rtl-spec tb <top.v> --module <M>       TB 幀;--emit-v --out tb.v 出模板 TB
  7. bash plugin/rtl2spec/run-spec-eval.sh  conformance 評測(iverilog 編譯+公共 TB 場景 diff)

Exit codes: 0=成功;1=域內失敗(findings/被測紅/驗證不過);2=用法或環境缺;3=輸入檔不存在。
stdout=raw JSON(無尾隨換行,與 wasm 導出 byte-equal);人讀診斷走 stderr。
(run-spec-eval.sh 隨 D-S4 落地;屆時本配方逐字可用。)
";

const ANALYZE_HELP: &str = "\
rtl-spec analyze — 結構投影摘要(端口/參數/聲明;與 wasm rtl_analyze_* byte-equal)

Usage: rtl-spec analyze <file.sv|.il>

  <file>   .sv/.v=SV 面,.il=RTLIL 面(--lang 缺省時按副檔名判定)
  --lang sv|rtlil   覆寫語言判定

stdout=raw JSON(無尾隨換行)。Exit codes: 0/2/3(--help 頂部四值約定)。
Example: rtl-spec analyze counter.v
";

const EXTRACT_HELP: &str = "\
rtl-spec extract — 行為規則書提取(與 wasm rtl_extract_rules_* byte-equal)

Usage: rtl-spec extract <file> [--lang sv|rtlil] [--max-rules N] [--format json|md]

  <file>            .sv/.v(SV)或 .il(RTLIL;--lang 缺省時按副檔名判定)
  --lang sv|rtlil   覆寫語言判定
  --max-rules N     每模組規則上限(0 或缺省=核心默認 256)
  --format json|md  stdout 形(json=raw JSON 鍵序 byte-exact;md=規則書 markdown 發射面)

Exit codes: 0=成功;2=用法/旗標誤;3=輸入檔不可讀。
Example: rtl-spec extract top.v --max-rules 128
";

const GAPS_HELP: &str = "\
rtl-spec gaps — 破口掃描(測試缺口清單;與 wasm rtl_scan_gaps_* byte-equal)

Usage: rtl-spec gaps <file.sv|.il>

  <file>            .sv/.v(SV)或 .il(RTLIL;--lang 缺省時按副檔名判定)
  --lang sv|rtlil   覆寫語言判定

stdout=raw JSON(無尾隨換行)。Exit codes: 0=成功;2=用法/旗標誤;3=輸入檔不可讀。
Example: rtl-spec gaps counter.v
";

const TB_HELP: &str = "\
rtl-spec tb — 測試計劃幀(與 wasm rtl_tb_plan_sv byte-equal)

Usage: rtl-spec tb <file> --module M [--seed S] [--cycles C] [--flush F] [--emit-v --out <path>]

  --module M        必填;目標模組名
  --seed S          0 或缺省=核心默認;--cycles C 0 或缺省=32;--flush F 0 或缺省=8
  --emit-v          生成模板 TB Verilog;必須顯式 --out <path>(默認零 FS 寫)

stdout=幀 JSON:{\"ok\":true,\"plan\":{…},\"template_v\":\"…\"} 或
{\"ok\":false,\"error\":\"…(milestone TB1)\"}(錯誤走資料,不 trap)。
計劃失敗=exit 1(stdout 幀仍出,供 agent 解析 error 欄);--out 寫失敗=exit 2。
Exit codes: 0=成功;1=計劃失敗(TB1);2=用法/環境缺;3=輸入檔不可讀。
Example: rtl-spec tb top.v --module counter --emit-v --out tb_gen.v
";

const VALUE_LANG: &str = "--lang";
const VALUE_FORMAT: &str = "--format";
const VALUE_MAX_RULES: &str = "--max-rules";
const VALUE_MODULE: &str = "--module";
const VALUE_SEED: &str = "--seed";
const VALUE_CYCLES: &str = "--cycles";
const VALUE_FLUSH: &str = "--flush";
const VALUE_OUT: &str = "--out";
const BOOL_EMIT_V: &str = "--emit-v";

/// 解析結果;Help=印該子命令 help(exit 0);Err=已印 stderr 診斷的退出碼。
enum ArgParse {
    Help,
    Parsed(Parsed),
    Err(i32),
}

#[derive(Default)]
struct Parsed {
    file: Option<String>,
    lang: Option<String>,
    format: Option<String>,
    max_rules: Option<String>,
    module: Option<String>,
    seed: Option<String>,
    cycles: Option<String>,
    flush: Option<String>,
    out: Option<String>,
    emit_v: bool,
}

fn parse_args(
    cmd: &'static str,
    value_flags: &[&str],
    bool_flags: &[&str],
    args: &[String],
) -> ArgParse {
    let mut p = Parsed::default();
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--help" || a == "-h" {
            return ArgParse::Help;
        }
        if a.starts_with("--") {
            let (name, inline) = match a.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None => (a, None),
            };
            if bool_flags.contains(&name) {
                if inline.is_some() {
                    unknown_flag(cmd, a);
                    return ArgParse::Err(2);
                }
                p.emit_v = true;
                i += 1;
                continue;
            }
            if value_flags.contains(&name) {
                let v = match inline {
                    Some(v) => v,
                    None => {
                        if i + 1 >= args.len() {
                            eprintln!("rtl-spec {cmd}: {name} 缺值 — run: rtl-spec {cmd} --help");
                            return ArgParse::Err(2);
                        }
                        i += 1;
                        args[i].clone()
                    }
                };
                match name {
                    VALUE_LANG => p.lang = Some(v),
                    VALUE_FORMAT => p.format = Some(v),
                    VALUE_MAX_RULES => p.max_rules = Some(v),
                    VALUE_MODULE => p.module = Some(v),
                    VALUE_SEED => p.seed = Some(v),
                    VALUE_CYCLES => p.cycles = Some(v),
                    VALUE_FLUSH => p.flush = Some(v),
                    VALUE_OUT => p.out = Some(v),
                    _ => {}
                }
                i += 1;
                continue;
            }
            unknown_flag(cmd, a);
            return ArgParse::Err(2);
        }
        if p.file.is_some() {
            eprintln!("rtl-spec {cmd}: 多餘位置參數 `{a}` — run: rtl-spec {cmd} --help");
            return ArgParse::Err(2);
        }
        p.file = Some(a.to_string());
        i += 1;
    }
    ArgParse::Parsed(p)
}

fn unknown_flag(cmd: &str, flag: &str) {
    eprintln!("rtl-spec {cmd}: 未知旗標 `{flag}` — run: rtl-spec {cmd} --help");
}

fn put(out: &mut dyn Write, s: &str) -> i32 {
    match out.write_all(s.as_bytes()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("rtl-spec: stdout 寫出失敗: {e}");
            2
        }
    }
}

/// 入口(vl-cli 同款:薄 main 把返回值交 std::process::exit)。
pub fn run(args: &[String]) -> i32 {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let code = dispatch(args, &mut lock);
    let _ = lock.flush();
    code
}

fn dispatch(args: &[String], out: &mut dyn Write) -> i32 {
    match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") => put(out, TOP_HELP),
        Some("analyze") => cmd_analyze(&args[1..], out),
        Some("extract") => cmd_extract(&args[1..], out),
        Some("gaps") => cmd_gaps(&args[1..], out),
        Some("tb") => cmd_tb(&args[1..], out),
        Some("spec") => cmd_spec(&args[1..], out),
        Some(other) => {
            eprintln!("rtl-spec: 未知子命令 `{other}` — run: rtl-spec --help 看工作流配方");
            2
        }
    }
}

fn make_input(name: &str, source: &str, lang: &str) -> SourceInput {
    if lang == "rtlil" {
        SourceInput::Rtlil(RtlilInput {
            name: name.to_string(),
            source: source.to_string(),
        })
    } else {
        SourceInput::Sv(SvInput {
            name: name.to_string(),
            source: source.to_string(),
            ..Default::default()
        })
    }
}

fn resolve_lang(cmd: &str, explicit: Option<&str>, path: &str) -> Result<&'static str, i32> {
    match explicit {
        Some("sv") => Ok("sv"),
        Some("rtlil") => Ok("rtlil"),
        Some(other) => {
            eprintln!("rtl-spec {cmd}: --lang 值 `{other}` ∉ sv|rtlil — run: rtl-spec {cmd} --help");
            Err(2)
        }
        None if path.ends_with(".il") => Ok("rtlil"),
        None => Ok("sv"),
    }
}

fn read_input(cmd: &str, path: &str) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!(
                "rtl-spec {cmd}: 輸入檔不可讀: {path}({e}) — 檢查路徑;run: rtl-spec {cmd} --help"
            );
            None
        }
    }
}

fn u32_opt(cmd: &str, flag: &str, v: Option<&String>) -> Result<Option<u32>, i32> {
    match v {
        None => Ok(None),
        Some(s) => match s.parse::<u32>() {
            Ok(n) => Ok(Some(n)),
            Err(_) => {
                eprintln!("rtl-spec {cmd}: {flag} 值 `{s}` 非非負整數 — run: rtl-spec {cmd} --help");
                Err(2)
            }
        },
    }
}

fn need_file(cmd: &'static str, usage: &str, file: Option<&String>) -> Result<String, i32> {
    match file {
        Some(f) => Ok(f.clone()),
        None => {
            eprintln!("rtl-spec {cmd}: 缺輸入檔\n  {usage}\n  run: rtl-spec {cmd} --help");
            Err(2)
        }
    }
}

const SPEC_HELP: &str = "\\
rtl-spec spec — 高級 spec 檔解析與七查 lint(與 wasm rtl_spec_spec_parse/lint byte-equal)

Usage:
  rtl-spec spec parse <spec.md>    結構 JSON(stdout;exit 0)
  rtl-spec spec lint <spec.md>    findings JSON 陣列(stdout);exit 0=clean / 1=findings

  <spec.md>  高級 spec 檔(格式正樣=plugin/rtl2spec/fixtures/d-spec/ 三檔可抄形)
stdout=raw JSON(無尾隨換行)。findings 陣列每項=一條人讀診斷(①..⑦ 前綴標查號)。
Exit codes: 0=clean(或 parse 成功);1=findings;2=用法/環境缺;3=輸入檔不可讀。
Example: rtl-spec spec lint counter8.spec.md
";

fn cmd_spec(args: &[String], out: &mut dyn Write) -> i32 {
    match args.first().map(String::as_str) {
        None | Some("--help") | Some("-h") => put(out, SPEC_HELP),
        Some("parse") => cmd_spec_parse(&args[1..], out),
        Some("lint") => cmd_spec_lint(&args[1..], out),
        Some(other) => {
            eprintln!("rtl-spec spec: 未知子命令 `{other}`(可用: parse lint)— run: rtl-spec spec --help");
            2
        }
    }
}

fn cmd_spec_parse(args: &[String], out: &mut dyn Write) -> i32 {
    match parse_args("spec parse", &[], &[], args) {
        ArgParse::Help => put(out, SPEC_HELP),
        ArgParse::Err(c) => c,
        ArgParse::Parsed(p) => {
            let path = match need_file("spec parse", "rtl-spec spec parse <spec.md>", p.file.as_ref()) {
                Ok(f) => f,
                Err(c) => return c,
            };
            let Some(src) = read_input("spec parse", &path) else {
                return 3;
            };
            match parse_spec_md(&src) {
                Ok(doc) => put(out, &spec_json(&doc)),
                Err(e) => {
                    eprintln!("rtl-spec spec parse: {e} — run: rtl-spec spec parse --help");
                    2
                }
            }
        }
    }
}

fn cmd_spec_lint(args: &[String], out: &mut dyn Write) -> i32 {
    match parse_args("spec lint", &[], &[], args) {
        ArgParse::Help => put(out, SPEC_HELP),
        ArgParse::Err(c) => c,
        ArgParse::Parsed(p) => {
            let path = match need_file("spec lint", "rtl-spec spec lint <spec.md>", p.file.as_ref()) {
                Ok(f) => f,
                Err(c) => return c,
            };
            let Some(src) = read_input("spec lint", &path) else {
                return 3;
            };
            match parse_spec_md(&src) {
                Ok(doc) => {
                    let json = spec_lint_json(&doc);
                    let clean = json == "[]";
                    let w = put(out, &json);
                    if !clean {
                        let n = spec_lint(&doc).len();
                        eprintln!("rtl-spec spec lint: {n} findings — 逐條見 stdout 陣列;修復後重跑(run: rtl-spec spec lint --help)");
                        return 1;
                    }
                    w
                }
                Err(e) => {
                    eprintln!("rtl-spec spec lint: {e} — run: rtl-spec spec lint --help");
                    2
                }
            }
        }
    }
}

fn cmd_analyze(args: &[String], out: &mut dyn Write) -> i32 {
    match parse_args("analyze", &[], &[], args) {
        ArgParse::Help => put(out, ANALYZE_HELP),
        ArgParse::Err(c) => c,
        ArgParse::Parsed(p) => {
            let path = match need_file("analyze", "rtl-spec analyze <file.sv|.il>", p.file.as_ref()) {
                Ok(f) => f,
                Err(c) => return c,
            };
            let lang = match resolve_lang("analyze", None, &path) {
                Ok(l) => l,
                Err(c) => return c,
            };
            let Some(src) = read_input("analyze", &path) else {
                return 3;
            };
            let input = make_input(&path, &src, lang);
            put(out, &analysis_json(&analyze_summary(&input)))
        }
    }
}

fn cmd_extract(args: &[String], out: &mut dyn Write) -> i32 {
    let flags = [VALUE_LANG, VALUE_FORMAT, VALUE_MAX_RULES];
    match parse_args("extract", &flags, &[], args) {
        ArgParse::Help => put(out, EXTRACT_HELP),
        ArgParse::Err(c) => c,
        ArgParse::Parsed(p) => {
            let path = match need_file("extract", "rtl-spec extract <file> [--lang sv|rtlil] [--max-rules N] [--format json|md]", p.file.as_ref()) {
                Ok(f) => f,
                Err(c) => return c,
            };
            let lang = match resolve_lang("extract", p.lang.as_deref(), &path) {
                Ok(l) => l,
                Err(c) => return c,
            };
            let format = match p.format.as_deref() {
                None => "json",
                Some("json") => "json",
                Some("md") => "md",
                Some(other) => {
                    eprintln!("rtl-spec extract: --format 值 `{other}` ∉ json|md — run: rtl-spec extract --help");
                    return 2;
                }
            };
            let max = match u32_opt("extract", VALUE_MAX_RULES, p.max_rules.as_ref()) {
                Ok(n) => n.unwrap_or(0),
                Err(c) => return c,
            };
            let max = if max == 0 { DEFAULT_MAX_RULES } else { max as usize };
            let Some(src) = read_input("extract", &path) else {
                return 3;
            };
            let input = make_input(&path, &src, lang);
            let rb = extract_rules(&input, max);
            if format == "md" {
                put(out, &rb.render_markdown())
            } else {
                put(out, &rulebook_json(&rb))
            }
        }
    }
}

fn cmd_gaps(args: &[String], out: &mut dyn Write) -> i32 {
    match parse_args("gaps", &[], &[], args) {
        ArgParse::Help => put(out, GAPS_HELP),
        ArgParse::Err(c) => c,
        ArgParse::Parsed(p) => {
            let path = match need_file("gaps", "rtl-spec gaps <file.sv|.il>", p.file.as_ref()) {
                Ok(f) => f,
                Err(c) => return c,
            };
            let lang = match resolve_lang("gaps", p.lang.as_deref(), &path) {
                Ok(l) => l,
                Err(c) => return c,
            };
            let Some(src) = read_input("gaps", &path) else {
                return 3;
            };
            let input = make_input(&path, &src, lang);
            put(out, &gap_report_json(&scan_gaps(&input)))
        }
    }
}

fn cmd_tb(args: &[String], out: &mut dyn Write) -> i32 {
    let flags = [VALUE_MODULE, VALUE_SEED, VALUE_CYCLES, VALUE_FLUSH, VALUE_OUT];
    let bools = [BOOL_EMIT_V];
    match parse_args("tb", &flags, &bools, args) {
        ArgParse::Help => put(out, TB_HELP),
        ArgParse::Err(c) => c,
        ArgParse::Parsed(p) => {
            let path = match need_file("tb", "rtl-spec tb <file> --module M [--seed S] [--cycles C] [--flush F] [--emit-v --out <path>]", p.file.as_ref()) {
                Ok(f) => f,
                Err(c) => return c,
            };
            let Some(module) = p.module.as_deref() else {
                eprintln!("rtl-spec tb: 缺必填 --module M — run: rtl-spec tb --help");
                return 2;
            };
            if p.emit_v && p.out.is_none() {
                eprintln!("rtl-spec tb: --emit-v 需顯式 --out <path>(默認零 FS 寫)— run: rtl-spec tb --help");
                return 2;
            }
            if !p.emit_v && p.out.is_some() {
                eprintln!("rtl-spec tb: --out 僅與 --emit-v 同用 — run: rtl-spec tb --help");
                return 2;
            }
            let seed = match u32_opt("tb", VALUE_SEED, p.seed.as_ref()) {
                Ok(v) => v.unwrap_or(0),
                Err(c) => return c,
            };
            let cycles = match u32_opt("tb", VALUE_CYCLES, p.cycles.as_ref()) {
                Ok(v) => v.unwrap_or(0),
                Err(c) => return c,
            };
            let flush = match u32_opt("tb", VALUE_FLUSH, p.flush.as_ref()) {
                Ok(v) => v.unwrap_or(0),
                Err(c) => return c,
            };
            let lang = match resolve_lang("tb", p.lang.as_deref(), &path) {
                Ok(l) => l,
                Err(c) => return c,
            };
            let Some(src) = read_input("tb", &path) else {
                return 3;
            };
            let input = make_input(&path, &src, lang);
            let frame = tb_plan_frame(&input, Some(module), seed, cycles, flush);
            let failed = frame.starts_with("{\"ok\":false");
            if p.emit_v && !failed {
                match tb_plan(&input, Some(module)) {
                    Ok(plan) => {
                        let opts = TbOpts::resolve(seed, cycles, flush);
                        let v = tb_template_v(&plan, &opts);
                        let out_path = p.out.as_deref().unwrap_or_default();
                        if let Err(e) = std::fs::write(out_path, v) {
                            eprintln!("rtl-spec tb: 寫 `{out_path}` 失敗: {e}");
                            return 2;
                        }
                        eprintln!("rtl-spec tb: 模板 TB 已寫 {out_path}");
                    }
                    Err(_) => {} // failed 已涵蓋;防禦臂。
                }
            }
            let w = put(out, &frame);
            if failed {
                eprintln!("rtl-spec tb: 計劃失敗(stdout 幀 error 欄,詳 milestone TB1)— run: rtl-spec tb --help");
                return 1;
            }
            w
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg_file(rel: &str) -> String {
        // 套件根(crates/rtl-spec)相對;CWD=cargo test 套件根,與 CLI 讀檔同基準。
        format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))
    }

    fn buf(args: &[&str]) -> (i32, String) {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let mut b = Vec::new();
        let code = dispatch(&args, &mut b);
        (code, String::from_utf8(b).expect("utf8"))
    }

    fn sv_input(name: &str, source: &str) -> SourceInput {
        SourceInput::Sv(SvInput {
            name: name.to_string(),
            source: source.to_string(),
            ..Default::default()
        })
    }

    fn il_input(name: &str, source: &str) -> SourceInput {
        SourceInput::Rtlil(RtlilInput {
            name: name.to_string(),
            source: source.to_string(),
        })
    }

    // 相對 CWD(cargo test 的 CWD=套件根 crates/rtl-spec);name=路徑逐字,
    // 兩側(呼叫與期望)同字串即可 byte-equal。
    const SV: &str = "tests/fixtures/rulebook_sample.v";
    const IL: &str = "tests/fixtures/sample.il";
    const SPEC: &str = "../../plugin/rtl2spec/fixtures/d-spec/counter8.spec.md";

    #[test]
    fn top_help_exit0_with_recipe_keywords() {
        let (code, out) = buf(&[]);
        assert_eq!(code, 0);
        for kw in ["工作流配方", "extract", "gaps", "analyze", "Exit codes", "spec lint", "byte-equal"] {
            assert!(out.contains(kw), "--help 缺配方關鍵字 `{kw}`");
        }
        let (code, out) = buf(&["--help"]);
        assert_eq!(code, 0);
        assert!(out.contains("工作流配方"));
    }

    #[test]
    fn subcommand_help_self_explains() {
        for (code, out) in [buf(&["extract", "--help"]), buf(&["tb", "-h"])] {
            assert_eq!(code, 0);
            assert!(out.contains("Usage:") && out.contains("Exit codes"), "{out}");
        }
    }

    #[test]
    fn exit0_data_legs_byte_equal_with_core() {
        let sv_src = std::fs::read_to_string(pkg_file(SV)).unwrap();
        let il_src = std::fs::read_to_string(pkg_file(IL)).unwrap();
        let spec_src = std::fs::read_to_string(pkg_file(SPEC)).unwrap();

        let (code, out) = buf(&["analyze", SV]);
        assert_eq!(code, 0);
        assert_eq!(out, analysis_json(&analyze_summary(&sv_input(SV, &sv_src))));

        let (code, out) = buf(&["analyze", IL]);
        assert_eq!(code, 0);
        assert_eq!(out, analysis_json(&analyze_summary(&il_input(IL, &il_src))));

        // 默認 max-rules(wasm 契約 0=默認;CLI 缺省=核心默認)。
        let (code, out) = buf(&["extract", SV]);
        assert_eq!(code, 0);
        assert_eq!(
            out,
            rulebook_json(&extract_rules(&sv_input(SV, &sv_src), DEFAULT_MAX_RULES))
        );

        let (code, out) = buf(&["extract", SV, "--max-rules", "128"]);
        assert_eq!(code, 0);
        assert_eq!(
            out,
            rulebook_json(&extract_rules(&sv_input(SV, &sv_src), 128))
        );

        let (code, out) = buf(&["extract", IL]);
        assert_eq!(code, 0);
        assert_eq!(
            out,
            rulebook_json(&extract_rules(&il_input(IL, &il_src), DEFAULT_MAX_RULES))
        );

        // §5.1 命名腿:spec md 作輸入(SV 面)。
        let (code, out) = buf(&["extract", SPEC]);
        assert_eq!(code, 0);
        assert_eq!(
            out,
            rulebook_json(&extract_rules(&sv_input(SPEC, &spec_src), DEFAULT_MAX_RULES))
        );

        let (code, out) = buf(&["gaps", SV]);
        assert_eq!(code, 0);
        assert_eq!(out, gap_report_json(&scan_gaps(&sv_input(SV, &sv_src))));

        // raw JSON 無尾隨換行(byte-equal 的形面)。
        let (code, out) = buf(&["extract", SV]);
        assert_eq!(code, 0);
        assert!(!out.ends_with('\n'), "stdout 不得帶尾隨換行");
        assert!(out.starts_with('{'));
    }

    #[test]
    fn exit1_tb_tb1_frame() {
        // RTLIL 面恆 TB1(wasm 同契約):幀 ok:false,CLI exit 1。
        let il_src = std::fs::read_to_string(pkg_file(IL)).unwrap();
        let (code, out) = buf(&["tb", IL, "--module", "m"]);
        assert_eq!(code, 1);
        assert_eq!(
            out,
            tb_plan_frame(&il_input(IL, &il_src), Some("m"), 0, 0, 0)
        );
        assert!(out.starts_with("{\"ok\":false"));
    }

    #[test]
    fn exit2_usage_family() {
        assert_eq!(buf(&["nope"]).0, 2);
        assert_eq!(buf(&["extract"]).0, 2);
        assert_eq!(buf(&["extract", "--lang"]).0, 2);
        assert_eq!(buf(&["extract", SV, "--lang", "banana"]).0, 2);
        assert_eq!(buf(&["extract", SV, "--format", "yaml"]).0, 2);
        assert_eq!(buf(&["extract", SV, "--max-rules", "x"]).0, 2);
        assert_eq!(buf(&["extract", SV, "--wat"]).0, 2);
        assert_eq!(buf(&["tb", SV]).0, 2);
        assert_eq!(buf(&["tb", SV, "--module", "m", "--emit-v"]).0, 2);
        assert_eq!(buf(&["tb", SV, "--module", "m", "--out", "x.v"]).0, 2);
        assert_eq!(buf(&["spec", "wat"]).0, 2);
    }

    #[test]
    fn exit3_missing_input() {
        assert_eq!(buf(&["extract", "/nonexistent/definitely-absent.v"]).0, 3);
        assert_eq!(buf(&["analyze", "/nonexistent/definitely-absent.il"]).0, 3);
        assert_eq!(buf(&["tb", "/nonexistent/definitely-absent.v", "--module", "m"]).0, 3);
    }

    #[test]
    fn tb_emit_v_writes_template() {
        let sv_src = std::fs::read_to_string(pkg_file(SV)).unwrap();
        let dir = std::env::temp_dir().join(format!("rtl-spec-cli-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tb_path = dir.join("tb_gen.v");
        let tb_arg = tb_path.to_string_lossy().to_string();
        let (code, out) = buf(&["tb", SV, "--module", "rulebook_sample", "--emit-v", "--out", &tb_arg]);
        assert_eq!(code, 0);
        assert_eq!(
            out,
            tb_plan_frame(&sv_input(SV, &sv_src), Some("rulebook_sample"), 0, 0, 0)
        );
        let v = std::fs::read_to_string(&tb_path).unwrap();
        let plan = tb_plan(&sv_input(SV, &sv_src), Some("rulebook_sample")).unwrap();
        assert_eq!(v, tb_template_v(&plan, &TbOpts::resolve(0, 0, 0)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn spec_subcommands_parse_lint() {
        // parse:exit 0+byte-equal spec_json。
        let spec_src = std::fs::read_to_string(pkg_file(SPEC)).unwrap();
        let (code, out) = buf(&["spec", "parse", SPEC]);
        assert_eq!(code, 0);
        assert_eq!(out, spec_json(&parse_spec_md(&spec_src).unwrap()));
        // lint 正樣:exit 0+「[]」。
        let (code, out) = buf(&["spec", "lint", SPEC]);
        assert_eq!(code, 0);
        assert_eq!(out, "[]");
        // lint 負樣:exit 1+findings 陣列與核心一致。
        let bad = "# spec: b\n- spec-format: 2\n- module: bm\n- port: a input wire scalar\n\n## behavior\n\n- B001 [c] 一\n  check(guard): trigger=a posedge; when=a; async=true; effects=a←1'b0\n\n## scenarios\n\n- SCENE1 s: c\n";
        let bad_path = {
            let p = std::env::temp_dir().join(format!("rtl-spec-cli-lint-{}.spec.md", std::process::id()));
            std::fs::write(&p, bad).unwrap();
            p.to_string_lossy().to_string()
        };
        let (code, out) = buf(&["spec", "lint", &bad_path]);
        assert_eq!(code, 1);
        let doc = parse_spec_md(bad).unwrap();
        assert_eq!(out, spec_lint_json(&doc));
        let _ = std::fs::remove_file(&bad_path);
        // 負樣檔不存在 → exit 3;spec 無參 → help。
        assert_eq!(buf(&["spec", "lint", "/nonexistent/x.spec.md"]).0, 3);
        assert_eq!(buf(&["spec"]).0, 0);
    }

    #[test]
    fn extract_md_format_matches_rulebook_renderer() {
        let sv_src = std::fs::read_to_string(pkg_file(SV)).unwrap();
        let (code, out) = buf(&["extract", SV, "--format", "md"]);
        assert_eq!(code, 0);
        assert_eq!(
            out,
            extract_rules(&sv_input(SV, &sv_src), DEFAULT_MAX_RULES).render_markdown()
        );
    }
}
