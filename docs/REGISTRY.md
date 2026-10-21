# rust-cad-suite REGISTRY(組件登記簿 = 本 repo 的 CRM:組件×能力×閘×證據的關係表)

> 單一事實來源原則:此表只放「關係與指針」,不放瞬時值(通過率看報告頁,
> 由 `vl-difftest --html` 重生成)。改動組件職責必須同輪更新本表。

## Crates

| Crate | 職責 | 依賴 | 對應上游角色 | 證據/閘 |
|---|---|---|---|---|
| `rcs-diag` | spans/source map/診斷/caret 渲染(suite 共享) | 0 | verilator 的 %Error 輸出層 | 渲染 golden 測試 |
| `vl-syntax` | pp+lexer+parser+AST(V2005) | rcs-diag | VPreproc/V3Parser | lexer 12 + frontend 17 測試;E2E 全語料過解析閘 |
| `vl-sim` | 4-state 值/求值/展開/事件調度器 | vl-syntax, rcs-diag | VCSD/eval/scheduling | value 8 測試;sim_gate 2(counter8+fulladd byte-diff) |
| `vl-cli` | bin `rverilator`(--version/-E/--lint-only/--dump-ast/--binary/--top-module) | 上全部 | verilator driver | 未知旗標=錯 exit1(對抗審 #8) |
| `vl-difftest` | oracle 錄製/差分閘/HTML 報告(dev-only, publish=false) | vl-syntax, vl-sim, rcs-diag | —(自建) | regression_gate ≥80% 斷言 |

## 語言能力 ↔ 里程碑 ↔ 證據 case

| 能力 | 里程碑 | E2E case |
|---|---|---|
| 運算符/上下文位寬/符號 | ✅ | 01,02,03,04,05,12,31,33,34,46,47 |
| 字面量/格式化契約 | ✅ | 10,11,45 |
| case/casez/identity | ✅ | 06,13,35 |
| 循環 | ✅ | 14,50 |
| 參數/$clog2 傳播 | ✅ | 15,38 |
| 層次實例化 | ✅ | 16 |
| 函式(靜態) | ✅ | 17,18,39,40 |
| 任務(內聯) | ✅ | 19,20,41 |
| always @*/電平敏感 t0 | ✅ | 21 |
| 時序/NBA/邊沿 | ✅(M5) | 22,23,25,27,42,43,44 |
| $time/%t/timescale | ✅ | 24,48 |
| 多 initial 確定序 | ✅ | 32,49 |
| generate for/if/case | ✅ | 28,29,36,37 |
| 存儲器陣列 | M7 硬錯 | 30(聲明 FAIL) |
| VCD | M6 未開工 | — |
| lint-only(WIDTH 族) | 未開工 | — |

## 閘(Gates)

| 閘 | 觸發 | 斷言 |
|---|---|---|
| `cargo test`(全量) | 每次提交 | 41 tests 全綠 + E2E ≥80% |
| `bash scripts/regress.sh` | 交付前 | fmt→build→test→difftest 四連 |
| `vl-difftest --html` | 報告需求 | 生成 docs/e2e-report/index.html |
| `vl-difftest --record` | 語義變更後 | 重錄 oracle 期望(人工讀 diff) |
