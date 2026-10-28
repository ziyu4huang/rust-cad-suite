//! M0 CLI 契約 goldens(RYOSYS-MVP §3-M0 閘③)。
//!
//! * 未註冊命令=逐字重現 oracle 的 No such command 錯誤(§0-3 實測);
//! * 登記層命中=milestone Y0..Y3 硬錯(D-ys2;Y 碼映射 §1.2);
//! * `-V` 自標識不冒充(D-ys1);
//! * `-p` 解析契約 fixture 組含無空格暗角形(§0-11/R2)。

use std::io::Write;
use std::process::{Command, Stdio};

fn ryosys() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ryosys"))
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = ryosys().args(args).output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn version_self_identify() {
    let (rc, so, _) = run(&["-V"]);
    assert_eq!(rc, 0);
    assert_eq!(so, format!("ryosys {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn read_sverilog_is_not_a_survey_command() {
    // 0.70+179 的 `-H` 普查**不含** read_sverilog(SV 併入 read_verilog)——
    // 它是普查外名字,逐字重現 oracle 的 No such command(非 Y3 stub)。
    let (rc, _, se) = run(&["-p", "read_sverilog a.sv"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: No such command: read_sverilog (type 'help' for a command overview)\n"
    );
}

#[test]
fn unregistered_command_verbatim_error() {
    // §0-3 逐字:`yosys -q -p "nosuchcmd"` 的錯誤面(ERROR 走 stderr)。
    let (rc, so, se) = run(&["-p", "nosuchcmd"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: No such command: nosuchcmd (type 'help' for a command overview)\n"
    );
    assert!(so.starts_with("-- Running command `nosuchcmd' --"), "stdout={so:?}");
    assert!(!so.contains("ERROR"));
}

#[test]
fn hierarchy_and_read_json_m1_surface() {
    // M1:hierarchy -auto-top/-top 與 read_json 閉環的 CLI 面(stub 已移除,
    // 命中實現層;未知旗標仍為 Y0 硬錯)。
    let dir = std::env::temp_dir().join(format!("ry-cli-m1-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        &dir.join("case.il"),
        "module \\top\n  wire input 1 \\a\n  wire output 1 \\y\n  connect \\y \\a\nend\n",
    )
    .unwrap();
    let script = "read_rtlil case.il; hierarchy -auto-top; write_rtlil -readable o.il";
    let out = ryosys().arg("-p").arg(&script).current_dir(&dir).output().unwrap();
    let _ = &script;
    assert_eq!(out.status.code(), Some(0));
    let so = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(so.contains("2. Executing HIERARCHY pass (managing design hierarchy).\n"));
    assert!(so.contains("2.1. Finding top of design hierarchy..\n"));
    assert!(so.contains("Automatically selected top as design top module.\n"));
    assert!(so.contains("2.2. Analyzing design hierarchy..\n"));
    assert!(so.contains("Top module:  \\top\n"));
    let il = std::fs::read_to_string(dir.join("o.il")).unwrap();
    assert!(il.contains("attribute \\top 1"));

    // -top 未知名(實測逐字)。
    let out = ryosys()
        .arg("-p")
        .arg("read_rtlil case.il; hierarchy -top nosuch")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let se = String::from_utf8_lossy(&out.stderr);
    assert_eq!(se, "ERROR: Module `nosuch' not found!\n");

    // 未知旗標=Y0。
    let out = ryosys()
        .arg("-p")
        .arg("read_rtlil case.il; hierarchy -check")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("milestone Y0"));

    // read_json:先產 json 再閉環。
    let out = ryosys()
        .arg("-p")
        .arg("read_rtlil case.il; write_json a.json")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let out = ryosys()
        .arg("-p")
        .arg("read_json a.json; write_rtlil -readable r.il; write_json r.json")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let so = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(so.contains("1. Executing JSON frontend.\n"));
    assert!(so.contains("Importing module top from JSON tree.\n"));
    let fa = std::fs::read_to_string(dir.join("a.json")).unwrap();
    let fr = std::fs::read_to_string(dir.join("r.json")).unwrap();
    let a: Vec<&str> = fa.lines().filter(|l| !l.contains("\"creator\"")).collect();
    let r: Vec<&str> = fr.lines().filter(|l| !l.contains("\"creator\"")).collect();
    assert_eq!(a, r, "read_json→write_json 閉環必須冪等");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn registered_stub_milestones() {
    // Y 碼映射(§1.2;M1 後 hierarchy/read_json/simplemap 已兌現實現層):
    // proc*/opt*→Y2(M2);read_verilog 族→Y3;其餘→Y0。
    for (cmd, y) in [
        ("proc", "Y2"),
        ("opt_expr -keepdc", "Y2"),
        ("proc_dff", "Y2"),
        ("read_verilog a.v", "Y3"),
        ("read_verilog_file_list f.txt", "Y3"),
        ("techmap", "Y0"),
        ("synth", "Y0"),
        ("flatten", "Y0"),
    ] {
        let (rc, _, se) = run(&["-p", cmd]);
        let first = cmd.split(' ').next().unwrap();
        assert_eq!(rc, 1, "{cmd}");
        assert_eq!(
            se,
            format!("ERROR: {first}: not implemented in ryosys (milestone {y})\n"),
            "{cmd}"
        );
    }
}

#[test]
fn simplemap_m1d_surface() {
    // M1-d:simplemap 無參子集——Mapping 行(`%s.%s (%s).` unescape 形)、
    // autoidx 分配($auto$simplemap.cc:<L>:<func>$N 行號錨=二進制實錄,
    // 87 例普查 16 錨與 clone 零漂移)、ff 家族 FfData per-bit slice
    // ($auto$ff.cc:337:slice$N)、swap-and-pop cell 序(remove 原 cell 把
    // 最後插入的 gate 挪前——寬 2 $dff dump=slice$2 先於 slice$1,oracle
    // 實測釘死)。形態斷言的 oracle 證據=/tmp 探針(always01+寬 2 ff 族
    // 五形);byte-exact 契約由 ys-e2e -sm 層 golden 承擔(鐵則:期望
    // 一律錄製)。
    let dir = std::env::temp_dir().join(format!("ry-cli-sm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        &dir.join("t.il"),
        "module \\top\n  wire input 1 \\c\n  wire width 2 input 2 \\d\n  wire width 2 output 3 \\q\n  wire output 4 \\y\n  cell $dff $f1\n    parameter \\CLK_POLARITY 1\n    parameter \\WIDTH 2\n    connect \\CLK \\c\n    connect \\D \\d\n    connect \\Q \\q\n  end\n  cell $not $n1\n    parameter \\A_SIGNED 0\n    parameter \\A_WIDTH 1\n    parameter \\Y_WIDTH 1\n    connect \\A \\c\n    connect \\Y \\y\n  end\nend\n",
    )
    .unwrap();
    let out = ryosys()
        .arg("-p")
        .arg("read_rtlil t.il; simplemap; write_rtlil -readable o.il")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let so = String::from_utf8_lossy(&out.stdout);
    assert!(so.contains("2. Executing SIMPLEMAP pass (map simple cells to gate primitives).\n"));
    // Mapping 行=cells 迭代序(逆插入序:$n1 先於 $f1),mod/cell/type unescape。
    let m1 = so.find("Mapping top.$n1 ($not).\n").expect("not mapping line");
    let m2 = so.find("Mapping top.$f1 ($dff).\n").expect("dff mapping line");
    assert!(m1 < m2, "Mapping 行序=cells 逆插入序");
    let il = std::fs::read_to_string(dir.join("o.il")).unwrap();
    // ff per-bit slice(寬 2 → 兩個 $_DFF_P_;autoidx 起=max($f1,$n1 後綴)=1
    // → slice$2/slice$3;dump 序=swap-and-pop:迭代序 $n1 先(逆插入)——
    // not gate 分配 $1、slice 分配 $2/$3,remove 後 dump=slice$3,not$1,
    // slice$2,oracle 兩端實測逐字)。
    assert!(il.contains("cell $_DFF_P_ $auto$ff.cc:337:slice$3"));
    assert!(il.contains("cell $_DFF_P_ $auto$ff.cc:337:slice$2"));
    assert!(il.contains("connect \\D \\d [1]"));
    assert!(il.contains("connect \\Q \\q [1]"));
    assert!(il.contains("cell $_NOT_ $auto$simplemap.cc:46:simplemap_not$1"));
    assert!(il.contains("\nautoidx 4\n"));

    // 未知旗標=Y0 硬錯。
    let out = ryosys()
        .arg("-p")
        .arg("read_rtlil t.il; simplemap -foo")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("milestone Y0"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_rtlil_default_form_is_y1() {
    // D-ys3:默認壓縮形(twines)硬錯,只承諾 -readable(read 先成功,
    // 確保錯在 write_rtlil 的旗標面而非前置命令)。
    let dir = std::env::temp_dir().join(format!("ry-cli-y1-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.il"), "module \\m\nend\n").unwrap();
    let out = ryosys()
        .arg("-p")
        .arg("read_rtlil a.il; write_rtlil out.il")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let se = String::from_utf8_lossy(&out.stderr);
    assert!(se.contains("milestone Y1"), "{se}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parser_error_on_stderr() {
    // §0-3 逐字:壞 .il(`wire` 後無 ID)→ ERROR: Parser error 在 stderr。
    let dir = std::env::temp_dir().join(format!("ry-cli-t-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("bad.il");
    std::fs::write(&bad, "module \\m\n  wire\nend\n").unwrap();
    let (rc, so, se) = run(&["-p", &format!("read_rtlil {}", bad.display())]);
    assert_eq!(rc, 1);
    assert!(se.starts_with("ERROR: Parser error in line 3"), "{se}");
    assert!(!so.contains("ERROR"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn happy_path_single_command_surface() {
    // 五命令縱切 smoke:tiny 設計過 read_rtlil→check→stat→write_rtlil
    // -readable→write_json,exit 0,兩產物形狀正確。
    let dir = std::env::temp_dir().join(format!("ry-cli-h-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let il = dir.join("case.il");
    std::fs::write(
        &il,
        "module \\top\n  wire input 1 \\a\n  wire output 1 \\y\n  connect \\y \\a\nend\n",
    )
    .unwrap();
    let script = "read_rtlil case.il; check; stat; write_rtlil -readable out.il; write_json out.json";
    let out = ryosys()
        .arg("-p")
        .arg(script)
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let so = String::from_utf8_lossy(&out.stdout);
    assert!(so.contains("1. Executing RTLIL frontend."));
    assert!(so.contains("2. Executing CHECK pass"));
    assert!(so.contains("3. Printing statistics."));
    assert!(so.contains("4. Executing RTLIL backend."));
    assert!(so.contains("5. Executing JSON backend."));
    assert!(so.contains("=== top ==="));
    let json = std::fs::read_to_string(dir.join("out.json")).unwrap();
    assert!(json.contains("\"modules\""));
    let rtlil = std::fs::read_to_string(dir.join("out.il")).unwrap();
    assert!(rtlil.contains("module \\top"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stdout_log_face_banner_and_headers() {
    // log 面:橫幅逐字回顯 -p 串;編號 header 順次;ERROR 分流到 stderr。
    let dir = std::env::temp_dir().join(format!("ry-cli-l-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = "read_rtlil x.il; check";
    let out = ryosys()
        .arg("-p")
        .arg(script)
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1)); // x.il 不存在 → 硬錯
    let so = String::from_utf8_lossy(&out.stdout);
    let se = String::from_utf8_lossy(&out.stderr);
    assert!(so.starts_with(&format!("-- Running command `{script}' --\n")));
    assert!(so.contains("1. Executing RTLIL frontend.\n"));
    assert!(!so.contains("ERROR"));
    assert!(se.starts_with("ERROR: "), "{se}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// `-p` 解析契約 fixture 組(§0-11;R2:暗角形先釘死再擴命令面)
// ---------------------------------------------------------------------------

use ry_cli::script::split_script;

fn cmds(s: &str) -> Vec<Vec<String>> {
    split_script(s).unwrap()
}

#[test]
fn p_split_no_space_semicolon_is_single_token() {
    // `clean;clean`(無空白)=單一未知命令(§0-11 實測)。
    assert_eq!(cmds("clean;clean"), vec![vec!["clean;clean".to_string()]]);
}

#[test]
fn p_split_space_semicolon_is_boundary() {
    assert_eq!(
        cmds("clean; clean"),
        vec![vec!["clean".to_string()], vec!["clean".to_string()]]
    );
}

#[test]
fn p_split_double_semicolon_appends_clean() {
    assert_eq!(
        cmds("a.il;; next"),
        vec![
            vec!["a.il".to_string()],
            vec!["clean".to_string()],
            vec!["next".to_string()]
        ]
    );
}

#[test]
fn p_split_triple_semicolon_appends_clean_purge() {
    assert_eq!(
        cmds("stat ;;; opt"),
        vec![
            vec!["stat".to_string()],
            vec!["clean".to_string(), "-purge".to_string()],
            vec!["opt".to_string()]
        ]
    );
}

#[test]
fn p_split_quad_semicolon_appends_nothing() {
    assert_eq!(
        cmds("a ;;;; b"),
        vec![vec!["a".to_string()], vec!["b".to_string()]]
    );
}

#[test]
fn p_split_hash_is_comment_to_eol() {
    assert_eq!(
        cmds("one # comment here\ntwo"),
        vec![vec!["one".to_string()], vec!["two".to_string()]]
    );
}

#[test]
fn p_split_quoted_semicolon_does_not_split() {
    // 引號內 `;` 不切;引號保留在 token 內(next_token 語義)。
    assert_eq!(
        cmds("log \"a;b\""),
        vec![vec!["log".to_string(), "\"a;b\"".to_string()]]
    );
}

#[test]
fn p_split_quoted_semicolon_before_delimiter_merges() {
    // io.cc:56-60 long_strings 分支:引號串後跟 `;`+分隔符=引號串連同
    // 尾分號作單 token 返回,再由 Pass::call 剝尾分號切分(§7.0-3 兩端
    // 實測探針 `-p '"a;b"; nosuch'` 同報逐字 No such command)。
    assert_eq!(
        cmds("\"a;b\"; nosuch"),
        vec![vec!["\"a;b\"".to_string()], vec!["nosuch".to_string()]]
    );
}

#[test]
fn quoted_semicolon_merge_cli_error_verbatim() {
    // CLI 面(§7.0-3 探針逐字):`"a;b";` 合併形 → 首命令 `"a;b"`(含
    // 引號)= 普查外名字 → oracle 的 No such command 錯誤面,ERROR 在
    // stderr、橫幅在 stdout。
    let (rc, so, se) = run(&["-p", "\"a;b\"; nosuch"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: No such command: \"a;b\" (type 'help' for a command overview)\n"
    );
    assert!(so.starts_with("-- Running command `\"a;b\"; nosuch' --\n"), "stdout={so:?}");
    assert!(!so.contains("ERROR"));
}

#[test]
fn p_split_newline_is_command_boundary() {
    assert_eq!(
        cmds("one\ntwo arg"),
        vec![vec!["one".to_string()], vec!["two".to_string(), "arg".to_string()]]
    );
}

#[test]
fn p_split_empty_and_shell_escape() {
    assert_eq!(cmds(""), Vec::<Vec<String>>::new());
    assert_eq!(cmds("   \n  "), Vec::<Vec<String>>::new());
    assert!(split_script("!ls -l").is_err());
    assert!(split_script("read_rtlil a.il").is_ok());
}

#[test]
fn p_split_trailing_args_flush() {
    assert_eq!(
        cmds("check; stat -width"),
        vec![
            vec!["check".to_string()],
            vec!["stat".to_string(), "-width".to_string()]
        ]
    );
}

// 讓 Write 引入不觸發 unused 警告(happy_path 的 Stdio 用 piping)。
#[allow(dead_code)]
fn _w(w: &mut std::fs::File) {
    let _ = w.write(b"");
}
