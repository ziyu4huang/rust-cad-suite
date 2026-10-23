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
| `vl-wasm` | WASM facade(cdylib):`run_verilog(_stdout)`/`WasmSim`,復用 CLI 頂層偵測;wasm-pack --target web,~500KB | `wasm-test/run.mjs` 差分閘(同 oracle goldens,6 例)入 regress.sh(pkg 過期自動重建) | ADR-0003 facade 慣例 |
| `vl-ffi` | C-ABI host facade(cdylib):`rcs_run(manifest)/rcs_version/rcs_free`,行式 manifest(file/inject);Bun bun:ffi 兩式(FFI 進程內/spawn subprocess) | bun-example 差分閘同 goldens 入 regress(dylib 缺=略過提示) | ADR-0003 facade 慣例 |
| `vl-spec` | Spec 驗證核心:Signals 句柄/Invariant trait/GoldenModel trait/LockstepSpec harness(put+run_until 成對紀律,ADR-0007) | vl-sim, vl-syntax, rcs-diag | —(自建) | spec_gate 4 測 |
| `spec-i2c` | 乾淨室 I2C master demo:RTL+golden model+匯流排驅動器+4 協議不變量+負向變種偵測(publish=false) | vl-spec, vl-sim, rcs-diag | OpenCores i2c(GPL,唯讀啟發零拷貝) | scripted/隨機 20 事務乾淨;LSB-first/no-ack-release 負向全被抓 |
| `vl-vcd` | VCD 單一核心:writer(IEEE §18 正典,4 慢字串介面)+容忍式 reader(E1xx/W1xx,GTKWave 啟發子集)+SVG 渲染器(ADR-0005:確定性 XML 轉義)+`vcd2svg` bin | 0 | —(自建;GTKWave gw-vcd-loader 為研究參照) | reader+render 單測 15;golden=tests/goldens/counter.svg byte-diff(CLI/wasm 同字節);零漂移=e2e expect.vcd byte-diff |

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
| VCD | ✅(M6) | 51_vcd_counter(+21/27/42 加 dump);oracle-ref.vcd 交叉對拍=vcd_gate |
| 前端 fuzz | ✅ | vl-syntax/tests/fuzz_smoke.rs(600 迭代;三個 UTF-8 bug 在案) |
| lint-only(WIDTH 族) | 未開工 | — |

## 閘(Gates)

| 閘 | 觸發 | 斷言 |
|---|---|---|
| `cargo test`(全量) | 每次提交 | 41 tests 全綠 + E2E ≥80% |
| `bash scripts/regress.sh` | 交付前 | build→test→difftest→report 四連 |
| `cargo test --test fuzz_smoke` | 每次提交 | 語料變異+隨機字節無 panic/無掛死 |
| `cargo test --test corpus_lint` | 每次提交 | R1 同邊沿競爭模式 / R2 期望在場 / R3 dump↔oracle-ref 一致 / 歸一化契約 |
| `cargo test --test vcd_gate` | 每次提交 | 有 oracle-ref.vcd 的 case 與 oracle 逐訊號零失配 |
| `bun crates/vl-wasm/wasm-test/vcd_parse.mjs` | regress.sh | 三面共用 reader:wasm 版 BigInt 時序/單調/終值/VL-E101 錯誤路徑 |
| `bun crates/vl-ffi/bun-example/vcd_read.mjs` | regress.sh | agent 讀波形:FFI run→`vcd <path>` 強制 dump→rcs_vcd_parse→q 終值斷言 |
| difftest `--min-pass 100`(regress.sh) | 每次交付 | e2e 精確數閘(80% 門檻曾讓 55/57 藏身綠燈,1023 教訓) |
| lint parity 基線 80(regress.sh) | 每次交付 | 四輪後基線;閘曾攔下 78 誤算(ADR-0006 備忘) |
| `cargo test -q -p vl-spec -p spec-i2c` | regress.sh | spec 驗證層:lockstep/不變量/負向偵測/隨機 |
| D17 timer 期限推進 | — | rVPI AfterDelay 於自身期限準點觸發(舊=拖到下一 # 事件);vpi_gate 20000→16000 |
| `bun crates/vl-wasm/wasm-test/vcd_svg.mjs` | regress.sh | 三面同源渲染器:wasm one-shot==golden byte-diff+句柄重渲+u64 窗口+hostile 轉義+錯誤路徑 |
| `bun crates/vl-ffi/bun-example/vcd_svg.mjs` | regress.sh | agent 產波形:run→dump→rcs_vcd_svg→svg 形狀+q 終值 title 斷言 |
| `cargo test -p vl-vcd --test render_golden` | 每次提交 | render golden byte-diff+CLI stdout golden+未知旗標 exit1+VL-E101 |

## Divergence register(我方 vs oracle 已知且接受的差異 — 新分歧先登記再動工)

| # | 領域 | oracle(Verilator 5.050) | 我方 | 政策 | 證據 |
|---|---|---|---|---|---|
| D1 | X/Z 傳播 | 2-state:x 折成 0(`1x00+1=1001`) | IEEE §5 4-state(算術整體-x §5.1.4) | byte-diff 語料 X-clean;4-state 走單測軌 | corpus/oracle-fixtures/xprobe.sv;ADR-0001 |
| D2 | 同邊沿競爭序 | 內部排序(實錄 43 初版 q=54) | 宣告序(ADR-0001) | 語料禁競爭(corpus_lint R1) | corpus/e2e/26,43 歷史 |
| D3 | VCD 參數 | 參數作為 var dump(`W [31:0]`) | 不 dump 參數 | 交叉對拍只比匹配訊號,oracle-only 列報告 | vcd_gate 輸出 `oracle-only: ["tb.dut.W"]` |
| D4 | VCD 終時戳 | `$finish` 時刻也打 `#t`(無變化) | 只在有變化時打 `#t` | 步函數比對與 dump 頻率無關 | 51 號 17 vs 16 標記 |
| D5 | 超長 sized 字面量 | 報錯 | 報錯(跟 oracle,非 LRM 截斷) | oracle 即 spec | value.rs from_numlit |
| D6 | 二元 `~&`/`~|` | 拒收(Annex A 無) | 拒收 | 同上 | parser/expr.rs |
| D7 | 電平敏感 always t0 | t0 先執行一次 | 同 oracle | 同上 | 21 號 |
| D8 | `$display` 診斷前綴 | `%Error:` 等 | 相同前綴;lint 文案不逐字對齊 | exit code+類別對齊,文案自由 | PLAN §6 |
| D9 | 讀入 x 值(2-state) | x→0(`readmem x2`→02) | 4-state 保留 x | 差分語料 X-clean;4-state 單測軌 | 53_readmem 實錄(1023) |
| D10 | 值域字面量檢查位置 | 靜態全函數體 lint | 惰性求值(函數體調用才檢) | 未調用函數的字面量錯誤不攔(values.v) | LINT-DIFF.tsv |
| D11 | 位置參數覆蓋超界 | 寬容(忽略) | 同(對齊 oracle,比 LRM 松) | oracle 即 spec | interp elab_instance |
| D12 | rVPI ValueChange 投遞 | C-VPI 同步逐次排隊 | 槽邊界排水+寫爆發摺疊(VCD pending 同語義) | 結構等價;觀察面一致(內部交叉+corpus 對拍) | vpi_gate;ADR-0003 |
| D13 | rVPI EndOfSimulation | C 於 $finish 後回調 | 兩個 Ok 出口觸發,SimError 路徑不觸發 | 註冊分歧 | vpi_gate |
| D14 | rVPI finish/stop | vpiFinish/vpiStop 有別 | 合流(同 $stop 既有行為) | 與 TB 系統任務行為一致 | interp syscall |
| D15 | rVPI Transport/Inertial put | 脈衝消除有別 | 切片 2:兩模式同義(事件模型無脈衝消除) | 註冊分歧,切片 2 落地 | ADR-0003 D4 |
| D16 | 混合 timescale 檔序 | TIMESCALEMOD 警告預設致命(檔序相關) | 不報(通過) | 語料面已修(XLS 生成檔注入 timescale);引擎面對齊候補 | corpus-external/xls;REGISTRY 1023 |

## Incident register(工具鏈開發事故;五欄制)

| 日期 | 症狀 | 根因 | 修法 | 硬化層 |
|---|---|---|---|---|
| 10-21 | `task one(input integer n)` 解析器掛死 | sync 停結構關鍵字不消費+外層不強制前進 | 模組循環 pos 守衛;ANSI 端口收類型關鍵字 | AGENTS §4 + corpus(fuzz 覆蓋) |
| 10-21 | counter8 期望 `q=5` 錯 | NBA 區語義(讀到 4) | TB `#1` 後讀;oracle 即 spec | AGENTS §1 |
| 10-21 | `%0d` 補位/`%0Nd` 零填充兩次改 | 格式契約手猜 | golden 釘死 | corpus 11 號 |
| 10-21 | 36 號誤判 M7 | 語料殘留存儲器聲明 | 刪;R2/oracle warning 當語料審稿 | corpus_lint |
| 10-21 | 43 號競爭兩改一漏 | 寫在邊沿活躍區 | `@(posedge); #1; 寫` 模式 | corpus_lint R1 |
| 10-22 | 98→90% 回歸 | 錄製無 --trace 混入 `-Info` 行 | 錄製帶 --trace;歸一化精確前綴 | normalization_contract 測試 |
| 10-22 | 02 號被歸一化吃掉 | 過寬的裸 `-` 過濾 | 精確前綴 | 同上 |
| 10-22 | fuzz:lexer 三處 UTF-8 panic | 單字節步進/續字節當空白/盲 bump | 按字符處理 | fuzz_smoke 常駐 |
| 10-22 | oracle VCD 只有 2 標記 | 自寫 main 多此一舉 | `--binary --trace` 原生 \$dumpvars | scripts/oracle_vcd.sh |
| 10-22 | PLAN.md 漏提交 | `grep -c` 零匹配斷 && 鏈 | `|| true`+終檢 git status | AGENTS §10.5 |
| 10-23 | probe 移除 regex `[\s\S]*?` 吃掉 GenBlock 臂+三個後續臂 | 點星 regex 錨錨到 8 空格 `}` 跨臂貪吃 | 手術還原+鐵則:探針插拔也走錨點斷言手術,禁 regex 點星跨區 | AGENTS §10.6 |
| 10-23 | Scope.push 巢狀丟前綴(genblk1.foo.x→foo.x) | chain 段=完整路徑前綴,巢狀 generate push 未串接;單層語料掩蓋結構 bug | 三臂串接 innermost();外部語料(yosys)3 分鐘抓出 | 外部語料收編策略=PLAN §7.5 |
| 10-23 | `3'h0` 被拒(位數檢查) | 檢查「數字位寬」而非「值域顯著位」 | 值域檢查(3'h0 合法,3'h9 拒) | value.rs |
| `vl-difftest --html` | 報告需求 | 生成 docs/e2e-report/index.html |
| `vl-difftest --record` | 語義變更後 | 重錄 oracle 期望(人工讀 diff) |
