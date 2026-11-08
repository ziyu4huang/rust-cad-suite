# rtl2spec spec-eval 證據頁(D 弧評測床;機械生成禁手改)

- 命令行:`bash plugin/rtl2spec/run-spec-eval.sh`(默認模式重生成本頁到 build/ 並 byte 比對;`--record --slice <D-Sn>` 寫本頁;直調不進 regress,D-PLAN §5.2)
- 口徑:iverilog -g2001 編譯(tb+wrapperO+DUT)+tbproto.sceneBlocks 逐場景塊 diff(形沿 reimpl-eval);首批腿=自對拍(reimplDut==origDut),盲寫腿 D-S5 凍結後增補;mutprobe=每設計一條確定性變異必紅(10.12 判別力)
- 工具鏈(錄製時):Icarus Verilog version 13.0 (stable) (v13_0);wasm sha256 `fe64d518cbcde3d1`
- 接線契約(R1-2):spec.module SSOT=spec 檔(rtl_spec_spec_parse);DUT 檔含 `module <spec.module>`=啟動斷言

| 腿 | 設計 | compiled | matched | expected |
|---|---|---|---|---|
| counter8-self | counter8 | true | 6/6 | 6/true |
| acc_pipe-self | acc_pipe | true | 6/6 | 6/true |
| i2c_master-self | i2c_master | true | 11/11 | 11/true |
| counter8-blind | counter8 | true | 6/6 | 6/true |
| acc_pipe-blind | acc_pipe | true | 6/6 | 6/true |
| i2c_master-blind | i2c_master | true | 11/11 | 11/true |

| mutprobe | 變異 | compiled | matched | 判別 |
|---|---|---|---|---|
| counter8 | counter-by-2 | true | 2/6 | 紅 |
| acc_pipe | capture-swap-xy | true | 1/6 | 紅 |
| i2c_master | idle-scl-oe-flip | true | 7/11 | 紅 |

