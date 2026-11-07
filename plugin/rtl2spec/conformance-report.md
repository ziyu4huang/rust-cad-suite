# rtl2spec conformance 證據頁(機械生成禁手改)

- 命令行:`bash plugin/rtl2spec/run-conformance-eval.sh`(默認模式重生成本頁到 build/ 並 byte 比對;`--record --slice <C-Sn>` 寫本頁)
- 口徑:conformance 度量**行為復原力**(oracle=原版 RTL 在 iverilog 下的可觀測行為),**不是意圖正確性**;TB 全過≠設計正確(§19.1)。
- 算子目錄:opcat-v1(選用序與每規則 ≤2 預算見 record.json catalog 欄)
- 工具鏈(錄製時):iverilog Icarus Verilog version 13.0 (stable) (v13_0);verilator Verilator 5.052 2026-09-05 rev vUNKNOWN-built20260905;yosys Yosys 0.70+179 (git sha1 9534a7f4d, Release, Clang /usr/local/osxcross/target/bin/aarch64-apple-darwin25.5-clang++ 21.1.8)(僅錄製期存活裁決)

## 逐設計判據(Q1..Q9;§19.4.3)

| 設計 | top | 規則 | 場景 | Q1 | Q2 | Q3 | Q4 | Q5 | Q6 | Q7 | Q8 | Q9 | 判定 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| acc_pipe | __acc_pipe__acc_pipe | 9 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| add3 | __add3__add3 | 6 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| aes_kexp128 | aes_key_expand_128 | 4 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| always01 | uut_always01 | 1 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| always02 | uut_always02 | 2 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| always03 | uut_always03 | 7 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| attrib01_module | attrib01_bar | 2 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| attrib02_port_decl | attrib02_bar | 2 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| attrib06_operator_suffix | attrib06_bar | 2 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| attrib08_mod_inst | attrib08_bar | 2 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| attrib09_case | attrib09_bar | 4 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| dff_different_styles | dff | 1 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| dff_init | dff0_test | 1 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| fiedler-cooley | up3down5 | 11 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| forloops | forloops01 | 4 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| fsm | fsm_test | 23 | 6 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| fulladd | halfadd | 2 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| graphtest | graphtest | 6 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| i2c_master | i2c_master | 152 | 6 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| i2c_master_tests | i2c_test01 | 5 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| loop_prefix_case | loop_prefix_case_top | 2 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| loops | aes | 11 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| omsp_dbg_uart | omsp_dbg_uart | 4 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| partsel | partsel_test001 | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| realexpr | demo_001 | 4 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| retime | retime_test | 1 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| scopes | scopes_test_01 | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| signedexpr | signed_test01 | 6 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| sincos | d | 46 | 6 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| spec_i2c_master | i2c_master | 152 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| subbytes | subbytes_00 | 16 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| usb_phy_tests | usb_phy_test01 | 10 | 4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| vloghammer | test01 | 1 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |
| wreduce | wreduce_test0 | 3 | 2 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ADMITTED |

## 判別力(P-rule 池;§19.4.3 數字定義)

| 設計 | generated | stillborn | equivalent | eligible | killed | survived | timeout | 殺傷率 | 規則驗證覆蓋 獨佔(共享) | 目錄覆蓋 | 聲明覆蓋 | 控制組存活 | ambiguous | P-unruled gen/killed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| acc_pipe | 11 | 0 | 1 | 10 | 10 | 0 | 0 | 10/10 | 9/9(0/9) | 9/9 | 9/9 | 3/3 | 2 | 1/1 |
| add3 | 7 | 0 | 0 | 7 | 7 | 0 | 0 | 7/7 | 6/6(0/6) | 6/6 | 0/6 | 2/2 | 2 | 1/1 |
| aes_kexp128 | 8 | 0 | 0 | 8 | 8 | 0 | 0 | 8/8 | 4/4(0/4) | 4/4 | 0/4 | 1/1 | 2 | 4/4 |
| always01 | 2 | 0 | 0 | 2 | 2 | 0 | 0 | 2/2 | 1/1(0/1) | 1/1 | 0/1 | 1/1 | 0 | 0/0 |
| always02 | 4 | 0 | 0 | 4 | 4 | 0 | 0 | 4/4 | 2/2(0/2) | 2/2 | 0/2 | 1/1 | 0 | 0/0 |
| always03 | 7 | 0 | 0 | 7 | 7 | 0 | 0 | 7/7 | 6/6(0/6) | 6/7 | 0/7 | 1/1 | 0 | 0/0 |
| attrib01_module | 3 | 0 | 0 | 3 | 2 | 1 | 0 | 2/3 | 2/2(0/2) | 2/2 | 1/2 | 2/2 | 0 | 0/0 |
| attrib02_port_decl | 3 | 0 | 0 | 3 | 2 | 1 | 0 | 2/3 | 2/2(0/2) | 2/2 | 1/2 | 2/2 | 0 | 0/0 |
| attrib06_operator_suffix | 4 | 0 | 0 | 4 | 3 | 1 | 0 | 3/4 | 2/2(0/2) | 2/2 | 1/2 | 2/2 | 0 | 0/0 |
| attrib08_mod_inst | 3 | 0 | 0 | 3 | 2 | 1 | 0 | 2/3 | 2/2(0/2) | 2/2 | 1/2 | 2/2 | 0 | 0/0 |
| attrib09_case | 8 | 0 | 4 | 4 | 4 | 0 | 0 | 4/4 | 4/4(0/4) | 4/4 | 0/4 | 5/5 | 0 | 0/0 |
| dff_different_styles | 1 | 0 | 0 | 1 | 1 | 0 | 0 | 1/1 | 1/1(0/1) | 1/1 | 0/1 | 1/1 | 0 | 27/0 |
| dff_init | 1 | 0 | 0 | 1 | 1 | 0 | 0 | 1/1 | 1/1(0/1) | 1/1 | 0/1 | 1/1 | 0 | 18/2 |
| fiedler-cooley | 16 | 0 | 2 | 14 | 14 | 0 | 0 | 14/14 | 9/9(0/9) | 11/11 | 0/11 | 3/3 | 0 | 0/0 |
| forloops | 5 | 0 | 0 | 5 | 5 | 0 | 0 | 5/5 | 3/3(0/3) | 3/4 | 0/4 | 2/2 | 6 | 6/0 |
| fsm | 44 | 0 | 2 | 42 | 42 | 0 | 0 | 42/42 | 23/23(0/23) | 23/23 | 23/23 | 4/4 | 0 | 2/2 |
| fulladd | 2 | 0 | 0 | 2 | 2 | 0 | 0 | 2/2 | 2/2(0/2) | 2/2 | 0/2 | 1/1 | 0 | 2/0 |
| graphtest | 10 | 0 | 3 | 7 | 7 | 0 | 0 | 7/7 | 6/6(0/6) | 6/6 | 0/6 | 4/4 | 0 | 6/4 |
| i2c_master | 272 | 0 | 17 | 255 | 201 | 54 | 0 | 201/255 | 124/152(17/152) | 152/152 | 141/152 | 20/20 | 2 | 3/2 |
| i2c_master_tests | 10 | 0 | 5 | 5 | 3 | 2 | 0 | 3/5 | 3/5(0/5) | 5/5 | 0/5 | 6/6 | 0 | 14/0 |
| loop_prefix_case | 2 | 0 | 0 | 2 | 2 | 0 | 0 | 2/2 | 2/2(0/2) | 2/2 | 0/2 | 1/1 | 0 | 1/1 |
| loops | 13 | 0 | 0 | 13 | 13 | 0 | 0 | 13/13 | 11/11(0/11) | 11/11 | 0/11 | 1/1 | 0 | 18/18 |
| omsp_dbg_uart | 5 | 0 | 0 | 5 | 5 | 0 | 0 | 5/5 | 3/3(0/3) | 3/4 | 1/4 | 3/3 | 0 | 4/3 |
| partsel | 1 | 0 | 0 | 1 | 1 | 0 | 0 | 1/1 | 1/1(0/1) | 1/1 | 0/1 | 1/1 | 0 | 74/3 |
| realexpr | 8 | 0 | 2 | 6 | 6 | 0 | 0 | 6/6 | 4/4(0/4) | 4/4 | 0/4 | 3/3 | 0 | 12/0 |
| retime | 2 | 0 | 0 | 2 | 2 | 0 | 0 | 2/2 | 1/1(0/1) | 1/1 | 0/1 | 1/1 | 0 | 3/1 |
| scopes | 1 | 0 | 0 | 1 | 1 | 0 | 0 | 1/1 | 1/1(0/1) | 1/1 | 0/1 | 1/1 | 0 | 30/28 |
| signedexpr | 12 | 0 | 2 | 10 | 10 | 0 | 0 | 10/10 | 6/6(0/6) | 6/6 | 0/6 | 3/3 | 4 | 0/0 |
| sincos | 82 | 0 | 0 | 82 | 76 | 6 | 0 | 76/82 | 41/46(4/46) | 46/46 | 45/46 | 2/2 | 0 | 0/0 |
| spec_i2c_master | 272 | 0 | 17 | 255 | 77 | 178 | 0 | 77/255 | 56/152(10/152) | 152/152 | 20/152 | 20/20 | 2 | 3/2 |
| subbytes | 18 | 0 | 5 | 13 | 8 | 5 | 0 | 8/13 | 6/11(1/11) | 16/16 | 3/16 | 8/8 | 0 | 14/10 |
| usb_phy_tests | 20 | 0 | 9 | 11 | 10 | 1 | 0 | 10/11 | 9/10(0/10) | 10/10 | 0/10 | 11/11 | 0 | 2/2 |
| vloghammer | 2 | 0 | 0 | 2 | 2 | 0 | 0 | 2/2 | 1/1(0/1) | 1/1 | 0/1 | 1/1 | 0 | 13/0 |
| wreduce | 4 | 0 | 0 | 4 | 4 | 0 | 0 | 4/4 | 3/3(0/3) | 3/3 | 0/3 | 1/1 | 2 | 2/0 |

## D 腿/T 腿基線分列(§19.3.1:D 腿增益=D−T,禁合併;D 腿=凍結 LLM 產物,協議=CONFORMANCE-PROTOCOL.md)

| 設計 | 協議腿 | D 腿殺傷率 | T 腿基線殺傷率 | 增益(D−T killed) | D 獨佔(共享) | T 獨佔(共享) | 形族底線(D 腿) | 不劣於基線 |
|---|---|---|---|---|---|---|---|---|
| acc_pipe | acc-r0-k1 | 10/10 | 10/10 | 0 | 9/9(0/9) | 9/9(0/9) | sequence 5/5;relation 5/5 | ✓ |
| fsm | fsm-r0-k1 | 42/42 | 31/42 | 11 | 23/23(0/23) | 17/23(0/23) | transition 10/10;sequence 32/32 | ✓ |
| i2c_master | i2c-r0-k1 | 201/255 | 77/255 | 124 | 124/152(17/152) | 56/152(10/152) | guard 7/21;transition 9/9;sequence 185/225 | ✓ |
| sincos | sincos-r2-k1 | 76/82 | 74/82 | 2 | 41/46(4/46) | 41/46(4/46) | guard 5/9;sequence 71/73 | ✓ |

## 機械註記

- acc_pipe:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- add3:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- aes_kexp128:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- always01:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- always02:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- always03:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- attrib09_case:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- dff_different_styles:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- dff_init:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- fiedler-cooley:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- forloops:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- fsm:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- fulladd:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- graphtest:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- i2c_master_tests:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- loop_prefix_case:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- loops:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- partsel:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- realexpr:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- retime:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- scopes:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- signedexpr:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- usb_phy_tests:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- vloghammer:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- wreduce:規則書無 guard→tb_plan resets=∅→S-RST/S-ARST 未生成、guard 類算子(OP-POL/OP-ASYNC/OP-RVAL)不產生;復位路徑只經隨機場景激勵(R1-11)。殺傷率的機械成因,禁誤讀為規則書或設計缺陷。
- 聲明覆蓋只計 COVERS 顯式點名的規則(模板非定向場景標 `*`,不折算為點名);驗證覆蓋以獨佔為主數、共享括注(禁合併)。
- 入庫 tb.v 禁手改:TB-SHA-ALARM=record.tb.sha256/磁碟 tb.v/本輪主腿 TB 三方核對,矛盾=REJECTED 或拒錄(exit 1);換檔唯一路徑=--record --promote(CONFORMANCE-PROTOCOL §5)。
- WAIVE 標籤集=x-valued|unobservable|unadjudicated|tb-internal(tblint WAIVE_LABELS;文法外/集外 Q9 硬紅;與協議模板最新版枚舉由 selfcheck 逐集合核對)。
- 波形幀診斷腿=輔助證據,不入本頁任何判據(入口 `bash plugin/rtl2spec/run-frame-diag.sh`;條文 CONFORMANCE-PROTOCOL §8)。

## 等價裁決(錄製期測試側 harness;協議 §7)

- 生產命令形:`read_verilog <src>; rename <top> gold; read_verilog <mut-top-only>; rename <top> gate; proc; flatten; memory; async2sync; opt_clean; equiv_make gold gate equiv; hierarchy -top equiv; select -count t:$equiv; equiv_simple -seq 5; equiv_induct -seq 5; equiv_status`;oracle=Yosys 0.70+179 (git sha1 9534a7f4d, Release, Clang /usr/local/osxcross/target/bin/aarch64-apple-darwin25.5-clang++ 21.1.8),僅測試用禁 ship,默認模式不跑。
- 雙向措辭:proven≠無條件行為等價(弱等價(-seq 5)+flatten/async2sync/memory_map 抽象);unproven≠不等價。eqverdict=equivalent|unproven|adjudication-failed;adjudication-failed 與 unproven 一律保守留在殺傷率分母。
- 空真守衛:proven 且 `select -count t:$equiv` cells≥輸出端口位數才算 equivalent,否則 adjudication-failed(empty);Q8 接不住空真,本守衛是唯一防線。空真探針=獨立模板(全名單 blacklist),不屬生產管線。

| 設計 | 端口位數 all/out | gold-vs-gold(cells) | 空真探針 | 裁決數 | equivalent | unproven | adjudication-failed | killed 全母體 總數(主腿) | 其中 unproven/failed/equivalent |
|---|---|---|---|---|---|---|---|---|---|
| acc_pipe | 33/16 | equivalent(108) | adjudication-failed(empty) | 1 | 1 | 0 | 0 | 10(10) | 10/0/0 |
| add3 | 34/9 | equivalent(60) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 7(7) | 7/0/0 |
| aes_kexp128 | 34/16 | equivalent(32) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 8(8) | 8/0/0 |
| always01 | 6/4 | equivalent(4) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 2(2) | 2/0/0 |
| always02 | 6/4 | equivalent(4) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 4(4) | 4/0/0 |
| always03 | 11/3 | equivalent(3) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 7(7) | 7/0/0 |
| attrib01_module | 4/1 | equivalent(1) | adjudication-failed(empty) | 1 | 0 | 1 | 0 | 2(2) | 2/0/0 |
| attrib02_port_decl | 4/1 | equivalent(1) | adjudication-failed(empty) | 1 | 0 | 1 | 0 | 2(2) | 2/0/0 |
| attrib06_operator_suffix | 26/8 | equivalent(8) | adjudication-failed(empty) | 1 | 0 | 1 | 0 | 3(3) | 3/0/0 |
| attrib08_mod_inst | 4/1 | equivalent(1) | adjudication-failed(empty) | 1 | 0 | 1 | 0 | 2(2) | 2/0/0 |
| attrib09_case | 6/2 | equivalent(2) | adjudication-failed(empty) | 4 | 4 | 0 | 0 | 4(4) | 4/0/0 |
| dff_different_styles | 3/1 | equivalent(1) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 1(1) | 1/0/0 |
| dff_init | 3/2 | equivalent(2) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 1(1) | 1/0/0 |
| fiedler-cooley | 24/12 | equivalent(12) | adjudication-failed(empty) | 2 | 2 | 0 | 0 | 14(14) | 14/0/0 |
| forloops | 19/16 | equivalent(48) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 5(5) | 5/0/0 |
| fsm | 8/4 | equivalent(40) | adjudication-failed(empty) | 2 | 2 | 0 | 0 | 42(42) | 42/0/0 |
| fulladd | 4/2 | equivalent(2) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 2(2) | 2/0/0 |
| graphtest | 30/22 | equivalent(27) | adjudication-failed(empty) | 3 | 3 | 0 | 0 | 7(7) | 7/0/0 |
| i2c_master | 49/16 | equivalent(57) | adjudication-failed(empty) | 63 | 17 | 46 | 0 | 209(201) | 208/1/0 |
| i2c_master_tests | 4/1 | equivalent(2) | adjudication-failed(empty) | 7 | 5 | 2 | 0 | 3(3) | 3/0/0 |
| loop_prefix_case | 2/1 | equivalent(2) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 2(2) | 2/0/0 |
| loops | 48/22 | equivalent(22) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 13(13) | 13/0/0 |
| omsp_dbg_uart | 4/1 | equivalent(8) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 5(5) | 4/1/0 |
| partsel | 43/8 | equivalent(14) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 1(1) | 1/0/0 |
| realexpr | 32/32 | equivalent(32) | adjudication-failed(empty) | 2 | 2 | 0 | 0 | 6(6) | 6/0/0 |
| retime | 10/1 | equivalent(9) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 2(2) | 2/0/0 |
| scopes | 36/32 | equivalent(48) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 1(1) | 1/0/0 |
| signedexpr | 23/18 | equivalent(18) | adjudication-failed(empty) | 2 | 2 | 0 | 0 | 10(10) | 10/0/0 |
| sincos | 64/41 | equivalent(107) | adjudication-failed(empty) | 6 | 0 | 6 | 0 | 76(76) | 76/0/0 |
| spec_i2c_master | 49/16 | equivalent(57) | adjudication-failed(empty) | 195 | 17 | 178 | 0 | 77(77) | 76/1/0 |
| subbytes | 86/42 | equivalent(175) | adjudication-failed(empty) | 10 | 5 | 5 | 0 | 8(8) | 8/0/0 |
| usb_phy_tests | 4/1 | equivalent(5) | adjudication-failed(empty) | 10 | 9 | 1 | 0 | 10(10) | 10/0/0 |
| vloghammer | 12/4 | equivalent(4) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 2(2) | 2/0/0 |
| wreduce | 64/48 | equivalent(48) | adjudication-failed(empty) | 0 | 0 | 0 | 0 | 4(4) | 4/0/0 |

killed 全母體裁決失敗項(這些變異 TB 已殺,yosys 判不了;不入 equivalent):

- i2c_master:m0011 OP-POL@71 yosys-error: Async reset \rst_n yields non-constant value 1'm for signal \ack_rx.
- omsp_dbg_uart:m0002 OP-POL@27 yosys-error: Async reset \dbg_rst yields non-constant value 3'mmm for signal \uart_state.
- spec_i2c_master:m0011 OP-POL@69 yosys-error: Async reset \rst_n yields non-constant value 1'm for signal \ack_rx.

## WAIVE 逐規則(標籤×裁決分布;數據謂詞=協議 §6;靜態標籤只可見化)

| 設計 | 規則 | 標籤 | 變異 | killed | equivalent | unproven | adjudication-failed | 未裁決 | 數據謂詞 |
|---|---|---|---|---|---|---|---|---|---|
| fiedler-cooley | up3down5.R007 | x-valued | 1 | 0 | 1 | 0 | 0 | 0 | unobservable |
| i2c_master | i2c_master.R042 | unadjudicated | 2 | 0 | 1 | 1 | 0 | 0 | unadjudicated |
| i2c_master | i2c_master.R043 | unadjudicated | 2 | 0 | 0 | 2 | 0 | 0 | unadjudicated |
| i2c_master | i2c_master.R044 | unadjudicated | 2 | 0 | 0 | 2 | 0 | 0 | unadjudicated |
| i2c_master | i2c_master.R045 | unadjudicated | 2 | 0 | 0 | 2 | 0 | 0 | unadjudicated |
| i2c_master | i2c_master.R066 | unadjudicated | 2 | 0 | 0 | 2 | 0 | 0 | unadjudicated |
| i2c_master | i2c_master.R087 | unadjudicated | 2 | 0 | 0 | 2 | 0 | 0 | unadjudicated |
| i2c_master | i2c_master.R100 | unadjudicated | 2 | 0 | 0 | 2 | 0 | 0 | unadjudicated |
| i2c_master | i2c_master.R101 | unadjudicated | 2 | 0 | 0 | 2 | 0 | 0 | unadjudicated |
| i2c_master | i2c_master.R104 | unadjudicated | 1 | 0 | 0 | 1 | 0 | 0 | unadjudicated |
| i2c_master | i2c_master.R122 | unadjudicated | 2 | 0 | 0 | 2 | 0 | 0 | unadjudicated |
| i2c_master | i2c_master.R149 | unadjudicated | 2 | 0 | 0 | 2 | 0 | 0 | unadjudicated |
| sincos | d.R035 | unadjudicated | 2 | 0 | 0 | 2 | 0 | 0 | unadjudicated |

## P-unruled 被殺行(規則書未描述而 TB 可觀測=規則書缺陷的確定性證據;不入殺傷率分母)

- acc_pipe corpus/e2e/58_xls_acc_pipe/acc_pipe.v:43 OP-HOLD:`assign out = out__output_flop;` → `assign out = out;`(殺傷:SCENE1 pipe_stages_sync_ramp,SCENE2 only_x_changes,SCENE3 only_y_changes,SCENE4 boundary_walk)
- add3 corpus/e2e/57_xls_add3/add3.v:32 OP-HOLD:`assign out = out__output_flop;` → `assign out = out;`(殺傷:SCENE2 full,SCENE3 small,SCENE4 hold)
- aes_kexp128 corpus-external/yosys-simple/aes_kexp128.v:11 OP-HOLD:`assign wo_0 = w[0];` → `assign wo_0 = wo_0;`(殺傷:SCENE2 full,SCENE3 small,SCENE4 hold)
- aes_kexp128 corpus-external/yosys-simple/aes_kexp128.v:12 OP-HOLD:`assign wo_1 = w[1];` → `assign wo_1 = wo_1;`(殺傷:SCENE2 full,SCENE3 small,SCENE4 hold)
- aes_kexp128 corpus-external/yosys-simple/aes_kexp128.v:13 OP-HOLD:`assign wo_2 = w[2];` → `assign wo_2 = wo_2;`(殺傷:SCENE2 full,SCENE3 small,SCENE4 hold)
- aes_kexp128 corpus-external/yosys-simple/aes_kexp128.v:14 OP-HOLD:`assign wo_3 = w[3];` → `assign wo_3 = wo_3;`(殺傷:SCENE2 full,SCENE3 small,SCENE4 hold)
- dff_init corpus-external/yosys-simple/dff_init.v:4 OP-CONST:`reg n1 = 32'd0;` → `reg n1 = 32'd1;`(殺傷:SCENE2 full,SCENE3 small,SCENE4 hold)
- dff_init corpus-external/yosys-simple/dff_init.v:8 OP-HOLD:`assign n1_inv = ~n1;` → `assign n1_inv = n1_inv;`(殺傷:SCENE2 full,SCENE3 small,SCENE4 hold)
- fsm corpus-external/yosys-simple/fsm.v:63 OP-HOLD:`state <= 100;` → `state <= state;`(殺傷:SCENE3 b_arm_full,SCENE4 both_neither,SCENE5 mid_arm_reset,SCENE6 random_buttons)
- fsm corpus-external/yosys-simple/fsm.v:63 OP-CONST:`state <= 100;` → `state <= 101;`(殺傷:SCENE5 mid_arm_reset,SCENE6 random_buttons)
- graphtest corpus-external/yosys-simple/graphtest.v:20 OP-HOLD:`assign Z[0] = 1'b0;                      // Constant connects to PO` → `assign Z[0] = Z[0];                      // Constant connects to PO`(殺傷:SCENE1 exh,SCENE2 small)
- graphtest corpus-external/yosys-simple/graphtest.v:20 OP-CONST:`assign Z[0] = 1'b0;                      // Constant connects to PO` → `assign Z[0] = 1'b1;                      // Constant connects to PO`(殺傷:SCENE1 exh,SCENE2 small)
- graphtest corpus-external/yosys-simple/graphtest.v:21 OP-HOLD:`assign Z[1] = t[3];                      // Intermediate sig connects to PO` → `assign Z[1] = Z[1];                      // Intermediate sig connects to PO`(殺傷:SCENE1 exh,SCENE2 small)
- graphtest corpus-external/yosys-simple/graphtest.v:27 OP-HOLD:`X = B;` → `X = X;`(殺傷:SCENE1 exh)
- i2c_master corpus/e2e/59_i2c_master/i2c_master.v:68 OP-HOLD:`wire cnt_end = (cnt >= prescale - 1);` → `wire cnt_end = cnt_end;`(殺傷:SCENE2 wr_byte_seq,SCENE3 rd_byte_seq,SCENE4 repeated_start,SCENE5 prescale_sweep,SCENE6 mid_transfer_reset)
- i2c_master corpus/e2e/59_i2c_master/i2c_master.v:68 OP-ARITH:`wire cnt_end = (cnt >= prescale - 1);` → `wire cnt_end = (cnt >= prescale + 1);`(殺傷:SCENE2 wr_byte_seq,SCENE3 rd_byte_seq,SCENE4 repeated_start,SCENE5 prescale_sweep,SCENE6 mid_transfer_reset)
- loop_prefix_case corpus-external/yosys-simple/loop_prefix_case.v:9 OP-HOLD:`wire [i:i] z = x;` → `wire [i:i] z = z;`(殺傷:SCENE1 exh,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:33 OP-HOLD:`keysched_start_i = 0;` → `keysched_start_i = keysched_start_i;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:33 OP-CONST:`keysched_start_i = 0;` → `keysched_start_i = 1;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:34 OP-HOLD:`keysched_round_i = addroundkey_round;` → `keysched_round_i = keysched_round_i;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:35 OP-HOLD:`round_data_var = addroundkey_data_reg;` → `round_data_var = round_data_var;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:36 OP-HOLD:`next_addroundkey_data_reg = addroundkey_data_reg;` → `next_addroundkey_data_reg = next_addroundkey_data_reg;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:37 OP-HOLD:`next_addroundkey_ready_o = 0;` → `next_addroundkey_ready_o = next_addroundkey_ready_o;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:37 OP-CONST:`next_addroundkey_ready_o = 0;` → `next_addroundkey_ready_o = 1;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:38 OP-HOLD:`next_addroundkey_round = addroundkey_round;` → `next_addroundkey_round = next_addroundkey_round;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:41 OP-HOLD:`keysched_last_key_i = key_i;` → `keysched_last_key_i = keysched_last_key_i;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:43 OP-HOLD:`keysched_last_key_i = keysched_new_key_o;` → `keysched_last_key_i = keysched_last_key_i;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:47 OP-HOLD:`data_var = addroundkey_data_i;` → `data_var = data_var;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:48 OP-HOLD:`round_key_var = key_i;` → `round_key_var = round_key_var;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:50 OP-HOLD:`next_addroundkey_data_reg = round_data_var;` → `next_addroundkey_data_reg = next_addroundkey_data_reg;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:55 OP-HOLD:`keysched_last_key_i = key_i;` → `keysched_last_key_i = keysched_last_key_i;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:63 OP-HOLD:`keysched_last_key_i = keysched_new_key_o;` → `keysched_last_key_i = keysched_last_key_i;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:69 OP-HOLD:`data_var = addroundkey_data_i;` → `data_var = data_var;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:70 OP-HOLD:`round_key_var = keysched_new_key_o;` → `round_key_var = round_key_var;`(殺傷:SCENE1 rand,SCENE2 small)
- loops corpus-external/yosys-simple/loops.v:72 OP-HOLD:`next_addroundkey_data_reg = round_data_var;` → `next_addroundkey_data_reg = next_addroundkey_data_reg;`(殺傷:SCENE1 rand,SCENE2 small)
- omsp_dbg_uart corpus-external/yosys-simple/omsp_dbg_uart.v:20 OP-HOLD:`RX_SYNC  : uart_state_nxt =  RX_CMD;` → `RX_SYNC  : uart_state_nxt =  uart_state_nxt;`(殺傷:SCENE2 full,SCENE3 small)
- omsp_dbg_uart corpus-external/yosys-simple/omsp_dbg_uart.v:21 OP-HOLD:`RX_CMD   : uart_state_nxt =  mem_burst ? RX_DATA : RX_SYNC;` → `RX_CMD   : uart_state_nxt =  uart_state_nxt;`(殺傷:SCENE2 full,SCENE3 small,SCENE4 hold)
- omsp_dbg_uart corpus-external/yosys-simple/omsp_dbg_uart.v:22 OP-HOLD:`RX_DATA : uart_state_nxt =  RX_SYNC;` → `RX_DATA : uart_state_nxt =  uart_state_nxt;`(殺傷:SCENE2 full,SCENE3 small)
- partsel corpus-external/yosys-simple/partsel.v:2 OP-HOLD:`wire [5:0] offset = idx << 2;` → `wire [5:0] offset = offset;`(殺傷:SCENE1 rand,SCENE2 small)
- partsel corpus-external/yosys-simple/partsel.v:2 OP-ARITH:`wire [5:0] offset = idx << 2;` → `wire [5:0] offset = idx >> 2;`(殺傷:SCENE1 rand,SCENE2 small)
- partsel corpus-external/yosys-simple/partsel.v:3 OP-HOLD:`assign slice_up = data[offset +: 4];` → `assign slice_up = slice_up;`(殺傷:SCENE1 rand,SCENE2 small)
- retime corpus-external/yosys-simple/retime.v:5 OP-HOLD:`assign z = ff[7];` → `assign z = z;`(殺傷:SCENE2 full,SCENE3 small,SCENE4 hold)
- scopes corpus-external/yosys-simple/scopes.v:5 OP-HOLD:`func_01 = x + y;` → `func_01 = func_01;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:5 OP-ARITH:`func_01 = x + y;` → `func_01 = x - y;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:8 OP-HOLD:`x = y;` → `x = x;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:9 OP-HOLD:`func_01 = func_01 ^ x;` → `func_01 = func_01;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:9 OP-ARITH:`func_01 = func_01 ^ x;` → `func_01 = func_01 | x;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:11 OP-HOLD:`func_01 = func_01 ^ x;` → `func_01 = func_01;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:11 OP-ARITH:`func_01 = func_01 ^ x;` → `func_01 = func_01 | x;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:18 OP-HOLD:`func_02 = x - y;` → `func_02 = func_02;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:18 OP-ARITH:`func_02 = x - y;` → `func_02 = x + y;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:30 OP-HOLD:`y = a * 23;` → `y = y;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:30 OP-CONST:`y = a * 23;` → `y = a * 24;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:31 OP-HOLD:`x = x + y;` → `x = x;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:31 OP-ARITH:`x = x + y;` → `x = x - y;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:39 OP-HOLD:`x = y;` → `x = x;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:42 OP-HOLD:`x = 77 + a;` → `x = x;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:42 OP-ARITH:`x = 77 + a;` → `x = 77 - a;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:43 OP-HOLD:`z = -x;` → `z = z;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:43 OP-ARITH:`z = -x;` → `z = +x;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:45 OP-HOLD:`y = x ^ z;` → `y = y;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:45 OP-ARITH:`y = x ^ z;` → `y = x | z;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:50 OP-HOLD:`x = func_01(11, 22);` → `x = x;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:50 OP-CONST:`x = func_01(11, 22);` → `x = func_01(12, 22);`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:51 OP-HOLD:`y = func_02(33, 44);` → `y = y;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:51 OP-CONST:`y = func_02(33, 44);` → `y = func_02(34, 44);`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:56 OP-HOLD:`y = x;` → `y = y;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:58 OP-HOLD:`x = y;` → `x = x;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:60 OP-HOLD:`x = func_01(y, x);` → `x = x;`(殺傷:SCENE1 exh,SCENE2 small)
- scopes corpus-external/yosys-simple/scopes.v:61 OP-HOLD:`y = func_02(y, x);` → `y = y;`(殺傷:SCENE1 exh,SCENE2 small)
- spec_i2c_master crates/spec-i2c/rtl/i2c_master.v:66 OP-HOLD:`wire cnt_end = (cnt >= prescale - 1);` → `wire cnt_end = cnt_end;`(殺傷:SCENE3 small,SCENE4 hold)
- spec_i2c_master crates/spec-i2c/rtl/i2c_master.v:66 OP-ARITH:`wire cnt_end = (cnt >= prescale - 1);` → `wire cnt_end = (cnt >= prescale + 1);`(殺傷:SCENE3 small)
- subbytes corpus-external/yosys-simple/subbytes.v:47 OP-HOLD:`data_i_var = data_i;` → `data_i_var = data_i_var;`(殺傷:SCENE1 arst_reset,SCENE2 full,SCENE3 small,SCENE4 hold)
- subbytes corpus-external/yosys-simple/subbytes.v:59 OP-HOLD:`sbox_decrypt_o = decrypt_i;` → `sbox_decrypt_o = sbox_decrypt_o;`(殺傷:SCENE1 arst_reset,SCENE2 full,SCENE3 small,SCENE4 hold)
- subbytes corpus-external/yosys-simple/subbytes.v:60 OP-HOLD:`sbox_data_o = data_array[state];` → `sbox_data_o = sbox_data_o;`(殺傷:SCENE1 arst_reset,SCENE2 full,SCENE3 small,SCENE4 hold)
- subbytes corpus-external/yosys-simple/subbytes.v:64 OP-HOLD:`next_ready_o = 0;` → `next_ready_o = next_ready_o;`(殺傷:SCENE1 arst_reset,SCENE2 full,SCENE3 small,SCENE4 hold)
- subbytes corpus-external/yosys-simple/subbytes.v:64 OP-CONST:`next_ready_o = 0;` → `next_ready_o = 1;`(殺傷:SCENE1 arst_reset,SCENE2 full,SCENE3 small,SCENE4 hold)
- subbytes corpus-external/yosys-simple/subbytes.v:65 OP-HOLD:`data_o = data_reg;` → `data_o = data_o;`(殺傷:SCENE1 arst_reset,SCENE2 full,SCENE3 small,SCENE4 hold)
- subbytes corpus-external/yosys-simple/subbytes.v:73 OP-HOLD:`data_reg_128[ 31: 24] = data_reg_var[0];` → `data_reg_128[ 31: 24] = data_reg_128[ 31: 24];`(殺傷:SCENE1 arst_reset,SCENE2 full,SCENE3 small,SCENE4 hold)
- subbytes corpus-external/yosys-simple/subbytes.v:74 OP-HOLD:`data_reg_128[ 23: 16] = data_reg_var[1];` → `data_reg_128[ 23: 16] = data_reg_128[ 23: 16];`(殺傷:SCENE1 arst_reset,SCENE2 full,SCENE3 small,SCENE4 hold)
- subbytes corpus-external/yosys-simple/subbytes.v:75 OP-HOLD:`data_reg_128[ 15:  8] = data_reg_var[2];` → `data_reg_128[ 15:  8] = data_reg_128[ 15:  8];`(殺傷:SCENE1 arst_reset,SCENE2 full,SCENE3 small,SCENE4 hold)
- subbytes corpus-external/yosys-simple/subbytes.v:76 OP-HOLD:`data_reg_128[  7:  0] = data_reg_var[3];` → `data_reg_128[  7:  0] = data_reg_128[  7:  0];`(殺傷:SCENE1 arst_reset,SCENE2 full,SCENE3 small,SCENE4 hold)
- usb_phy_tests corpus-external/yosys-simple/usb_phy_tests.v:16 OP-HOLD:`fs_ce = 1'b0;` → `fs_ce = fs_ce;`(殺傷:SCENE2 full,SCENE3 small,SCENE4 hold)
- usb_phy_tests corpus-external/yosys-simple/usb_phy_tests.v:16 OP-CONST:`fs_ce = 1'b0;` → `fs_ce = 1'b1;`(殺傷:SCENE2 full,SCENE3 small,SCENE4 hold)
