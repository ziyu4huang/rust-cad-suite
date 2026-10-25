# rust-cad-suite REGISTRY(組件登記簿 = 本 repo 的 CRM:組件×能力×閘×證據的關係表)

> 單一事實來源原則:此表只放「關係與指針」,不放瞬時值(通過率看報告頁,
> 由 `vl-difftest --html` 重生成)。改動組件職責必須同輪更新本表。

## Crates

| Crate | 職責 | 依賴 | 對應上游角色 | 證據/閘 |
|---|---|---|---|---|
| `rcs-diag` | spans/source map/診斷/caret 渲染(suite 共享) | 0 | verilator 的 %Error 輸出層 | 渲染 golden 測試 |
| `vl-syntax` | pp+lexer+parser+AST(V2005) | rcs-diag | VPreproc/V3Parser | lexer 12 + frontend 17 測試;E2E 全語料過解析閘 |
| `vl-sim` | 4-state 值/求值/展開/事件調度器(值域已提取=`vl-values`,src/value.rs 為 shim re-export;wait/fork/force 族硬錯自 parse 級遷 build 級,M7/M8 文字逐臂保留 interp.rs:1767-1785) | vl-syntax, rcs-diag, vl-values | VCSD/eval/scheduling | value 8 測試;sim_gate 2(counter8+fulladd byte-diff);e2e 59/59 零漂移 |
| `vl-cli` | bin `rverilator`(--version/-E/--lint-only/--dump-ast/--binary/--top-module) | 上全部 | verilator driver | 未知旗標=錯 exit1(對抗審 #8) |
| `vl-difftest` | oracle 錄製/差分閘/HTML 報告(dev-only, publish=false) | vl-syntax, vl-sim, rcs-diag | —(自建) | regression_gate ≥80% 斷言 |
| `vl-wasm` | WASM facade(cdylib):`run_verilog(_stdout)`/`WasmSim`,復用 CLI 頂層偵測;wasm-pack --target web,~500KB | `wasm-test/run.mjs` 差分閘(同 oracle goldens,6 例)入 regress.sh(pkg 過期自動重建) | ADR-0003 facade 慣例 |
| `vl-ffi` | C-ABI host facade(cdylib):`rcs_run(manifest)/rcs_version/rcs_free`,行式 manifest(file/inject);Bun bun:ffi 兩式(FFI 進程內/spawn subprocess) | bun-example 差分閘同 goldens 入 regress(dylib 缺=略過提示) | ADR-0003 facade 慣例 |
| `vl-spec` | Spec 驗證核心:Signals 句柄/Invariant trait/GoldenModel trait/LockstepSpec harness(put+run_until 成對紀律,ADR-0007) | vl-sim, vl-syntax, rcs-diag | —(自建) | spec_gate 4 測 |
| `spec-i2c` | 乾淨室 I2C master demo:RTL+golden model+匯流排驅動器+4 協議不變量+負向變種偵測(publish=false) | vl-spec, vl-sim, rcs-diag | OpenCores i2c(GPL,唯讀啟發零拷貝) | scripted/隨機 20 事務乾淨;LSB-first/no-ack-release 負向全被抓 |
| `vl-vcd` | VCD 單一核心:writer(IEEE §18 正典,4 慢字串介面;`$version` 身分自 C3 起可參數化 `program_name`,預設 rverilator 不變=金樣零漂移,riverilog 傳自標識=rivD1)+容忍式 reader(E1xx/W1xx,GTKWave 啟發子集)+SVG 渲染器(ADR-0005:確定性 XML 轉義)+`vcd2svg` bin | 0 | —(自建;GTKWave gw-vcd-loader 為研究參照) | reader+render 單測 15;golden=tests/goldens/counter.svg byte-diff(CLI/wasm 同字節);零漂移=e2e expect.vcd byte-diff;writer 雙身分單測(writer.rs:253/260) |
| `vl-values` | 4-state 值域共享葉:`Bits` 兩臂 repr(N/W)+limb 引擎+IEEE §5 表+N-vs-W property 機檢——自 vl-sim::value 提取(riv-r2-design §2),vl-sim 與 riv-kernel 共用 | **0**(純 std) | —(自建;兩引擎共用值域軌) | `cargo test -p vl-values`;提取零漂移=e2e 59/59+全量 cargo test 綠 |
| `riv-ir` | RIV-1:自有 versioned 文字 IR(var/thread/func 槽定址+每啟用幀;字面量 (width,signed,base,digits) 載入期才變 Bits)+手寫 writer/reader(零 serde) | **0**(純 std) | vvp 的 .vvp 角色(格式主權自有,OD1;out.riv 前拉 R6→R2=riv-r2-design §9-1) | writer→reader→writer 往返 byte-equal 單測;riverilog `-o` ↔ rivvp 載入貫通 |
| `riv-elab` | AST→設計裝配:平層 Phase A(top 偵測/decl/常量/寬度符號 lowering 烘焙/display fmt 編譯);結構層(例化/generate/array/閘)=R4,未支援構造=帶 `milestone R<x>` 硬錯(err.rs 分類) | vl-syntax, rcs-diag, riv-ir, vl-values | ivl 的 Parser/Elaborator(平層部分) | riv e2e 臂 42/59;17 個 pending 全帶 R4 指針(即本 crate 的硬錯面) |
| `riv-kernel` | 事件內核:分層隊列(active/inactive/NBA/monitor)+時間輪+顯式 PC 執行緒+force-overlay 層+每啟用幀呼叫+DumpSink(VCD 經 vl-vcd) | riv-ir, vl-values(**零 vl-sim 依賴**,PLAN 9.8-2) | vvp 直譯器(分層隊列=iverilog 契約) | riv iv 臂 8/10 byte-exact(含 03_nba 層次序/04_force 殘值);riv-difftest 7 單測 |
| `riv-cli` | bin `riverilog`(pp→parse→riv-elab→RIV-1 write)+bin `rivvp`(載入 RIV-1 直譯;iverilog 形 $finish 橫幅/VCD info 行) | riv-elab, riv-kernel, riv-ir, vl-syntax, rcs-diag, vl-vcd | iverilog+vvp 兩進制(流程相容,檔案自有) | cli_gate 9 goldens(R0 契約 8+01_hello/03_nba 真流程 byte-exact) |
| `riv-difftest` | riv 差分 harness(dev-only):`--e2e --corpus`(正規化+看門狗+ledger 判定)與 iv 臂(case.json enforced/pending)**按 corpus 路徑尾段選臂**;UNLEDGERED/DRIFT 文字訊號,執法在 regress grep | 無 lib(子程序驅動兩 bin) | —(自建) | 7 單測(正規化敵意 fixture/看門狗/DECL 兩形/臂選擇);regress 兩臂精確數基線 |

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
| lint parity 基線 83(regress.sh) | 每次交付 | M8 五輪基線(ADR-0008);閘曾攔下 78 誤算(ADR-0006 備忘) |
| riv-cli/riv-difftest(riverilog R0→R3) | riverilog/rivvp 契約面 goldens byte-exact(cli_gate 9 測=R0 契約 8+01_hello/03_nba 真流程)+ riv-difftest 7 單測(正規化敵意 fixture/看門狗/DECL 兩形/臂選擇=corpus 尾段) | RIVERILOG-PLAN.md §5/§8;rivD1-6 預載+rivD7 撤回+rivD8 於 §6/本簿 |
| riv-difftest 兩臂精確數閘(regress.sh) | 每次交付 | `SUMMARY e2e=42/59 iv=0/0` 與 `SUMMARY e2e=0/0 iv=8/10` 逐字 grep(基線隨能力同輪上調,AGENTS 10.9);UNLEDGERED/DRIFT 計數==0;runner exit 恆 0,執法全在 grep |
| `cargo test -q -p vl-spec -p spec-i2c` | regress.sh | spec 驗證層:lockstep/不變量/負向偵測/隨機 |
| D17 timer 期限推進 | — | rVPI AfterDelay 於自身期限準點觸發(舊=拖到下一 # 事件);vpi_gate 20000→16000 |
| `bun crates/vl-wasm/wasm-test/vcd_svg.mjs` | regress.sh | 三面同源渲染器:wasm one-shot==golden byte-diff+句柄重渲+u64 窗口+hostile 轉義+錯誤路徑 |
| `bun crates/vl-wasm/wasm-test/spec.mjs` | regress.sh | spec 層 wasm 閉環(ADR-0007):correct 0 違例/LSB-first+no-ack 負向指名/BigInt 跨界/雙跑 byte 確定性/未知變體硬錯 |
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
| rivD1-6 | riverilog 預載分歧(VCD header/$stop 互動/效能/`-o -`/sorry 文案/目標插件) | iverilog 13.0 | 政策見預載表 | SSOT=RIVERILOG-PLAN.md §6(2026-10-24 預載);落地時逐條回填本簿 | RIVERILOG-PLAN §6 |
| rivD7 | 48_frac_delay 延遲捨入(**預登記撤回**) | 設計期預測 iverilog 按 precision 捨入(1500/1750) | 同 | **2026-10-25 live probe 反證**:iverilog 13.0 數據行 `t=2000/t=2000` 與 verilator 金樣逐字一致,僅 finish 橫幅時刻(1750)剩差且屬 rivD8 丟行;48 現狀 pass、無需 ledger;編號留墓碑,32_proc_order 未來分歧=rivD9 | 本 session 探針(vvp -n 輸出 vs expected.stdout diff 僅 1 行);RIVERILOG-PLAN §6 |
| rivD8 | riv e2e 臂 stdout 正規化:丟 riv 自家工具行 | e2e 金樣(verilator 錄製)無 iverilog 形工具行 | rivvp 印 iverilog 形 $finish 橫幅/`VCD info: ` 行 | 丟行白名單=vl-difftest 四前綴 ∪ 兩工具行形,禁內容改寫;敵意 fixture(`-7div2=-3`、`tb.v:6: ` 設計行)單測釘死;iv 臂不丟(工具行=金樣) | riv-difftest/src/main.rs:241-285,669-714;riv-r2-design §9-3/§10 注 1 |
| rivD9 | ✅ 2026-10-25 兌現:e2e oracle-ref.vcd=verilator 2-state 錄製,與 riv(iverilog 形 4-state)不同側——t0 值域(q=0 vs x)與參數傾印(D3 家族);交叉閘以 iverilog 腿另錄金樣(oracle-ref.iv.vcd 入庫),樣本點交叉=vcd_gate 形+rivD1 正規化 | 51_vcd_counter 雙腿實證 | regress vcd-xcheck 閘(6/6 var 50 樣本點 0 失配) | RIVERILOG-PLAN §6/§8.1 |
| rivD11 | $random 序列分歧:iverilog 13.0 首二值 303379748/-1064739199,riv=LCG 1103515245/12345;IEEE 明文序列為 implementation-specific | r5probe.v 探針 | 行為面(可重現+種子 inout)實作;含原值案例不入語料 | RIVERILOG-PLAN §6 |
| rivD10 | 未寫入記憶體元素:iverilog 讀 x vs verilator 金樣 2-state 讀 0;riv 匹配 iverilog,53_readmem ledger 化 | 53_readmem expected.iv.stdout L2(xxxx vs 0000) | riv-pending.txt ledger(登記分歧不入紅燈);後續記憶體案例須全初始化或預期此分歧 | RIVERILOG-PLAN §6 |

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
| 10-25 | riv-difftest `--e2e --corpus corpus/iv-e2e` 把 8 個 enforced iv 案例全誤報 UNLEDGERED diff(修前 `SUMMARY e2e=0/10`;手跑 riverilog+rivvp 八例全 byte-exact 才定位) | 臂選擇誤看 `--e2e` 旗標而非 corpus 路徑尾段:iv 案例被推入 e2e 臂→正規化丟掉身為金樣的 iverilog 工具行+跳過 -g/plusargs/case.json | 臂選擇=corpus 尾段(`arm_label`,main.rs:658-663)+單測釘死(`arm_label_comes_from_corpus_tail`,main.rs:779-785);修後 `SUMMARY e2e=0/0 iv=8/10` | regress 兩臂精確數基線逐字 grep(錯臂即錯數)+UNLEDGERED/DRIFT==0;riv-difftest 7 單測 |
| `vl-difftest --html` | 報告需求 | 生成 docs/e2e-report/index.html |
| `vl-difftest --record` | 語義變更後 | 重錄 oracle 期望(人工讀 diff) |
