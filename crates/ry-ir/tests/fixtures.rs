//! 語法與錯誤路徑 fixtures(10.5 紀律:歸一化/詞法/錯誤形態由敵意單測釘死)。
//!
//! 錯誤訊息逐字對齊 oracle 實錄/源碼(§0-3:`ERROR: Parser error in line 3:
//! Missing wire ID`;其餘=rtlil_frontend.cc 的 error() 字面)。庫層 Display
//! 不含 `ERROR: ` 前綴(CLI 層 slice-2 補,stderr 分流在那層釘)。
//!
//! 行為錨(全部 2026-10-27 本機 oracle 0.70+179 實錄,見 /tmp probe 與
//! RYOSYS-MVP §0):常數補位/autoint/全 x 短寫、拼接 const 合併 wire 不合併、
//! 屬性同名首值保留、實體傾印分組序、autoidx 消耗語義。

use ry_ir::{parse_rtlil, write_rtlil_readable, RyError};

fn roundtrip(input: &str) -> String {
    write_rtlil_readable(&parse_rtlil(input).unwrap())
}

/// 剝首行(Generated-by 自標識)後比對。
fn body(input: &str) -> String {
    let out = roundtrip(input);
    out.splitn(2, '\n').nth(1).unwrap_or("").to_string()
}

fn err_of(input: &str) -> String {
    format!("{}", parse_rtlil(input).unwrap_err())
}

// ---------------------------------------------------------------------------
// 詞法:ID 逐字照抄(最易做錯處,RYOSYS-MVP §5 slice-1)
// ---------------------------------------------------------------------------

/// 複合類型 `$paramod\submod\c=s3'111\d=s3'111` 是**單 token**(ID 詞法
/// 吃到空白,`=`/`'` 都是名字字元)——§0-10 普查的 16 個複合名形。
#[test]
fn id_lexicon_paramod_compound_is_single_token() {
    let il = "module \\top\n  cell $paramod\\submod\\c=s32'00000000000000000000000000000001\\d=s32'11 \\u\n  end\nend\n";
    let out = body(il);
    assert!(out.contains("cell $paramod\\submod\\c=s32'00000000000000000000000000000001\\d=s32'11 \\u\n"), "複合類型須逐字保留: {out}");
}

/// 切片前必須有空格:`\bus[7:4]` 整體是 ID(名字可含 `[]:`)→
/// wire 查找失敗,逐字 `Wire `$bus[7:4]' not found.`。
#[test]
fn id_lexicon_slice_requires_space() {
    let il = "module \\top\n  wire width 8 \\bus\n  wire width 4 \\x\n  connect \\x \\bus[7:4]\nend\n";
    assert_eq!(err_of(il), "Parser error in line 4: Wire `\\bus[7:4]' not found.");
}

/// `end1` 匹配 keyword `end`(下一字元非 a-z)後在 EOL 報錯——
/// oracle try_parse_keyword 的已知暗角。
#[test]
fn keyword_match_trailing_char_rule() {
    let il = "module \\m\n  wire \\w\nend1\n";
    assert_eq!(err_of(il), "Parser error in line 3: Expected EOL, got `1'.");
}

/// 註釋行(# 起頭)跳過;`# driver …` 行(0.70 dump_wire 產物)同為註釋。
#[test]
fn comment_lines_skipped() {
    let il = "# a comment\nmodule \\m\n  # driver \\c \\p\n  wire \\w\n# another\nend\n";
    let out = body(il);
    assert!(out.contains("wire \\w\n"));
    assert!(!out.contains("driver"));
}

/// 空行跳過;行內註釋(#)使行剩餘視為空。
#[test]
fn blank_lines_skipped() {
    let il = "\n\nmodule \\m\n\n  wire \\w # trailing comment\n\nend\n";
    let out = body(il);
    assert!(out.contains("wire \\w\n"));
}

// ---------------------------------------------------------------------------
// 常數模型:位寬/x/z/補位/autoint
// ---------------------------------------------------------------------------

/// 補位規則(實測):最後一個位元補位,但 `1` 補 `0`;無位元補 x。
#[test]
fn const_padding_rules() {
    let out = body("module \\m\n  attribute \\a0 4'01\n  attribute \\ax 4'x\n  attribute \\az 3'z\n  attribute \\an 4'\n  attribute \\am 4'0m1-\n  wire \\w\nend\n");
    assert!(out.contains("attribute \\a0 4'0001\n"), "MSB(文字首字元)補位: {out}");
    assert!(out.contains("attribute \\ax 4'x\n"), "全 x 短寫: {out}");
    assert!(out.contains("attribute \\az 3'zzz\n"), "z 補 z(不短寫): {out}");
    assert!(out.contains("attribute \\an 4'x\n"), "無位元補 x→全 x 短寫: {out}");
    assert!(out.contains("attribute \\am 4'0m1-\n"), "m/- 逐字: {out}");
}

/// autoint(實測):32-bit 全定值且 val>=0 印十進制;最高位 1(負值)回退
/// 二進制形;寬度≠32 不 autoint。
#[test]
fn const_autoint_forms() {
    let out = body("module \\m\n  attribute \\p 32'00000000000000000000000000000101\n  attribute \\n -3\n  attribute \\w 40'1010\n  attribute \\s 3'101\n  wire \\w\nend\n");
    assert!(out.contains("attribute \\p 5\n"), "32-bit 正值→十進制: {out}");
    assert!(out.contains("attribute \\n 32'11111111111111111111111111111101\n"), "-3→二補數二進制形: {out}");
    assert!(out.contains("attribute \\w 40'0000000000000000000000000000000000001010\n"), "非 32-bit 不 autoint+補零: {out}");
    assert!(out.contains("attribute \\s 3'101\n"), "窄寬照印: {out}");
}

/// 零寬全-x:`0'x`(memwr priority_mask 常見形,實測 golden)。
#[test]
fn const_zero_width_x() {
    let out = body("module \\m\n  attribute \\z 0'x\n  wire \\w\nend\n");
    assert!(out.contains("attribute \\z 0'x\n"), "0'x 短寫: {out}");
}

/// signed:屬性域保留 `s`;sigspec 域即丟(§6-R7 實測 `4's0001`→`4'0001`)。
#[test]
fn signed_flag_domains() {
    let out = body("module \\m\n  wire width 4 \\a\n  attribute \\s 4's0011\n  wire \\w\n  connect \\a 4's0001\nend\n");
    assert!(out.contains("attribute \\s 4's0011\n"), "屬性域保留 s: {out}");
    assert!(out.contains("connect \\a 4'0001\n"), "sigspec 域丟 s: {out}");
}

/// cell 參數:signed 保留;unsized 印出時不帶 `N'` 前綴(實測)。
#[test]
fn parameter_flags() {
    let out = body("module \\m\n  cell \\t \\u\n    parameter signed \\S 3'101\n    parameter unsized \\U 1\n  end\nend\n");
    assert!(out.contains("parameter signed \\S 3's101\n"), "參數 signed 保留: {out}");
    assert!(out.contains("parameter unsized \\U 1\n"), "unsized: {out}");
}

/// 字串轉義 roundtrip:\n \t \041(octal) " \\(實測 dump_const 轉義表)。
#[test]
fn string_escape_roundtrip() {
    let out = body("module \\m\n  attribute \\s \"a\\nb\\tc\\041d\\\"e\\\\f\"\n  wire \\w\nend\n");
    assert!(out.contains("attribute \\s \"a\\nb\\tc!d\\\"e\\\\f\"\n"), "轉義逐字(\\041→! :dump 只轉義 <32,oracle 同): {out}");
}

// ---------------------------------------------------------------------------
// SigSpec:拼接合併語義(實測釘死)
// ---------------------------------------------------------------------------

/// 相鄰 const 合併;wire chunk 不合併(`{ \bus [0] \bus [1] \bus [3:2] }`
/// 保持三 chunk——物理序與值序相反)。
#[test]
fn concat_merge_rules() {
    let out = body("module \\m\n  wire width 8 \\bus\n  wire width 2 \\a\n  wire width 4 \\b\n  connect \\a { 1'0 1'1 }\n  connect \\b { \\bus [0] \\bus [1] \\bus [3:2] }\nend\n");
    assert!(out.contains("connect \\a 2'01\n"), "const 相鄰合併: {out}");
    assert!(out.contains("connect \\b { \\bus [0] \\bus [1] \\bus [3:2] }\n"), "wire 不合併: {out}");
}

/// 值序連續且物理 offset 遞增的 wire 拼接**會**合併(next_chunk_bits 規則;
/// oracle append 的 CHUNK 快路徑等價形)。
#[test]
fn concat_wire_contiguous_merges() {
    let out = body("module \\m\n  wire width 8 \\bus\n  wire width 4 \\a\n  connect \\a { \\bus [7] \\bus [6:4] }\nend\n");
    assert!(out.contains("connect \\a \\bus [7:4]\n"), "物理連續拼接合併: {out}");
}

/// 切片形態:全寬→裸名;1 位→`[n]`;多位→`[hi:lo]`(實測)。
#[test]
fn slice_dump_forms() {
    let out = body("module \\m\n  wire width 8 \\bus\n  wire width 8 \\a\n  wire \\b\n  wire width 4 \\c\n  connect \\a \\bus\n  connect \\b \\bus [2]\n  connect \\c \\bus [7:4]\nend\n");
    assert!(out.contains("connect \\a \\bus\n"), "全寬裸名: {out}");
    assert!(out.contains("connect \\b \\bus [2]\n"), "單位切片: {out}");
    assert!(out.contains("connect \\c \\bus [7:4]\n"), "區間切片: {out}");
}

// ---------------------------------------------------------------------------
// 傾印語義:插入序、分組、屬性 insert 首值保留
// ---------------------------------------------------------------------------

/// 實體傾印分組序:wires→memories→cells→processes→connections(實測
/// cell 在 connect 前,即使宣告在後);組內插入序。
#[test]
fn dump_grouping_order() {
    let il = "module \\m\n  wire \\a\n  connect \\a 1'0\n  cell \\t \\u\n  end\n  memory width 2 size 3 \\mem\n  process \\p\n  end\nend\n";
    let out = body(il);
    let wire = out.find("wire \\a").unwrap();
    let mem = out.find("memory width 2 size 3 \\mem").unwrap();
    let cell = out.find("cell \\t \\u").unwrap();
    let proc = out.find("process \\p").unwrap();
    let conn = out.find("connect \\a 1'0").unwrap();
    assert!(wire < mem && mem < cell && cell < proc && proc < conn, "分組序: {out}");
}

/// 屬性同名=首值保留、位置保留(oracle dict::insert 語義,實測
/// `\a 1/\b 2/\a 3` 回寫 `attribute \a 1`)。
#[test]
fn attribute_insert_keeps_first() {
    let out = body("module \\m\n  attribute \\a 1\n  attribute \\b 2\n  attribute \\a 3\n  wire \\w\nend\n");
    assert!(out.contains("attribute \\a 1\n") && out.contains("attribute \\b 2\n"), "首值保留: {out}");
    assert!(!out.contains("attribute \\a 3"), "後值不覆蓋: {out}");
}

/// autoidx:parse 取 max;read 恰好消耗一個 id(等效 max(parse, 1),實測
/// `autoidx 0`→1、無行→1、`0`+`5`→5、三模組無行→仍 1)。
#[test]
fn autoidx_semantics() {
    assert!(body("autoidx 0\nmodule \\m\nend\n").starts_with("autoidx 1\n"));
    assert!(body("module \\m\nend\n").starts_with("autoidx 1\n"));
    assert!(body("autoidx 0\nautoidx 5\nmodule \\m\nend\n").starts_with("autoidx 5\n"));
    assert!(body("autoidx 477\nmodule \\m\nend\n").starts_with("autoidx 477\n"));
}

/// fixup_ports:埠按 (port_id, name) 重編 1..N;非埠 port_id 清零
/// (kernel/rtlil.cc:3124;規範形=恆等)。
#[test]
fn fixup_ports_renumber() {
    let out = body("module \\m\n  wire input 5 \\b\n  wire input 2 \\a\n  wire \\x\nend\n");
    // 傾印=宣告序(\b 先),重編後 \a=1、\b=2(按 (port_id,name) 排序)。
    assert!(out.contains("wire input 2 \\b\n"), "重編: {out}");
    assert!(out.contains("wire input 1 \\a\n"), "重編: {out}");
}

/// 模組級 parameter:無值形+有值形;同名首值保留。
#[test]
fn module_parameters() {
    let out = body("module \\m\n  parameter \\p\n  parameter \\q 5\n  parameter \\q 7\n  wire \\w\nend\n");
    assert!(out.contains("  parameter \\p\n"), "無值形: {out}");
    assert!(out.contains("  parameter \\q 5\n"), "有值形+首值保留: {out}");
}

// ---------------------------------------------------------------------------
// 錯誤路徑:逐字(oracle 實錄 §0-3 或源碼字面)
// ---------------------------------------------------------------------------

#[test]
fn err_missing_wire_id() {
    // §0-3 oracle 實錄逐字(壞 .il 的 `wire` 後無 ID)。
    let il = "module \\m\n  wire\n  wire \\a\nend\n";
    assert_eq!(err_of(il), "Parser error in line 3: Missing wire ID");
}

#[test]
fn err_missing_memory_id() {
    let il = "module \\m\n  memory\n  wire \\a\nend\n";
    assert_eq!(err_of(il), "Parser error in line 3: Missing memory ID");
}

#[test]
fn err_dangling_attribute_top() {
    let il = "attribute \\x 1\n";
    assert_eq!(err_of(il), "Parser error in line 2: dangling attribute");
}

#[test]
fn err_dangling_attribute_module_end() {
    let il = "module \\m\n  attribute \\x 1\nend\n";
    assert_eq!(err_of(il), "Parser error in line 4: dangling attribute");
}

#[test]
fn err_dangling_attribute_connect() {
    let il = "module \\m\n  wire \\a\n  attribute \\x 1\n  connect \\a 1'0\nend\n";
    assert_eq!(err_of(il), "Parser error in line 4: dangling attribute");
}

#[test]
fn err_wire_not_found() {
    let il = "module \\m\n  wire \\a\n  connect \\a \\nosuch\nend\n";
    assert_eq!(err_of(il), "Parser error in line 3: Wire `\\nosuch' not found.");
}

#[test]
fn err_bit_index_out_of_range() {
    let il = "module \\m\n  wire width 4 \\a\n  connect \\a 1'0 [4]\nend\n";
    assert_eq!(err_of(il), "Parser error in line 3: bit index 4 out of range");
}

#[test]
fn err_invalid_slice() {
    // left 不越界才走到 slice 檢查(oracle 實錄:[0:2]=invalid slice)。
    let il = "module \\m\n  wire width 4 \\a\n  connect \\a 1'0 [0:2]\nend\n";
    assert_eq!(err_of(il), "Parser error in line 3: invalid slice [0:2]");
}

#[test]
fn err_slice_left_range_checked_first() {
    // left 越界先報(oracle 實錄:[1:2] 對 1-bit sig=bit index 1 out of range)。
    let il = "module \\m\n  wire width 4 \\a\n  connect \\a 1'0 [1:2]\nend\n";
    assert_eq!(err_of(il), "Parser error in line 3: bit index 1 out of range");
}

#[test]
fn err_redefinition_wire() {
    let il = "module \\m\n  wire \\a\n  wire \\a\nend\n";
    assert_eq!(err_of(il), "Parser error in line 3: RTLIL error: redefinition of wire \\a.");
}

#[test]
fn err_redefinition_cell() {
    let il = "module \\m\n  cell \\t \\u\n  end\n  cell \\t \\u\n  end\nend\n";
    assert_eq!(err_of(il), "Parser error in line 5: RTLIL error: redefinition of cell \\u.");
}

#[test]
fn err_redefinition_cell_port() {
    let il = "module \\m\n  wire \\a\n  wire \\b\n  cell \\t \\u\n    connect \\p \\a\n    connect \\p \\b\n  end\nend\n";
    assert_eq!(err_of(il), "Parser error in line 6: RTLIL error: redefinition of cell port \\p.");
}

#[test]
fn err_redefinition_memory() {
    let il = "module \\m\n  memory \\z\n  memory \\z\nend\n";
    assert_eq!(err_of(il), "Parser error in line 3: RTLIL error: redefinition of memory \\z.");
}

#[test]
fn err_redefinition_process() {
    let il = "module \\m\n  process \\p\n  end\n  process \\p\n  end\nend\n";
    assert_eq!(err_of(il), "Parser error in line 5: RTLIL error: redefinition of process \\p.");
}

#[test]
fn err_redefinition_module() {
    let il = "module \\m\nend\nmodule \\m\nend\n";
    assert_eq!(err_of(il), "Parser error in line 4: RTLIL error: redefinition of module \\m.");
}

/// blackbox 重定義=忽略(oracle parse_module 語義)。
#[test]
fn blackbox_redefinition_ignored() {
    let il = "module \\m\nend\nattribute \\blackbox 1\nmodule \\m\nend\n";
    let out = body(il);
    assert_eq!(out.matches("module \\m").count(), 1, "blackbox 重定義被忽略: {out}");
}

#[test]
fn err_unexpected_token_module_body() {
    let il = "module \\m\n  bogus \\x\nend\n";
    assert_eq!(err_of(il), "Parser error in line 2: Unexpected token in module body: bogus");
}

#[test]
fn err_unexpected_token_cell_body() {
    let il = "module \\m\n  cell \\t \\u\n    bogus\n  end\nend\n";
    assert_eq!(err_of(il), "Parser error in line 3: Unexpected token in cell body: bogus");
}

#[test]
fn err_unexpected_token_top() {
    let il = "bogus\n";
    assert_eq!(err_of(il), "Parser error in line 1: Unexpected token: bogus");
}

#[test]
fn err_unexpected_wire_option() {
    let il = "module \\m\n  wire width 4 signedx \\a\nend\n";
    assert_eq!(err_of(il), "Parser error in line 2: Unexpected wire option: signedx");
}

#[test]
fn err_unexpected_sync_type() {
    let il = "module \\m\n  process \\p\n  sync bogus\n  end\nend\n";
    assert_eq!(err_of(il), "Parser error in line 3: Unexpected sync type: bogus");
}

#[test]
fn err_invalid_integer() {
    let il = "autoidx x\nmodule \\m\nend\n";
    assert_eq!(err_of(il), "Parser error in line 1: Invalid integer `x'.");
}

#[test]
fn err_constant_width_out_of_range() {
    let il = "module \\m\n  attribute \\a 99999999999'0\n  wire \\w\nend\n";
    assert_eq!(
        err_of(il),
        "Parser error in line 2: Constant width 99999999999 out of range before `'0`."
    );
}

#[test]
fn err_unterminated_string() {
    let il = "module \\m\n  attribute \\a \"abc\n  wire \\w\nend\n";
    assert_eq!(err_of(il), "Parser error in line 2: Unterminated string literal: `\"abc'.");
}

#[test]
fn err_connect_width_mismatch() {
    // oracle 此輸入是 log_assert 崩潰(實測 rtlil.cc:2995);本 crate 以 Err
    // 拒絕同樣非法的輸入,訊息文本自訂(不冒充 assert 文本)。
    let il = "module \\m\n  wire width 2 \\a\n  connect \\a 1'0\nend\n";
    assert_eq!(err_of(il), "Parser error in line 3: connect operands have different sizes (2 vs 1)");
}

#[test]
fn err_expected_id() {
    let il = "module \\m\n  cell \\t\nend\n";
    assert_eq!(err_of(il), "Parser error in line 2: Expected twine reference or ID, got `'.");
}

#[test]
fn err_expected_eol() {
    let il = "module \\m extra\nend\n";
    assert_eq!(err_of(il), "Parser error in line 1: Expected EOL, got `extra'.");
}

// ---------------------------------------------------------------------------
// milestone Y1 硬錯(RYOSYS-MVP §1.3;鐵則 3:未支援=硬錯,禁靜默跑偏)
// ---------------------------------------------------------------------------

#[test]
fn y1_twines_block() {
    let il = "twines\nmodule \\m\nend\n";
    assert_eq!(
        err_of(il),
        "Parser error in line 1: `twines' block is not supported in ryosys (milestone Y1)"
    );
}

#[test]
fn y1_priv_at_token() {
    let il = "module \\m\n  wire $priv@99\nend\n";
    assert_eq!(
        err_of(il),
        "Parser error in line 2: `$priv@' name token is not supported in ryosys (milestone Y1)"
    );
}

#[test]
fn y1_pub_at_token() {
    let il = "module \\m\n  wire $pub@7\nend\n";
    assert_eq!(
        err_of(il),
        "Parser error in line 2: `$pub@' name token is not supported in ryosys (milestone Y1)"
    );
}

#[test]
fn y1_parameter_real() {
    let il = "module \\m\n  cell \\t \\u\n    parameter real \\R 1\n  end\nend\n";
    assert_eq!(
        err_of(il),
        "Parser error in line 3: `parameter real' is not supported in ryosys (milestone Y1)"
    );
}

#[test]
fn y1_string_const_in_sigspec() {
    let il = "module \\m\n  wire width 8 \\a\n  connect \\a \"abc\"\nend\n";
    assert_eq!(
        err_of(il),
        "Parser error in line 3: string constant in sigspec is not supported in ryosys (milestone Y1)"
    );
}

/// RyError::Unsupported 可程式化分類(DECL 分類器 grep `milestone Y`,
/// RYOSYS-MVP §3)。
#[test]
fn unsupported_error_is_classifiable() {
    let e = parse_rtlil("twines\n").unwrap_err();
    match e {
        RyError::Unsupported { milestone, .. } => assert_eq!(milestone, "Y1"),
        other => panic!("應為 Unsupported: {other}"),
    }
}

// ---------------------------------------------------------------------------
// process 區塊:屬性歸屬+sync/update/memwr(小樣;大覆蓋在 golden 層)
// ---------------------------------------------------------------------------

#[test]
fn process_block_shapes() {
    let il = "module \\m\n  wire \\clk\n  wire width 2 \\q\n  process \\p\n    switch \\clk\n      case 1'1\n        assign \\q 2'11\n      case \n    end\n    sync posedge \\clk\n      update \\q \\q\n    sync always\n    sync init\n  end\nend\n";
    let out = body(il);
    assert!(out.contains("process \\p\n"), "{out}");
    assert!(out.contains("case 1'1\n"), "{out}");
    assert!(out.contains("case \n"), "空 case 尾隨空格: {out}");
    assert!(out.contains("sync posedge \\clk\n"), "{out}");
    assert!(out.contains("sync always\n"), "{out}");
    assert!(out.contains("sync init\n"), "{out}");
    assert!(out.contains("update \\q \\q\n"), "{out}");
}

/// sync 屬性穿透:memwr 吃 attrbuf;update 清單尾的懸掛屬性=硬錯
/// (oracle 「carry through the sync」語義)。
#[test]
fn sync_dangling_attribute() {
    let il = "module \\m\n  wire \\c\n  process \\p\n  sync always\n    attribute \\x 1\n  end\nend\n";
    assert_eq!(err_of(il), "Parser error in line 6: dangling attribute");
}

/// 屬性掛 switch(在 switch 行前)與 case(case 行前)——實測 golden 形態。
#[test]
fn switch_case_attributes() {
    let il = "module \\m\n  wire \\s\n  process \\p\n    attribute \\sw 1\n    switch \\s\n      attribute \\cs 2\n      case 1'1\n      case \n    end\n  end\nend\n";
    let out = body(il);
    assert!(out.contains("attribute \\sw 1\n    switch \\s\n"), "屬性掛 switch: {out}");
    assert!(out.contains("attribute \\cs 2\n      case 1'1\n"), "屬性掛 case: {out}");
}
