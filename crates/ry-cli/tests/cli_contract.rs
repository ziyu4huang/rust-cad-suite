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
    // Y 碼映射(§1.2;M2 後 proc 族/opt_expr 已兌現實現層,鐵則 3 同輪
    // 刪三條 Y2 fixture;read_verilog 族→Y3;其餘→Y0。
    for (cmd, y) in [
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
fn proc_and_opt_expr_keepdc_flip_pass() {
    // M2 翻轉契約(鐵則 3 同輪新增):proc 無參形與 opt_expr 僅 -keepdc
    // 單旗標=實現層(sync edge 拒收契約在 proc_stub_in_flip_script;
    // byte-exact 面由 ys-e2e -pb 層 golden 承擔)。
    let dir = std::env::temp_dir().join(format!("ry-cli-proc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        &dir.join("case.il"),
        "module \\top\n  wire input 1 \\d\n  wire output 2 \\q\n  process \\$p\n    assign \\q \\d\n    sync always\n      update \\q \\d\n  end\nend\n",
    )
    .unwrap();
    let out = ryosys()
        .arg("-p")
        .arg("read_rtlil case.il; proc; check; write_rtlil -readable o.il")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let so = String::from_utf8_lossy(&out.stdout);
    assert!(so.contains("Executing PROC pass (convert processes to netlists).\n"));
    assert!(so.contains("12. Executing OPT_EXPR pass (perform const folding).\n"));
    let il = std::fs::read_to_string(dir.join("o.il")).unwrap();
    assert!(il.contains("connect \\q \\d"));

    // opt_expr 其餘旗標=Y2 硬錯(詳案 §8.4)。
    let out = ryosys()
        .arg("-p")
        .arg("read_rtlil case.il; opt_expr -full")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("milestone Y2"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn empty_design_three_commands_rc0() {
    // D-ys6 清償(§9.0-P9 母本;oracle 空設計 stat/check/hierarchy rc=0):
    // * stat → `1. Printing statistics.` 無統計段
    // * check → header+`Found and reported 0 problems.`
    // * hierarchy → header 後無 sub-header
    // 自創無設計硬錯文案已全刪(驗收 7 負向閘;rg 字面量 exit=1)。
    for (cmd, want) in [
        ("stat", "1. Printing statistics.\n"),
        (
            "check",
            "1. Executing CHECK pass (checking for obvious problems).\nFound and reported 0 problems.\n",
        ),
        (
            "hierarchy",
            "1. Executing HIERARCHY pass (managing design hierarchy).\n",
        ),
    ] {
        let (rc, so, se) = run(&["-p", cmd]);
        assert_eq!(rc, 0, "{cmd}");
        assert_eq!(se, "", "{cmd}");
        let body: String = so
            .lines()
            .skip_while(|l| !l.starts_with("-- Running command"))
            .skip(1)
            .take_while(|l| !l.starts_with("End of script"))
            .filter(|l| !l.is_empty())
            .map(|l| format!("{l}\n"))
            .collect();
        assert_eq!(body, want, "{cmd}");
    }
}

#[test]
fn opt_family_flag_y2_probes() {
    // M3 slice-1 探针 fixture(R2 先釘死再擴命令面;RYOSYS-MVP §9.2):
    // opt 編排器旗標面與子 pass standalone 旗標面=Y2 硬錯(全文斷言取
    // contains 形——stub 期 `opt: not implemented` 與翻轉後 option 形
    // `opt: option `-fast' is not implemented` 兩態皆命中,鐵則 3 同輪
    // 更新由各 slice 消費)。
    for cmd in [
        "opt -fast",
        "opt -full",
        "opt -fine",
        "opt -keepdc",
        "opt -purge",
        "opt -noff",
        "opt -hier",
        "opt_clean -purge",
        "opt_merge -nomux",
        "opt_merge -share_all",
        "opt_reduce -fine",
        "opt_reduce -full",
        "opt_dff -nodffe",
        "opt_dff -nosdff",
        "opt_dff -sat",
        // opt_expr 6 新旗标(续弧维持硬错;验收 8:硬错裁决=fixture 含旗标 token)
        "opt_expr -mux_undef",
        "opt_expr -mux_bool",
        "opt_expr -undriven",
        "opt_expr -noclkinv",
        "opt_expr -fine",
        "opt_expr -full",
    ] {
        let (rc, _, se) = run(&["-p", cmd]);
        let first = cmd.split(' ').next().unwrap();
        assert_eq!(rc, 1, "{cmd}");
        assert!(
            se.contains("milestone Y2") && se.starts_with(&format!("ERROR: {first}")),
            "{cmd}: {se}"
        );
    }
}

// ---------------------------------------------------------------------------
// M3 续弧·批量收编 slice-1 探针 fixture 先钉(R2 先钉再翻;RYOSYS-MVP §10.1):
// 四单元 clean/log/help/opt_balance_tree 现行=stub 硬错,本轮以 stub 形钉死;
// 各 slice 翻转时同轮更新断言为翻轉后形态(铁则 3:能力变更同轮)。
// ---------------------------------------------------------------------------

#[test]
fn clean_cmd_forms() {
    // slice-3 翻转(铁则 3 同轮):无参/-purge=实现(rc=0、零输出、编号
    // 不占;P1);-foo=cmd_error 全形(oracle 实录形两端一致,stderr 三行
    // +stdout Syntax 块;byte-exact=PROVENANCE M3 续弧节实录)。
    for cmd in ["clean", "clean -purge", "clean -purge -purge"] {
        let (rc, so, se) = run(&["-p", cmd]);
        assert_eq!(rc, 0, "{cmd}");
        assert_eq!(se, "", "{cmd}");
        assert_eq!(so, format!("-- Running command `{cmd}' --\n\n"), "{cmd}");
    }
    let (rc, so, se) = run(&["-p", "clean -foo"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> clean -foo\n>       ^\n"
    );
    assert!(so.contains("Syntax error in command `clean -foo':"), "{so}");
    assert!(so.contains("    clean [options] [selection]"), "{so}");
    assert!(
        so.contains("This is identical to 'opt_clean', but less verbose."),
        "{so}"
    );
}

#[test]
fn clean_cmd_selection_divergence() {
    // D-ys10(§10.1-1/§10.2):oracle 经 extra_args(select=true 默认)
    // 接受 selection 并正常执行 rc=0;我方=cmd_error rc=1=真实行为面
    // 分歧,登记 docs/REGISTRY.md。fixture 钉我方 cmd_error 全文断言形
    // (实录形与我方硬错永不相等,禁用 oracle 实录 golden)。
    let (rc, _, se) = run(&["-p", "clean carryadd/w:*"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> clean carryadd/w:*\n>       ^\n"
    );
}

#[test]
fn triple_semicolon_end_to_end_clean_purge() {
    // `;;;` 端到端(§10.1-1;token 层 fixture=p_split_triple_semicolon_
    // appends_clean_purge 已在):clean -purge 真执行且不占编号
    // (两 CHECK header 编号连续 2./3.,锚=ys-m3-anchors/
    // semicolon-end-to-end byte-exact)。
    let dir = std::env::temp_dir().join(format!("ry-cli-sc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        &dir.join("t.il"),
        "module \\top\n  wire width 4 input 1 \\a\n  wire width 4 \\t\n  wire width 4 output 2 \\y\n  cell $and $u\n    parameter \\A_SIGNED 0\n    parameter \\B_SIGNED 0\n    parameter \\A_WIDTH 4\n    parameter \\B_WIDTH 4\n    parameter \\Y_WIDTH 4\n    connect \\A \\a\n    connect \\B \\a\n    connect \\Y \\t\n  end\n  connect \\y \\a\nend\n",
    )
    .unwrap();
    let out = ryosys()
        .arg("-p")
        .arg("read_rtlil t.il; check ;;; check")
        .current_dir(&dir)
        .output()
        .unwrap();
    let rc = out.status.code().unwrap();
    let so = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(rc, 0);
    assert!(so.contains("2. Executing CHECK pass"), "{so}");
    assert!(so.contains("3. Executing CHECK pass"), "{so}");
    assert!(so.contains("Removed 1 unused cells and 1 unused wires."), "{so}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn log_flag_stdout_y2_error() {
    // slice-2 翻转(铁则 3 同轮):七已知旗标=Y2 硬错(oracle 真实行为
    // =旗标语义执行,行为面歧登记 D-ys10;P3 七形实录 PROVENANCE)。
    let (rc, _, se) = run(&["-p", "log -stdout"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: log: option `-stdout' is not implemented in ryosys (milestone Y2)\n"
    );
}

#[test]
fn log_flag_stderr_y2_error() {
    let (rc, _, se) = run(&["-p", "log -stderr"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: log: option `-stderr' is not implemented in ryosys (milestone Y2)\n"
    );
}

#[test]
fn log_flag_nolog_y2_error() {
    let (rc, _, se) = run(&["-p", "log -nolog"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: log: option `-nolog' is not implemented in ryosys (milestone Y2)\n"
    );
}

#[test]
fn log_flag_n_y2_error() {
    let (rc, _, se) = run(&["-p", "log -n"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: log: option `-n' is not implemented in ryosys (milestone Y2)\n"
    );
}

#[test]
fn log_flag_header_y2_error() {
    let (rc, _, se) = run(&["-p", "log -header"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: log: option `-header' is not implemented in ryosys (milestone Y2)\n"
    );
}

#[test]
fn log_flag_push_y2_error() {
    let (rc, _, se) = run(&["-p", "log -push"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: log: option `-push' is not implemented in ryosys (milestone Y2)\n"
    );
}

#[test]
fn log_flag_pop_y2_error() {
    let (rc, _, se) = run(&["-p", "log -pop"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: log: option `-pop' is not implemented in ryosys (milestone Y2)\n"
    );
}

#[test]
fn log_unknown_token_prints_verbatim() {
    // P3 oracle 亲证(免费对齐):未知 -token=原样拼接打印 rc=0,
    // 防「把 - 开头 token 误当旗标报错」的实现走偏(§10.1-2)。
    let (rc, so, se) = run(&["-p", "log -foo"]);
    assert_eq!(rc, 0);
    assert_eq!(se, "");
    assert_eq!(so, "-- Running command `log -foo' --\n-foo\n\n");
}

#[test]
fn log_quoted_semicolon_end_to_end() {
    // 引号端到端(§10.1-2;token 层 fixture=p_split_quoted_semicolon_
    // does_not_split 已在):`log "a;b"` 命令真执行,引号保留原样打印
    // (§0-11 引号契约的执行载体)。
    let (rc, so, se) = run(&["-p", "log \"a;b\""]);
    assert_eq!(rc, 0);
    assert_eq!(se, "");
    assert_eq!(so, "-- Running command `log \"a;b\"' --\n\"a;b\"\n\n");
}

#[test]
fn help_bare_312_line_listing() {
    // slice-2 翻转:bare help=横幅+空行+306 名单(`    %-20s %s`)+
    // 空行+两行 Type+空行=312 行(归一化形;oracle 原形 323 行=P5,
    // 差 11 行=driver 横幅 8+epilogue 3)。与 oracle -H 剥面对拍零差
    // (免费对拍闸,PROVENANCE M3 续弧节实录)。
    let (rc, so, se) = run(&["-p", "help"]);
    assert_eq!(rc, 0);
    assert_eq!(se, "");
    let lines: Vec<&str> = so.split('\n').collect();
    assert_eq!(lines.len(), 313, "312 lines + trailing empty");
    assert_eq!(lines[0], "-- Running command `help' --");
    assert_eq!(lines[1], "");
    assert_eq!(
        lines[2],
        "    abc                  use ABC for technology mapping"
    );
    assert_eq!(
        lines[307],
        "    zinit                add inverters so all FF are zero-initialized"
    );
    assert_eq!(lines[308], "");
    assert_eq!(
        lines[309],
        "Type 'help <command>' for more information on a command."
    );
    assert_eq!(
        lines[310],
        "Type 'help -cells' for a list of all cell types."
    );
    assert_eq!(lines[311], "");
}

#[test]
fn help_subform_y2_error_probes() {
    // slice-2 翻转:子形(<名>/未知名/-foo/两参/-all/-cells)=Y2 硬错
    // (D-ys10:oracle 未知名 rc=0 log 形=分歧本体,P5/P6)。
    for cmd in [
        "help nosuchpass",
        "help -foo",
        "help opt clean",
        "help -all",
        "help -cells",
    ] {
        let (rc, _, se) = run(&["-p", cmd]);
        assert_eq!(rc, 1, "{cmd}");
        assert!(
            se.contains("milestone Y2") && se.starts_with("ERROR: help"),
            "{cmd}: {se}"
        );
    }
}

#[test]
fn opt_balance_tree_arith_logic_y2_errors() {
    // slice-4 翻转(铁则 3 同轮):无参形=实现(carryadd/multiplier 锚
    // byte-exact);-arith/-logic=option 形 Y2 硬错(D-ys10:oracle 真实
    // 型別置換,§10.2 边界)。
    let (rc, _, _) = run(&["-p", "opt_balance_tree"]);
    assert_eq!(rc, 0);
    for cmd in ["opt_balance_tree -arith", "opt_balance_tree -logic"] {
        let (rc, _, se) = run(&["-p", cmd]);
        assert_eq!(rc, 1, "{cmd}");
        assert_eq!(
            se,
            format!(
                "ERROR: opt_balance_tree: option `{}' is not implemented in ryosys (milestone Y2)\n",
                cmd.split(' ').nth(1).unwrap()
            ),
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

// ---------------------------------------------------------------------------
// M2 翻轉腳本契約(RYOSYS-MVP §8.5-slice-1;先釘死再擴命令面,R2)
// ---------------------------------------------------------------------------

/// 翻轉腳本逐字(§8.2-P2;slice-5 的 SCRIPT_PB 常量同串)。
const SCRIPT_PB: &str = "read_rtlil case.il; proc; check; stat; write_rtlil -readable expected.rtlil; write_json expected.json";

#[test]
fn proc_flip_script_split_contract() {
    // 翻轉腳本在 -p 解析契約下切 6 命令、token 逐字(token 尾分號切分,
    // §0-11)——slice-5 的 ry-difftest SCRIPT_PB 常量與 ys-m2-anchors
    // script.txt 同串,解析面先於命令面釘死。
    assert_eq!(
        cmds(SCRIPT_PB),
        vec![
            vec!["read_rtlil".to_string(), "case.il".to_string()],
            vec!["proc".to_string()],
            vec!["check".to_string()],
            vec!["stat".to_string()],
            vec![
                "write_rtlil".to_string(),
                "-readable".to_string(),
                "expected.rtlil".to_string(),
            ],
            vec!["write_json".to_string(), "expected.json".to_string()],
        ]
    );
}

#[test]
fn proc_stub_in_flip_script() {
    // 翻轉後契約(鐵則 3 同輪改寫):翻轉腳本對 sync edge 形(P7-① 自寫
    // 最小案例)產 proc_dff 的逐字拒收 ERROR(exit 1、ERROR 走 stderr;
    // ys-m2-anchors/edge_reject golden 同源形)。
    let dir = std::env::temp_dir().join(format!("ry-cli-pb-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        &dir.join("case.il"),
        "module \\top\n  wire width 4 input 1 \\d\n  wire width 4 output 2 \\q\n  wire input 3 \\c\n  process \\$proc\n    assign \\q \\d\n    sync edge \\c\n      update \\q \\d\n  end\nend\n",
    )
    .unwrap();
    let out = ryosys()
        .arg("-p")
        .arg(SCRIPT_PB)
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let se = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        se,
        "ERROR: Event with any-edge sensitivity found for this signal!\n"
    );
    let so = String::from_utf8_lossy(&out.stdout);
    assert!(so.starts_with(&format!("-- Running command `{SCRIPT_PB}' --\n")));
    assert!(!so.contains("ERROR"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// M3 续弧·批量收编第二批 slice-1 探针 fixture 先钉(R2 先钉再翻;
// RYOSYS-MVP §11.4):六命令 async2sync/pmuxtree/dffunmap/setattr/torder/
// edgetypes 现行=registry stub Y0 硬错,本轮以 stub 形钉死;各 slice 翻转
// 时同轮更新断言为翻轉后形态(铁则 3:能力变更同轮)。
// * 未知旗标形翻转目标=cmd_error 全形(oracle 实录形=stderr 三行
//   `Command syntax error…`+`> 串`+`> 空格^`,stdout=header(有 header
//   面)+`Syntax error in command …`+help 全文;六份 help 常量已落
//   help_desc.rs);
// * selection 形(裸字参数)翻转目标=我方 cmd_error(D-ys11:oracle 经
//   extra_args 接受 rc=0=行为面歧,本轮 stub 期=Y0);
// * 已知旗标形翻转目标=Y2 文案(async2sync -nolower/dffunmap 两旗标含
//   双投互斥形/torder 两旗标,§11.1-8);
// * setattr decode 错误形暂钉 stub 行为,slice-6 翻转为 oracle 逐字
//   `ERROR: Can't decode value 'true'!`(rc=1)。
// ---------------------------------------------------------------------------

#[test]
fn batch2_six_stub_bare_forms_y0() {
    // slice-3 翻转(铁则 3 同轮):async2sync 无参形=实现层(空设计=
    // header 即止 rc=0,oracle 空跑亲证);余五命令维持 stub Y0。
    let (rc, so, se) = run(&["-p", "async2sync"]);
    assert_eq!(rc, 0);
    assert_eq!(se, "");
    assert_eq!(
        so,
        "-- Running command `async2sync' --\n\n1. Executing ASYNC2SYNC pass.\n\n"
    );
    let (rc, so, se) = run(&["-p", "pmuxtree"]);
    assert_eq!(rc, 0);
    assert_eq!(se, "");
    assert_eq!(
        so,
        "-- Running command `pmuxtree' --\n\n1. Executing PMUXTREE pass.\n\n"
    );
    let (rc, so, se) = run(&["-p", "dffunmap"]);
    assert_eq!(rc, 0);
    assert_eq!(se, "");
    assert_eq!(
        so,
        "-- Running command `dffunmap' --\n\n1. Executing DFFUNMAP pass (unmap clock enable and synchronous reset from FFs).\n\n"
    );
    // slice-6 翻转(铁则 3 同轮):setattr 无参形=实现层(执行体静默,
    // 零 header 不占编号=oracle sa1.log 形)。
    let (rc, so, se) = run(&["-p", "setattr"]);
    assert_eq!(rc, 0);
    assert_eq!(se, "");
    assert_eq!(so, "-- Running command `setattr' --\n\n");
    // slice-7 翻转(铁则 3 同轮):torder/edgetypes 无参形=实现层。
    let (rc, so, se) = run(&["-p", "torder"]);
    assert_eq!(rc, 0);
    assert_eq!(se, "");
    assert_eq!(
        so,
        "-- Running command `torder' --\n\n1. Executing TORDER pass (print cells in topological order).\n\n"
    );
    let (rc, so, se) = run(&["-p", "edgetypes"]);
    assert_eq!(rc, 0);
    assert_eq!(se, "");
    assert_eq!(so, "-- Running command `edgetypes' --\n\n");
}

#[test]
fn batch2_six_stub_flag_forms_y0() {
    // slice-3 翻转(铁则 3 同轮):async2sync -foo=cmd_error 全形(oracle
    // 实录形=stderr 三行+stdout header+Syntax 块+help 全文);-nolower=
    // Y2 文案;余五命令 stub 期一律 Y0(命中 registry 表,旗标不分已知
    // 未知);翻转 slice 同轮按三形分流(未知=cmd_error 全形/已知=Y2)。
    let (rc, so, se) = run(&["-p", "async2sync -foo"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> async2sync -foo\n>            ^\n"
    );
    assert!(
        so.starts_with("-- Running command `async2sync -foo' --\n\n1. Executing ASYNC2SYNC pass.\n\nSyntax error in command `async2sync -foo':\n"),
        "{so}"
    );
    assert!(so.contains("    async2sync [options] [selection]"), "{so}");
    assert!(so.contains("Do not automatically run 'chformal -lower'"), "{so}");
    let (rc, _, se) = run(&["-p", "async2sync -nolower"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: async2sync: option `-nolower' is not implemented in ryosys (milestone Y2)\n"
    );
    let (rc, so, se) = run(&["-p", "pmuxtree -foo"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> pmuxtree -foo\n>          ^\n"
    );
    assert!(
        so.starts_with("-- Running command `pmuxtree -foo' --\n\n1. Executing PMUXTREE pass.\n\nSyntax error in command `pmuxtree -foo':\n"),
        "{so}"
    );
    assert!(so.contains("    pmuxtree [selection]"), "{so}");
    let (rc, so, se) = run(&["-p", "dffunmap -foo"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> dffunmap -foo\n>          ^\n"
    );
    assert!(
        so.starts_with("-- Running command `dffunmap -foo' --\n\n1. Executing DFFUNMAP pass (unmap clock enable and synchronous reset from FFs).\n\nSyntax error in command `dffunmap -foo':\n"),
        "{so}"
    );
    assert!(so.contains("    dffunmap [options] [selection]"), "{so}");
    for cmd in ["dffunmap -ce-only", "dffunmap -srst-only", "dffunmap -ce-only -srst-only"] {
        let (rc, _, se) = run(&["-p", cmd]);
        let f = cmd.split(' ').nth(1).unwrap();
        assert_eq!(rc, 1, "{cmd}");
        assert_eq!(
            se,
            format!("ERROR: dffunmap: option `{f}' is not implemented in ryosys (milestone Y2)\n"),
            "{cmd}"
        );
    }
    // 余五命令已全部翻转,stub 列表清空(六命令=实现层;铁则 3 收口)。
}

#[test]
fn batch2_six_stub_selection_forms_y0() {
    // selection 裸字形(D-ys11 面):oracle 经 extra_args 接受 rc=0
    // (本轮亲证 async2sync MyMem/* rc=0);我方=cmd_error rc=1=行为面
    // 分歧已登记。slice-3 翻转:async2sync=cmd_error 全文断言形;余五
    // 命令 stub 期=Y0。
    let (rc, _, se) = run(&["-p", "async2sync top/*"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> async2sync top/*\n>            ^\n"
    );
    let (rc, _, se) = run(&["-p", "pmuxtree top/*"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> pmuxtree top/*\n>          ^\n"
    );
    let (rc, _, se) = run(&["-p", "dffunmap top/*"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> dffunmap top/*\n>          ^\n"
    );
    let (rc, so, se) = run(&["-p", "setattr -foo"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> setattr -foo\n>         ^\n"
    );
    assert!(
        so.starts_with("-- Running command `setattr -foo' --\n\nSyntax error in command `setattr -foo':\n"),
        "{so}"
    );
    assert!(
        so.contains("    setattr [ -mod ] [ -set name value | -unset name ]... [selection]"),
        "{so}"
    );
    // decode 错误形(slice-6 翻转;oracle 逐字=stderr ERROR+rc=1,stdout
    // 仅横幅;setq 锚 golden=corpus/ys-batch2-anchors/setattr-decode-err)。
    let (rc, so, se) = run(&["-p", "setattr -set src_sync true"]);
    assert_eq!(rc, 1);
    assert_eq!(se, "ERROR: Can't decode value 'true'!\n");
    assert_eq!(so, "-- Running command `setattr -set src_sync true' --\n");
    let (rc, _, se) = run(&["-p", "setattr top/*"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> setattr top/*\n>         ^\n"
    );
    let (rc, _, se) = run(&["-p", "torder -stop"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: torder: option `-stop' is not implemented in ryosys (milestone Y2)\n"
    );
    let (rc, _, se) = run(&["-p", "torder -noautostop"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: torder: option `-noautostop' is not implemented in ryosys (milestone Y2)\n"
    );
    let (rc, _, se) = run(&["-p", "torder -stop $and Y"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: torder: option `-stop' is not implemented in ryosys (milestone Y2)\n"
    );
    let (rc, _, se) = run(&["-p", "torder -foo"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> torder -foo\n>        ^\n"
    );
    let (rc, _, se) = run(&["-p", "torder top/*"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> torder top/*\n>        ^\n"
    );
    let (rc, _, se) = run(&["-p", "edgetypes -foo"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> edgetypes -foo\n>           ^\n"
    );
    let (rc, _, se) = run(&["-p", "edgetypes top/*"]);
    assert_eq!(rc, 1);
    assert_eq!(
        se,
        "ERROR: Command syntax error: Unknown option or option in arguments.\n> edgetypes top/*\n>           ^\n"
    );

}

// 讓 Write 引入不觸發 unused 警告(happy_path 的 Stdio 用 piping)。
#[allow(dead_code)]
fn _w(w: &mut std::fs::File) {
    let _ = w.write(b"");
}
