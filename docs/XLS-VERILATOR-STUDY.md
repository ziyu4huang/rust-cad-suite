# XLS 與 Verilator 的依賴關係(2026-10-23 查證)

問題:rverilator 能不能替代/補充 Verilator 在 XLS 流程裡的角色?XLS 是否「需要」Verilator?

## 結論

**XLS 不需要 Verilator。** 它的模擬驗證層是可插拔介面,唯一內建實作是 **Icarus Verilog**;
Verilator 只出現在文件範例與一處發射端遷就註解。rverilator 可以填同一介面角色。

## 證據(google/xls @ main,2026-10-23 gh api)

1. **可插拔模擬器介面** `xls/simulation/verilog_simulator.h`:
   純虛介面 `Run(text, file_type, macro_definitions, includes) -> (stdout, stderr)`
   + `RunSyntaxChecking(text, ...)`(語法檢查=我方 `--lint-only` 的同構物)。
   輸入=Verilog 文字,輸出=stdout/stderr——**介面的流通貨就是 testbench 的 `$display` 輸出**,
   這正是 rverilator 的差分閘每天都在消費的東西。
2. **唯一內建實作** `xls/simulation/simulators/`:目錄裡只有 `iverilog_simulator.cc`(+BUILD)。
   實作=寫暫存 top.v → 子流程 `iverilog -o` 編譯 → 子流程 `vvp` 執行 → 回 (stdout, stderr)。
   沒有 `verilator_simulator.cc`。
3. **Verilator 出現處全查**(code search 5 檔):
   - `docs_src/interpreters.md`:"Generated Verilog can be simulated using external simulators
     (e.g., Verilator)" —— 文件舉例。
   - `xls/codegen/module_builder.cc`:`// Verilator does not like declaration of arrays and
     assignments in the same line` —— 發射端遷就 Verilator lint 的 workaround 註解。
   - `docs_src/ideas_and_projects.md`、`experimental/busperf/examples/README.md`、`MODULE.bazel.lock`
     —— 提及/鎖檔殘影,非依賴。
4. **fuzzer**(`xls/fuzzer/sample.proto`/`run_crasher.cc`)同走 iverilog 模擬器路徑。

## 對 rverilator 的整合含義

- **介面對位**:XLS 的 `VerilogSimulator` 只要求「文字進、stdout/stderr 出」+「巨集定義 `-D`」
  +「include 檔」。rverilator CLI(`--lint-only`/simulate/`-D`/`-I`)已覆蓋全部面;
  `$display` 為本的 testbench 慣例直接沿用。
- **獨立驗證不依賴高階**:XLS 自身的 DSLX interpreter/JIT 是高階驗證;RTL 層的獨立潔室覆核
  應由外部模擬器做。本 repo 的做法=oracle 錄製(XLS 產 Verilog+race-safe TB)→ rverilator
  與 Verilator 各跑一遍 byte-diff(`corpus/e2e/57_xls_add3`、`58_xls_acc_pipe`)。
- **瀏覽器內獨立驗證**:wasm demo 的 XLS 卡(`crates/vl-wasm/wasm-demo/index.html`)把
  XLS 產 Verilog 在瀏覽器裡用 rverilator-wasm 跑+對 oracle 期望值逐字比對
  ——「不只依賴高階 XLS 的驗證」的展示面。
  證據圖=`docs/e2e-report/wasm-demo-v2-verified.png`、`wasm-demo-v2-accpipe.png`。
- 若要正式插進上游 C++ 介面:實作一個 `VerilogSimulator` 子類包 rverilator 子流程
  (照 `iverilog_simulator.cc` 的形狀)即可;上游還要求 SystemVerilog 時 iverilog 直接
  `UnimplementedError`,我方 V2005 範圍同樣明確報錯(方向一致)。

## 授權/引用

XLS = Apache-2.0。語料快照與 NOTICE 見 `corpus-external/xls/README.md`(收割協議
`.github/workflows/xls-harvest.yml`)。
