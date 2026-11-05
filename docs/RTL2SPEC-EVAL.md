# rtl2spec 真實評測擴面報告(§16.4;機械生成禁手改)

- 日期:2026-11-03
- 命令行:`bash plugin/rtl2spec/run-realworld-eval.sh`(機械生成禁手改)
- 工具鏈:bun v26.3.0、wasm ABI 0.1.0、wasm sha256 `4b143d3afd22c41f85a0c2cebc23edd0c0176e3eb0f2ae88849fc4c8d855f3cd`
- 耗時:107ms(單 bun 進程循環 353 檔);語料零 GPL(yosys-simple/sv-tests=ISC、corpus=repo MIT、spec-i2c=MIT 乾淨室、xls=Apache-2.0 載體)
- 判據鏈:per-file 三檔分類(design-rules/design-sparse/skip-perr/skip-tb);閘級硬 FAIL=trap/JSON 不可解析/design-rules 檔 V4/V5-①②④ 敗/V5-③ 指紋重複(L1+L2 全量)/成功呼叫≠檔數。V1(≥3)/V2/V3 為報告列不設單檔 FAIL(擴面集失敗條件=結構性違例而非產量);13 檔驗證集 V1..V6 閾值閘另行執法不動(run-rtl-validation.sh)。

## 閘級判據表

| 判據 | 實測 | 判定 |
|---|---|---|
| trap(終止性防線=F2 修復+trap 計數,R16-3) | 0 | PASS |
| design-rules 檔 V4+V5-①②④ | 37/37 檔全過(checker 同形獨立重算) | PASS |
| V5-③ 指紋去重(L1+L2 全量) | 672 條規則,重複=0 | PASS |
| V6 覆蓋面(成功呼叫=檔數) | 353/353 | PASS |
| parse_err 如實計數(L2 已知事實記錄,不 FAIL) | skip-perr=193 檔 | INFO |

閘級判定:**PASS**

## 分層統計表

| 層 | 檔數 | design-rules | design-sparse | skip-perr | skip-tb |
|---|---|---|---|---|---|
| L1 質量層 | 93 | 37 | 55 | 0 | 1 |
| L2 容錯層 | 260 | 0 | 62 | 193 | 5 |
| 全集 | 353 | 37 | 117 | 193 | 6 |

## L1 per-file 表(93 檔全列)

| 檔名 | license | 行數 | 規則數 | 類別數 | 破口數(按類) | 判定 |
|---|---|---|---|---|---|---|
| corpus-external/yosys-simple/aes_kexp128.v | ISC(COPYING 隨附) | 24 | 4 | 1 | missing_reset=1 | PASS |
| corpus-external/yosys-simple/always01.v | ISC(COPYING 隨附) | 11 | 1 | 1 | missing_reset=1 | OK-sparse |
| corpus-external/yosys-simple/always02.v | ISC(COPYING 隨附) | 14 | 2 | 2 | missing_reset=1 | OK-sparse |
| corpus-external/yosys-simple/always03.v | ISC(COPYING 隨附) | 23 | 7 | 1 | missing_reset=1 | PASS |
| corpus-external/yosys-simple/arraycells.v | ISC(COPYING 隨附) | 15 | 1 | 1 | 0 | OK-sparse |
| corpus-external/yosys-simple/arrays01.v | ISC(COPYING 隨附) | 17 | 2 | 2 | missing_reset=1 | OK-sparse |
| corpus-external/yosys-simple/attrib01_module.v | ISC(COPYING 隨附) | 21 | 2 | 2 | 0 | OK-sparse |
| corpus-external/yosys-simple/attrib02_port_decl.v | ISC(COPYING 隨附) | 25 | 2 | 2 | 0 | OK-sparse |
| corpus-external/yosys-simple/attrib03_parameter.v | ISC(COPYING 隨附) | 28 | 2 | 2 | 0 | OK-sparse |
| corpus-external/yosys-simple/attrib04_net_var.v | ISC(COPYING 隨附) | 32 | 4 | 4 | missing_reset=1 | PASS |
| corpus-external/yosys-simple/attrib06_operator_suffix.v | ISC(COPYING 隨附) | 23 | 2 | 2 | 0 | OK-sparse |
| corpus-external/yosys-simple/attrib08_mod_inst.v | ISC(COPYING 隨附) | 22 | 2 | 2 | 0 | OK-sparse |
| corpus-external/yosys-simple/attrib09_case.v | ISC(COPYING 隨附) | 26 | 0 | 0 | incomplete_case=1 | OK-sparse |
| corpus-external/yosys-simple/carryadd.v | ISC(COPYING 隨附) | 25 | 4 | 1 | 0 | PASS |
| corpus-external/yosys-simple/case_expr_const.v | ISC(COPYING 隨附) | 50 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/case_expr_non_const.v | ISC(COPYING 隨附) | 60 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/case_large.v | ISC(COPYING 隨附) | 274 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/const_branch_finish.v | ISC(COPYING 隨附) | 37 | 0 | 0 | 0 | SKIP |
| corpus-external/yosys-simple/const_fold_func.v | ISC(COPYING 隨附) | 62 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/const_func_shadow.v | ISC(COPYING 隨附) | 34 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/constmuldivmod.v | ISC(COPYING 隨附) | 68 | 46 | 1 | 0 | PASS |
| corpus-external/yosys-simple/constpower.v | ISC(COPYING 隨附) | 16 | 2 | 1 | 0 | OK-sparse |
| corpus-external/yosys-simple/dff_different_styles.v | ISC(COPYING 隨附) | 104 | 17 | 2 | missing_reset=3 | PASS |
| corpus-external/yosys-simple/dff_init.v | ISC(COPYING 隨附) | 55 | 5 | 1 | missing_reset=5 multi_driver=5 | PASS |
| corpus-external/yosys-simple/dynslice.v | ISC(COPYING 隨附) | 13 | 1 | 1 | missing_reset=1 | OK-sparse |
| corpus-external/yosys-simple/fiedler-cooley.v | ISC(COPYING 隨附) | 34 | 11 | 1 | missing_reset=1 | PASS |
| corpus-external/yosys-simple/forgen01.v | ISC(COPYING 隨附) | 24 | 0 | 0 | undriven_read=1 | OK-sparse |
| corpus-external/yosys-simple/forgen02.v | ISC(COPYING 隨附) | 31 | 3 | 1 | comb_loop=2 | PASS |
| corpus-external/yosys-simple/forloops.v | ISC(COPYING 隨附) | 26 | 7 | 2 | undriven_read=2 missing_reset=1 | PASS |
| corpus-external/yosys-simple/fsm.v | ISC(COPYING 隨附) | 69 | 18 | 3 | incomplete_case=1 missing_reset=1 | PASS |
| corpus-external/yosys-simple/func_block.v | ISC(COPYING 隨附) | 34 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/func_recurse.v | ISC(COPYING 隨附) | 26 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/func_width_scope.v | ISC(COPYING 隨附) | 42 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/genblk_collide.v | ISC(COPYING 隨附) | 28 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/genblk_dive.v | ISC(COPYING 隨附) | 22 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/genblk_order.v | ISC(COPYING 隨附) | 19 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/genblk_port_shadow.v | ISC(COPYING 隨附) | 11 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/generate.v | ISC(COPYING 隨附) | 326 | 15 | 2 | missing_reset=2 undriven_read=1 | PASS |
| corpus-external/yosys-simple/graphtest.v | ISC(COPYING 隨附) | 35 | 2 | 1 | 0 | OK-sparse |
| corpus-external/yosys-simple/hierarchy.v | ISC(COPYING 隨附) | 27 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/hierdefparam.v | ISC(COPYING 隨附) | 26 | 1 | 1 | comb_loop=1 | OK-sparse |
| corpus-external/yosys-simple/i2c_master_tests.v | ISC(COPYING 隨附) | 62 | 9 | 3 | missing_reset=3 | PASS |
| corpus-external/yosys-simple/ifdef_1.v | ISC(COPYING 隨附) | 89 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/ifdef_2.v | ISC(COPYING 隨附) | 22 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/localparam_attr.v | ISC(COPYING 隨附) | 12 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/loop_prefix_case.v | ISC(COPYING 隨附) | 19 | 0 | 0 | incomplete_case=1 | OK-sparse |
| corpus-external/yosys-simple/loop_var_shadow.v | ISC(COPYING 隨附) | 16 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/loops.v | ISC(COPYING 隨附) | 79 | 4 | 1 | latch=3 | PASS |
| corpus-external/yosys-simple/macro_arg_surrounding_spaces.v | ISC(COPYING 隨附) | 21 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/macros.v | ISC(COPYING 隨附) | 245 | 5 | 1 | comb_loop=1 | PASS |
| corpus-external/yosys-simple/mem_arst.v | ISC(COPYING 隨附) | 41 | 4 | 1 | undriven_read=1 missing_reset=1 | PASS |
| corpus-external/yosys-simple/mem2reg_bounds_tern.v | ISC(COPYING 隨附) | 20 | 2 | 1 | missing_reset=2 | OK-sparse |
| corpus-external/yosys-simple/mem2reg.v | ISC(COPYING 隨附) | 117 | 5 | 3 | undriven_read=1 missing_reset=1 | PASS |
| corpus-external/yosys-simple/memory.v | ISC(COPYING 隨附) | 320 | 59 | 2 | missing_reset=15 undriven_read=4 multi_driver=1 | PASS |
| corpus-external/yosys-simple/module_scope_case.v | ISC(COPYING 隨附) | 12 | 0 | 0 | incomplete_case=1 | OK-sparse |
| corpus-external/yosys-simple/module_scope_func.v | ISC(COPYING 隨附) | 46 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/module_scope.v | ISC(COPYING 隨附) | 30 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/multiplier.v | ISC(COPYING 隨附) | 133 | 10 | 1 | 0 | PASS |
| corpus-external/yosys-simple/muxtree.v | ISC(COPYING 隨附) | 83 | 6 | 2 | incomplete_case=1 | PASS |
| corpus-external/yosys-simple/named_genblk.v | ISC(COPYING 隨附) | 28 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/nested_genblk_resolve.v | ISC(COPYING 隨附) | 15 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/omsp_dbg_uart.v | ISC(COPYING 隨附) | 34 | 4 | 3 | 0 | PASS |
| corpus-external/yosys-simple/operators.v | ISC(COPYING 隨附) | 122 | 85 | 1 | incomplete_case=1 missing_reset=1 | PASS |
| corpus-external/yosys-simple/param_attr.v | ISC(COPYING 隨附) | 12 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/paramods.v | ISC(COPYING 隨附) | 53 | 1 | 1 | 0 | OK-sparse |
| corpus-external/yosys-simple/partsel.v | ISC(COPYING 隨附) | 152 | 39 | 3 | incomplete_case=1 missing_reset=2 | PASS |
| corpus-external/yosys-simple/process.v | ISC(COPYING 隨附) | 84 | 13 | 3 | incomplete_case=2 | PASS |
| corpus-external/yosys-simple/realexpr.v | ISC(COPYING 隨附) | 34 | 7 | 1 | 0 | PASS |
| corpus-external/yosys-simple/repwhile.v | ISC(COPYING 隨附) | 37 | 0 | 0 | undriven_read=2 | OK-sparse |
| corpus-external/yosys-simple/retime.v | ISC(COPYING 隨附) | 7 | 1 | 1 | missing_reset=1 multi_driver=1 | OK-sparse |
| corpus-external/yosys-simple/rotate.v | ISC(COPYING 隨附) | 48 | 1 | 1 | undriven_read=1 comb_loop=1 | OK-sparse |
| corpus-external/yosys-simple/scopes.v | ISC(COPYING 隨附) | 64 | 1 | 1 | comb_loop=4 | OK-sparse |
| corpus-external/yosys-simple/sign_part_assign.v | ISC(COPYING 隨附) | 21 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/signed_full_slice.v | ISC(COPYING 隨附) | 30 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/signedexpr.v | ISC(COPYING 隨附) | 19 | 6 | 1 | 0 | PASS |
| corpus-external/yosys-simple/sincos.v | ISC(COPYING 隨附) | 125 | 46 | 3 | incomplete_case=1 missing_reset=3 | PASS |
| corpus-external/yosys-simple/specify.v | ISC(COPYING 隨附) | 31 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/string_format.v | ISC(COPYING 隨附) | 8 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/subbytes.v | ISC(COPYING 隨附) | 83 | 7 | 3 | latch=1 | PASS |
| corpus-external/yosys-simple/task_func.v | ISC(COPYING 隨附) | 142 | 6 | 1 | missing_reset=3 | PASS |
| corpus-external/yosys-simple/undef_eqx_nex.v | ISC(COPYING 隨附) | 12 | 8 | 1 | 0 | PASS |
| corpus-external/yosys-simple/usb_phy_tests.v | ISC(COPYING 隨附) | 36 | 1 | 1 | incomplete_case=1 missing_reset=1 | OK-sparse |
| corpus-external/yosys-simple/values.v | ISC(COPYING 隨附) | 44 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/verilog_primitives.v | ISC(COPYING 隨附) | 16 | 0 | 0 | 0 | OK-sparse |
| corpus-external/yosys-simple/vloghammer.v | ISC(COPYING 隨附) | 82 | 6 | 1 | 0 | PASS |
| corpus-external/yosys-simple/wandwor.v | ISC(COPYING 隨附) | 37 | 2 | 1 | 0 | OK-sparse |
| corpus-external/yosys-simple/wreduce.v | ISC(COPYING 隨附) | 10 | 6 | 1 | 0 | PASS |
| corpus/counter8/counter.v | repo MIT | 19 | 2 | 2 | 0 | OK-sparse |
| corpus/e2e/57_xls_add3/add3.v | Apache-2.0(xls 派生;repo MIT 載體) | 34 | 6 | 2 | missing_reset=2 | PASS |
| corpus/e2e/58_xls_acc_pipe/acc_pipe.v | Apache-2.0(xls 派生;repo MIT 載體) | 45 | 7 | 2 | missing_reset=3 | PASS |
| corpus/e2e/59_i2c_master/i2c_master.v | repo MIT | 317 | 69 | 4 | 0 | PASS |
| corpus/fulladd/fulladd.v | repo MIT | 26 | 3 | 1 | 0 | PASS |
| crates/spec-i2c/rtl/i2c_master.v | repo MIT(乾淨室;OpenCores 唯讀啟發零拷貝) | 315 | 69 | 4 | 0 | PASS |

## L2 容錯層(260 檔;分檔統計+parse_err 清單,不逐檔列表)

- 可終止+零 trap+JSON 可解析:260/260 檔(trap=0)
- parse_err(SV-2009 構造超 parser 閘,已知事實記錄不 FAIL):193 檔
- 非 parse_err 判定分布:design-rules=0、design-sparse=62、skip-tb=5

<details><summary>parse_err 檔名清單(機械列出)</summary>

- corpus-external/sv-tests/12.4.2--priority_if.sv(2 err)
- corpus-external/sv-tests/12.4.2--unique_if.sv(2 err)
- corpus-external/sv-tests/12.4.2--unique0_if.sv(2 err)
- corpus-external/sv-tests/12.5.4--case_set.sv(4 err)
- corpus-external/sv-tests/12.6.1--case_pattern.sv(17 err)
- corpus-external/sv-tests/12.6.1--casex_pattern.sv(17 err)
- corpus-external/sv-tests/12.6.1--casez_pattern.sv(17 err)
- corpus-external/sv-tests/12.6.2--if_pattern.sv(10 err)
- corpus-external/sv-tests/12.6.3--conditional_pattern.sv(15 err)
- corpus-external/sv-tests/12.7.1--for.sv(5 err)
- corpus-external/sv-tests/12.7.2--repeat.sv(1 err)
- corpus-external/sv-tests/12.7.3--foreach-synth.sv(5 err)
- corpus-external/sv-tests/12.7.3--foreach.sv(3 err)
- corpus-external/sv-tests/12.7.4--while.sv(4 err)
- corpus-external/sv-tests/12.7.5--dowhile.sv(7 err)
- corpus-external/sv-tests/12.8--break.sv(4 err)
- corpus-external/sv-tests/12.8--continue.sv(5 err)
- corpus-external/sv-tests/12.8--return_val.sv(8 err)
- corpus-external/sv-tests/12.8--return.sv(11 err)
- corpus-external/sv-tests/5.10-structure-arrays-illegal.sv(5 err)
- corpus-external/sv-tests/5.10-structure-arrays.sv(7 err)
- corpus-external/sv-tests/5.10-structure-replication.sv(20 err)
- corpus-external/sv-tests/5.10-structures.sv(16 err)
- corpus-external/sv-tests/5.11-arrays-key-index.sv(3 err)
- corpus-external/sv-tests/5.11-arrays-replication.sv(3 err)
- corpus-external/sv-tests/5.11-arrays.sv(4 err)
- corpus-external/sv-tests/5.12-attributes-case.sv(1 err)
- corpus-external/sv-tests/5.12-attributes-conditional.sv(1 err)
- corpus-external/sv-tests/5.12-attributes-operator.sv(3 err)
- corpus-external/sv-tests/5.12-attributes-variable.sv(3 err)
- corpus-external/sv-tests/5.13-builtin-methods-strings.sv(1 err)
- corpus-external/sv-tests/5.6--wrong-identifiers.sv(2 err)
- corpus-external/sv-tests/5.6.4--compiler-directives-debug.sv(1 err)
- corpus-external/sv-tests/5.6.4--compiler-directives-define.sv(1 err)
- corpus-external/sv-tests/5.6.4--compiler-directives-include.sv(1 err)
- corpus-external/sv-tests/5.6.4--compiler-directives-preprocessor-macro_0.sv(1 err)
- corpus-external/sv-tests/5.6.4--compiler-directives-preprocessor-macro_1.sv(3 err)
- corpus-external/sv-tests/5.7.1--integers-left-padding-bit.sv(1 err)
- corpus-external/sv-tests/5.7.1--integers-left-padding.sv(2 err)
- corpus-external/sv-tests/5.7.1--integers-signed-illegal.sv(2 err)
- corpus-external/sv-tests/5.7.1--integers-signed.sv(4 err)
- corpus-external/sv-tests/5.7.1--integers-sized.sv(5 err)
- corpus-external/sv-tests/5.7.1--integers-underscores.sv(3 err)
- corpus-external/sv-tests/5.7.1--integers-unsized-illegal.sv(2 err)
- corpus-external/sv-tests/5.7.1--integers-unsized.sv(1 err)
- corpus-external/sv-tests/5.7.2-real-constants-illegal.sv(5 err)
- corpus-external/sv-tests/5.7.2-real-constants.sv(1 err)
- corpus-external/sv-tests/5.8-time-literals.sv(7 err)
- corpus-external/sv-tests/5.9-string-assignment.sv(3 err)
- corpus-external/sv-tests/5.9-string-word-assignment.sv(2 err)
- corpus-external/sv-tests/6.12--shortreal.sv(1 err)
- corpus-external/sv-tests/6.13--void.sv(3 err)
- corpus-external/sv-tests/6.14--chandle.sv(1 err)
- corpus-external/sv-tests/6.16--string.sv(1 err)
- corpus-external/sv-tests/6.16.1--string_len.sv(2 err)
- corpus-external/sv-tests/6.16.10--string_atoreal.sv(1 err)
- corpus-external/sv-tests/6.16.11--string_itoa.sv(1 err)
- corpus-external/sv-tests/6.16.12--string_hextoa.sv(1 err)
- corpus-external/sv-tests/6.16.13--string_octtoa.sv(1 err)
- corpus-external/sv-tests/6.16.14--string_bintoa.sv(1 err)
- corpus-external/sv-tests/6.16.15--string_realtoa.sv(1 err)
- corpus-external/sv-tests/6.16.2--string_putc.sv(1 err)
- corpus-external/sv-tests/6.16.3--string_getc.sv(2 err)
- corpus-external/sv-tests/6.16.4--string_toupper.sv(2 err)
- corpus-external/sv-tests/6.16.5--string_tolower.sv(2 err)
- corpus-external/sv-tests/6.16.6--string_compare.sv(3 err)
- corpus-external/sv-tests/6.16.7--string_icompare.sv(3 err)
- corpus-external/sv-tests/6.16.8--string_substr.sv(2 err)
- corpus-external/sv-tests/6.16.9--string_atobin.sv(2 err)
- corpus-external/sv-tests/6.16.9--string_atohex.sv(2 err)
- corpus-external/sv-tests/6.16.9--string_atoi.sv(2 err)
- corpus-external/sv-tests/6.16.9--string_atooct.sv(2 err)
- corpus-external/sv-tests/6.18--typedef.sv(2 err)
- corpus-external/sv-tests/6.19--enum_anon.sv(1 err)
- corpus-external/sv-tests/6.19--enum_value_inv.sv(1 err)
- corpus-external/sv-tests/6.19--enum_xx_inv_order.sv(1 err)
- corpus-external/sv-tests/6.19--enum_xx_inv.sv(1 err)
- corpus-external/sv-tests/6.19--enum_xx.sv(1 err)
- corpus-external/sv-tests/6.19.1--enum_typedef.sv(2 err)
- corpus-external/sv-tests/6.19.2--enum_sequence_range.sv(1 err)
- corpus-external/sv-tests/6.19.2--enum_sequence.sv(1 err)
- corpus-external/sv-tests/6.19.3--enum_type_checking_inv.sv(2 err)
- corpus-external/sv-tests/6.19.3--enum_type_checking.sv(2 err)
- corpus-external/sv-tests/6.19.4--enum_numerical_expr_cast.sv(6 err)
- corpus-external/sv-tests/6.19.4--enum_numerical_expr_no_cast.sv(3 err)
- corpus-external/sv-tests/6.19.4--enum_numerical_expr.sv(2 err)
- corpus-external/sv-tests/6.19.5.1--enum_first.sv(2 err)
- corpus-external/sv-tests/6.19.5.2--enum_last.sv(2 err)
- corpus-external/sv-tests/6.19.5.3--enum_next.sv(2 err)
- corpus-external/sv-tests/6.19.5.4--enum_prev.sv(2 err)
- corpus-external/sv-tests/6.19.5.5--enum_num.sv(3 err)
- corpus-external/sv-tests/6.19.5.6--enum_name.sv(3 err)
- corpus-external/sv-tests/6.20.2--parameter_aggregate.sv(2 err)
- corpus-external/sv-tests/6.20.3--parameter_type.sv(1 err)
- corpus-external/sv-tests/6.20.4--localparam_int.sv(1 err)
- corpus-external/sv-tests/6.20.4--localparam_logic.sv(1 err)
- corpus-external/sv-tests/6.20.4--localparam_string.sv(1 err)
- corpus-external/sv-tests/6.20.4--localparam_unsigned_int.sv(1 err)
- corpus-external/sv-tests/6.20.6--const.sv(7 err)
- corpus-external/sv-tests/6.23--localparam_type_decl.sv(2 err)
- corpus-external/sv-tests/6.23--type_op.sv(1 err)
- corpus-external/sv-tests/6.24.1--cast_op.sv(2 err)
- corpus-external/sv-tests/6.24.2--cast_fn.sv(1 err)
- corpus-external/sv-tests/6.24.2--cast_task.sv(1 err)
- corpus-external/sv-tests/6.24.3--bitstream_cast.sv(6 err)
- corpus-external/sv-tests/6.5--variable_assignment.sv(1 err)
- corpus-external/sv-tests/6.5--variable_mixed_assignments.sv(1 err)
- corpus-external/sv-tests/6.5--variable_multiple_assignments.sv(1 err)
- corpus-external/sv-tests/6.6.7--nettype_resolution_fn.sv(5 err)
- corpus-external/sv-tests/6.6.7--nettype.sv(1 err)
- corpus-external/sv-tests/6.6.8--interconnect.sv(1 err)
- corpus-external/sv-tests/6.9.1--logic_vector.sv(1 err)
- corpus-external/sv-tests/6.9.2--vector_scalared.sv(1 err)
- corpus-external/sv-tests/6.9.2--vector_vectored_inv.sv(1 err)
- corpus-external/sv-tests/6.9.2--vector_vectored.sv(1 err)
- corpus-external/sv-tests/8.10--static_methods.sv(9 err)
- corpus-external/sv-tests/8.11--this.sv(7 err)
- corpus-external/sv-tests/8.12--assignment.sv(8 err)
- corpus-external/sv-tests/8.12--shallow_copy.sv(9 err)
- corpus-external/sv-tests/8.13--inheritance.sv(16 err)
- corpus-external/sv-tests/8.14--override_member.sv(21 err)
- corpus-external/sv-tests/8.15--super-default-new.sv(3 err)
- corpus-external/sv-tests/8.15--super.sv(21 err)
- corpus-external/sv-tests/8.16--cast_func.sv(2 err)
- corpus-external/sv-tests/8.17--constructor_const_arg.sv(11 err)
- corpus-external/sv-tests/8.18--var_local.sv(3 err)
- corpus-external/sv-tests/8.18--var_protected.sv(3 err)
- corpus-external/sv-tests/8.19--global_constant.sv(3 err)
- corpus-external/sv-tests/8.19--instance_constant.sv(6 err)
- corpus-external/sv-tests/8.20--virtual_method.sv(14 err)
- corpus-external/sv-tests/8.21--abstract_class_inst.sv(10 err)
- corpus-external/sv-tests/8.21--abstract_class.sv(10 err)
- corpus-external/sv-tests/8.22--dynamic_method_lookup.sv(25 err)
- corpus-external/sv-tests/8.23--scope_resolution.sv(14 err)
- corpus-external/sv-tests/8.24--out_of_block_methods.sv(10 err)
- corpus-external/sv-tests/8.25--parametrized_class_extend.sv(5 err)
- corpus-external/sv-tests/8.25.1--parametrized_class_invalid_scope_resolution.sv(6 err)
- corpus-external/sv-tests/8.25.1--parametrized_class_scope_resolution.sv(6 err)
- corpus-external/sv-tests/8.26.2--implements_extends.sv(17 err)
- corpus-external/sv-tests/8.26.2--implements_multiple.sv(16 err)
- corpus-external/sv-tests/8.26.2--implements.sv(9 err)
- corpus-external/sv-tests/8.26.3--type_access_extends.sv(8 err)
- corpus-external/sv-tests/8.26.3--type_access_implements_invalid.sv(10 err)
- corpus-external/sv-tests/8.26.3--type_access_implements.sv(10 err)
- corpus-external/sv-tests/8.26.4--illegal_forward_def_implements.sv(11 err)
- corpus-external/sv-tests/8.26.4--illegal_implements_parameter.sv(10 err)
- corpus-external/sv-tests/8.26.5--cast_between_interface_classes.sv(18 err)
- corpus-external/sv-tests/8.26.5--implemented_class_handle.sv(10 err)
- corpus-external/sv-tests/8.26.5--invalid_interface_instantiation.sv(4 err)
- corpus-external/sv-tests/8.26.6.1--name_conflict_resolved.sv(12 err)
- corpus-external/sv-tests/8.26.6.1--name_conflict_unresolved.sv(12 err)
- corpus-external/sv-tests/8.26.6.2--parameter_type_conflict_unresolved.sv(8 err)
- corpus-external/sv-tests/8.26.6.2--parameter_type_conflict.sv(9 err)
- corpus-external/sv-tests/8.26.6.3--diamond_relationship_parametrized.sv(13 err)
- corpus-external/sv-tests/8.26.6.3--diamond_relationship.sv(13 err)
- corpus-external/sv-tests/8.26.7--partial_implementation.sv(18 err)
- corpus-external/sv-tests/8.27--forward_declaration.sv(6 err)
- corpus-external/sv-tests/8.4--instantiation.sv(3 err)
- corpus-external/sv-tests/8.5--parameters.sv(2 err)
- corpus-external/sv-tests/8.5--properties_enum.sv(3 err)
- corpus-external/sv-tests/8.5--properties.sv(3 err)
- corpus-external/sv-tests/8.6--methods.sv(7 err)
- corpus-external/sv-tests/8.7--constructor_param.sv(7 err)
- corpus-external/sv-tests/8.7--constructor_super.sv(12 err)
- corpus-external/sv-tests/8.7--constructor.sv(4 err)
- corpus-external/sv-tests/8.8--typed_constructor_param.sv(13 err)
- corpus-external/sv-tests/8.8--typed_constructor.sv(13 err)
- corpus-external/sv-tests/8.9--static_properties.sv(4 err)
- corpus-external/sv-tests/9.2.2.1--always.sv(1 err)
- corpus-external/sv-tests/9.2.2.2--always_comb.sv(1 err)
- corpus-external/sv-tests/9.2.2.3--always_latch.sv(1 err)
- corpus-external/sv-tests/9.2.2.4--always_ff.sv(1 err)
- corpus-external/sv-tests/9.2.3--final.sv(1 err)
- corpus-external/sv-tests/9.3.2--parallel_block_join_any.sv(1 err)
- corpus-external/sv-tests/9.3.2--parallel_block_join_none.sv(1 err)
- corpus-external/sv-tests/9.3.3--fork_return.sv(1 err)
- corpus-external/sv-tests/9.3.4--block_names_par.sv(1 err)
- corpus-external/sv-tests/9.3.5--statement_labels_par.sv(2 err)
- corpus-external/sv-tests/9.3.5--statement_labels_seq.sv(4 err)
- corpus-external/sv-tests/9.4.2--event_control_edge.sv(1 err)
- corpus-external/sv-tests/9.4.2.3--event_conditional.sv(1 err)
- corpus-external/sv-tests/9.4.2.4--event_sequence.sv(8 err)
- corpus-external/sv-tests/9.4.5--event_nonblocking_assignment_event.sv(1 err)
- corpus-external/sv-tests/9.4.5--event_nonblocking_assignment_repeat_int_neg.sv(2 err)
- corpus-external/sv-tests/9.4.5--event_nonblocking_assignment_repeat_int.sv(2 err)
- corpus-external/sv-tests/9.4.5--event_nonblocking_assignment_repeat_neg.sv(1 err)
- corpus-external/sv-tests/9.4.5--event_nonblocking_assignment_repeat.sv(1 err)
- corpus-external/sv-tests/9.6.1--wait_fork.sv(1 err)
- corpus-external/sv-tests/9.6.3--disable_fork.sv(1 err)
- corpus-external/sv-tests/9.7--process_cls_await.sv(8 err)
- corpus-external/sv-tests/9.7--process_cls_kill.sv(9 err)
- corpus-external/sv-tests/9.7--process_cls_self.sv(7 err)
- corpus-external/sv-tests/9.7--process_cls_suspend_resume.sv(11 err)

</details>

## 類別分布表(L1 全集規則×類別,九類+param/structure;R4 複評素材)

| 類別 | 檔數 | 規則數 |
|---|---|---|
| arithmetic | 29 | 172 |
| counter | 10 | 25 |
| fsm | 3 | 23 |
| reset | 20 | 92 |
| timing | 36 | 351 |
| **Σ** | — | **663** |

## 評審員區(R5 分層抽樣 5 檔定性複評;人工區,機械內聯自 eval-review-notes.md)

# R5 擴面複評筆記(§16.5;人工區,gen-eval-report.mjs 機械內聯進 RTL2SPEC-EVAL.md)

- 評審流程契約(§16.5):每檔探針題×3,先**只讀規則書**(build/eval verdict
  內嵌 rulebook JSON)作答、答案引用 rule id,後**粗讀 RTL 源檔**對照。
- provenance 注明:本複評=會話內評審(同會話讀規則書與源碼),非獨立第三人
  (S7 先例);R2 散文感抽查同批完成(見檔尾)。
- 抽樣(§16.5 分層 5 檔):頭部 1=operators(85 條 timing 密度面)/中部 2=
  fsm(18,fsm+counter)+multiplier(10,arithmetic+G3 誤報史)/尾部 2=
  subbytes(7,G1 對沖正樣+R007 when 適用域先例)+acc_pipe(7,G4+Apache 面)。

---

## operators.v(85 條,全 timing)

**探針 1:mode=='sd2 時 y 的行為?**
規則書作答:R004——`mode == 'sd2 成立後的下一拍,y 變為 s1 << s2`?否,
R004 原文=`mode == 'sd2 成立後的下一拍,y 變為 s1 << u2`(帶號左移無號量)。
粗讀對照:operators.v:16 `2: y <= s1 << u2`——**一致**(R004 與源行 16 對應)。

**探針 2:每拍 y 的「無條件基底值」是什麼,由哪條規則承載?**
規則書作答:R001——`clk 上升沿 成立後的下一拍,y 變為 8'h42`(無條件
statement,無 when 修飾);各 mode 分支規則(R002..)是**條件面**,基底
先行被分支覆蓋=case 優先語義在規則書中以「statement 有無條件前綴」可辨。
粗讀對照:operators.v:12 `y <= 8'h42;` 先於 case——**一致**。

**探針 3:mode 位寬與分支覆蓋率的關係?規則書是否如實暴露覆蓋缺口?**
規則書作答:mode 為 `input [6:0]`(R002..R0xx 條件面含 `mode == 'sd0..`),
規則數 85;mode 全域 0..127 共 128 編碼,規則僅覆蓋 case 顯式列出的分支
(8 個 shift 運算×8 有號組合=64+缺省)——規則書**未**宣告「未覆蓋編碼」
(那是 G2 incomplete_case 破口掃描的職責,不在 rules 模式)。結論=誠實
分工,非漏報。
粗讀對照:case 分支數與 R 條件數一致;mode [6:0]=128 編碼面,**一致**。

評審結論:頭部密度檔(85 條)的規則陳述逐條可對源行;timing 類 statement
格式統一(`條件 成立後的下一拍,<reg> 變為 <expr>`),散文感低、可機檢。

---

## fsm.v(18 條:fsm 5+timing 12+counter 1)

**探針 1:state='sd100 時兩個輸入條件各導向何態?**
規則書作答:R001(button_a && !button_b → 'sd200)、R002
(!button_a && button_b → 'sd300)。
粗讀對照:fsm.v case 100 分支兩個 if——**一致**。

**探針 2:'sd200 → 'sd210 的轉移條件?附屬效果?
規則書作答:R003(state='sd200 且 cnt == 'sd5 → 下一拍 state='sd210);
同態另有 R012 條件時序效果(red_a/green_a 在 state=='sd200 下的賦值)。
粗讀對照:fsm.v case 200:`cnt<=cnt+1; if (cnt==5) state<=210; red_a<=0;
green_a<=1`——**一致**(R003 只述轉移,同態效果由 R012 系分承載=規則
粒度單一職責,非漏)。

**探針 3:reset 是同步還是異步?規則書答得出嗎?**
規則書作答:R011=`reset 成立後的下一拍,state 變為 'sd100`——「下一拍」
措辭隱含同步;但**單看規則書無法確證** sensitivities(異步 posedge reset
在 evidence 行為類 span 上,不在 statement 文字)。
粗讀對照:`ifdef ASYNC_RESET` 默認未定義→`always @(posedge clk)`=同步;
**規則書作答(隱含同步)與源一致,但確證需讀 evidence 或源碼**——規則書
陳述粒度的已知邊界,評審按 certain/heuristic 差異化採信的適用例。

評審結論:fsm 類轉移規則忠實;counter(cnt)效果分佈在 timing 類;同步/
異步覆位面=陳述文字不承載、evidence 承載——下游消費者須連 evidence 讀,
文檔可注記(非本輪阻塞)。

---

## multiplier.v(10 條,全 arithmetic;G3 誤報史檔)

**探針 1:全加器的輸出關係式?**
規則書作答:FullAdder.R001——`{Cout, S} ≡ A + B + Cin`(拼接 LHS 表達
雙輸出)。
粗讀對照:multiplier.v FullAdder `assign {Cout,S} = A+B+Cin`——**一致**
(R6 拼接 LHS 驅動圖誤報修復後,該形規則化正常、G3 不誤報)。

**探針 2:最終積 P 取自哪個部分積行(2D 版)?**
規則書作答:Multiplier_2D.R002——`P ≡ PP[N - 'sd1]`。
粗讀對照:2D 生成器的末行部分積即 P——**一致**。

**探針 3:gA/gB 的位形中移位量與選擇子關係?**
規則書作答:R003=`gA ≡ {A & M{B['sd0]}, i{1'b0}}`、R004=
`gB ≡ PP[(i - 'sd1) * (i + 'sd2 * M) / 'sd2 +:: M + i]`——部分積門控
`A&M{B[i]}`(符號擴展門控)+移位拼接;R004 的 +:: 部分選擇寬=M+i。
粗讀對照:生成器參數化拼接與規則書組合表達式同形;**表達式為機械轉寫
(範圍索引轉寫為算式)**,可讀性受 generate 參數化影響=已知換算面,
判定忠實但散文感高於行為類。

評審結論:arithmetic 類對 generate 生成器的組合關係轉寫正確;G3 誤報史
檔零破口誤報(eval per-file 破口列=0);拼接 LHS 規則化=誤報修復的回歸
錨在場。

---

## subbytes.v(7 條:reset 3+timing 3+arithmetic 1;G1 對沖正樣+when 先例)

**探針 1:復位時哪些寄存器裝載什麼值?**
規則書作答:R001(data_reg←'sd0)、R002(state←'sd0)、R003(ready_o←
'sd0),三條均 `當 !reset 且 clk 上升沿`。
粗讀對照:always @(posedge clk) if (!reset) begin 三個賦值——**一致**。

**探針 2:next_state 何時等於 state+1?適用域?**
規則書作答:R007——`當 !(state) 時,next_state ≡ state + 'sd1`(逐拍
組合);when 適用域=`!(state)`(分支域轉寫,非全域恆等)。
粗讀對照:subbytes.v:72-77 `end else begin ... next_state = state+1;`
=僅 state==0 臂——**一致**(when 適用域把分支限定轉寫進規則,避免
「全域誤讀」=R3-② 先例成立)。

**探針 3:本檔為何無 G1 latch 破口?規則書+破口兩模式合併可答嗎?**
規則書作答:時序行程(posedge)+組合行程(next_* 右值鏈)分立;破口模式
(gaps)對本檔報告=0——組合行程的 data_reg_var 等中間變量有塊首頂層
賦值(data_reg_var[0..3]=data_reg 切片),對沖成立。
粗讀對照:組合 always 頂層無條件賦值在前、分支在後——**一致**(G1 塊首
默認賦值對沖判準的實檔應用;subbytes 正是 R3-① 收平輪誤報形來源檔)。

評審結論:when 適用域與 reset 類格式均正確;規則+破口兩模式合併讀=
「為何零破口」可從規則書部分推出(塊首賦值),破口掃描確認——兩模式
分工閉合。

---

## acc_pipe.v(7 條:timing 5+arithmetic 2;G4+Apache 面)

**探針 1:第一級流水寄存了什麼?**
規則書作答:R001/R002——`x__input_flop ← x`、`y__input_flop ← y`(clk
上升沿下一拍)。
粗讀對照:acc_pipe.v stage0 兩個非阻塞賦值——**一致**。

**探針 2:p2_add_98_comb 的兩個加數與位寫法?**
規則書作答:R006——`p2_add_98_comb ≡ {2'h0, p0_bit_slice_48} +
{1'h0, p0_add_20}`(逐拍組合)。
粗讀對照:stage2 `assign p2_add_98_comb = {2'h0, p0_bit_slice_48} +
{1'h0, p0_add_20};`——**逐字一致**(拼接零擴展位寫原樣保留)。

**探針 3:輸出寄存器的驅動鏈(三級)?**
規則書作答:R007(out__output_flop←p2_scaled_comb)+R003/R005/R006 組合
鏈(p1_add_96_comb 由 x/y__input_flop;p0 兩寄存器←p1 組合;p2_scaled_comb
含 p2_add_98_comb)——鏈可從 7 條規則重建。
粗讀對照:stage0→stage1→stage2 三級,**一致**;G4 破口面=本檔 3 條
missing_reset(eval per-file 破口列 G4×3,寄存器均無復位)——與 xls
生成器無復位風格一致,破口報告如實。

評審結論:行為類鏈路規則忠實且零改寫;G4×3 破口=設計風格而非提取缺陷
(xls 生成器無復位),按 confidence=certain 差異化採信後仍屬真實測試缺口。

---

## R2 散文感抽查(§16.5;同批完成)

- i2c_master 深層 else-if 條件鏈(corpus/e2e/59):陳述含忠實噪音
  (`i2c_master_top.R0xx` 系列 when 修飾鏈),接受基準(S7 先例)——
  本輪 5 檔同基準:組合表達式轉寫(generate 參數化)散文感最高但判定
  忠實;行為類 timing/guard 格式最乾淨。
- 整體:陳述全部引用源碼真標識符(V5-④ 機檢在場),無「描述性散文」
  空轉規則;九類類別分布見報告類別分布表(R4 素材)。

## 總閘命令清單

```
cargo test --workspace
bash scripts/regress.sh
bash plugin/rtl2spec/run-mcp-e2e.sh
bash plugin/rtl2spec/run-rtl-validation.sh
bash plugin/rtl2spec/run-plugin-check.sh
bash plugin/rtl2spec/run-golden-regression.sh
bash plugin/rtl2spec/run-gap-fixtures.sh
bash plugin/rtl2spec/run-realworld-eval.sh
```
