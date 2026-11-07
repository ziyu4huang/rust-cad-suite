# conformance-lint — TB-R1x 跨檔五向量(自著 MIT;§19.5 ② selfcheck;§19.3.3)

run-conformance-eval 的 selfcheck 每次必跑;E(DUT 沿集合)一律經 dist wasm `rtl_tb_plan_sv`
取 `sens_edges`(閘側不解析 DUT 文本),M 取 TB 實例化具名連接。

| 向量 | DUT | TB | 期望 |
|---|---|---|---|
| v1(+) | dut_pos.v `always @(posedge clk)` 讀 d | tb_v1_pos_same_edge.v `@(posedge clk); d = 1;` | 1 違例 |
| v2(−) | dut_pos.v | tb_v2_opposite_edge.v `@(negedge clk); d = 1;` | 0 |
| v3(+) | dut_neg.v `always @(negedge clk)` 讀 d | tb_v3_neg_same_edge.v `@(negedge clk); d = 1;` | 1 違例 |
| v4(−) | dut_pos.v | tb_v4_delay_same_line.v `@(posedge clk); #1; d = 1;` | 0 |
| v5(−) | dut_pos.v | tb_v5_delay_next_line.v `@(posedge clk); #1;`+換行 `d = 1;` | 0(文檔語義;Rust corpus_lint 報 1=D-c3) |

正向量期望誤配負向量必紅(判別力自證)。外部語料與 GPL 禁入本目錄。
