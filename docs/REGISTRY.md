# rust-cad-suite REGISTRY(組件登記簿 = 本 repo 的 CRM:組件×能力×閘×證據的關係表)

> 單一事實來源原則:此表只放「關係與指針」,不放瞬時值(通過率看報告頁,
> 由 `vl-difftest --html` 重生成)。改動組件職責必須同輪更新本表。

## Crates

| Crate | 職責 | 依賴 | 對應上游角色 | 證據/閘 |
|---|---|---|---|---|
| `rcs-diag` | spans/source map/診斷/caret 渲染(suite 共享) | 0 | verilator 的 %Error 輸出層 | 渲染 golden 測試 |
| `analyzer-core` | 統一分析 API(`analyze`+`Analysis` 投影視圖)+ASP/1 codec+§13 報告編碼(10.7 core-first 語義層) | vl-syntax,ry-ir,rcs-diag | —(自建,無 oracle) | `cargo test -p analyzer-core` 34 pass+1 ignored(json 17+api 13+codec 向量 4;2026-11-03 收平輪實測);`bun test`(同 fixtures 12);ADR-0011 |
| `analyzer-server` | bin `analyzerd`:UDS 面(握手/幀循環/ops/停機/殘留清理),零語義 | analyzer-core | —(自建) | `cargo test -p analyzer-server` 15 測(單測 3+回環 12;2026-11-03 收平輪實測);regress S5 `analyzer smoke` 塊 |
| `vl-syntax` | pp+lexer+parser+AST(V2005) | rcs-diag | VPreproc/V3Parser | lexer 12 + frontend 17 測試;E2E 全語料過解析閘 |
| `vl-sim` | 4-state 值/求值/展開/事件調度器(值域已提取=`vl-values`,src/value.rs 為 shim re-export;wait/fork/force 族硬錯自 parse 級遷 build 級,M7/M8 文字逐臂保留 interp.rs:1767-1785) | vl-syntax, rcs-diag, vl-values | VCSD/eval/scheduling | value 8 測試;sim_gate 2(counter8+fulladd byte-diff);e2e 59/59 零漂移 |
| `vl-cli` | bin `rverilator`(--version/-E/--lint-only/--dump-ast/--binary/--top-module) | 上全部 | verilator driver | 未知旗標=錯 exit1(對抗審 #8) |
| `rv-xls-sim` | XLS `VerilogSimulator` 形進程適配器:argv 與 stdin-JSON 兩入口共用 `vl_sim::USim`;stdout=設計輸出/stderr=診斷(CLI 的 Verilator 全-stdout 慣例不變);`.sv` 明確拒收;in-memory 文字落 scratch 目錄(相對 include/readmem 同真檔);JSON 子集零 serde | vl-syntax, vl-sim, vl-cli, rcs-diag | XLS `VerilogSimulator`(`iverilog_simulator.cc` 的同位插槽) | `cargo test -p rv-xls-sim`(10 unit+11 contract:add3/acc_pipe byte-equal×2、巨集、include、語法錯分流、`.sv`、JSON round-trip);regress「xls adapter」行;上游子類 `cpp/xls/simulation/simulators/`(對真 header 編譯,scripts/xls-cpp-gate.sh 10 項 harness);收割鏈=scripts/xls_harvest_manifest.py+xls_adopt.py(regress selftest);正典=docs/XLS-ADAPTER.md |
| `vl-difftest` | oracle 錄製/差分閘/HTML 報告(dev-only, publish=false) | vl-syntax, vl-sim, rcs-diag | —(自建) | regression_gate ≥80% 斷言 |
| `vl-wasm` | WASM facade(cdylib)雙面(ADR-0009):rverilator 面=`run_verilog(_stdout)`/`WasmSim`;riverilog 面=`riv_build`(→RIV-1 文本)+`riv_run`(rivvp 同核,iverilog 形橫幅);復用 CLI 頂層偵測;wasm-pack --target web,~1.2MB | `wasm-test/run.mjs`+`riv_run.mjs` 差分閘(oracle/iverilog goldens byte-exact)入 regress.sh(pkg 過期自動重建) | ADR-0003 facade 慣例+ADR-0009 §1 |
| `vl-ffi` | C-ABI host facade(cdylib):`rcs_run(manifest)/rcs_version/rcs_free`,行式 manifest(file/inject);Bun bun:ffi 兩式(FFI 進程內/spawn subprocess) | bun-example 差分閘同 goldens 入 regress(dylib 缺=略過提示) | ADR-0003 facade 慣例 |
| `vl-spec` | Spec 驗證核心:Signals 句柄/Invariant trait/GoldenModel trait/LockstepSpec harness(put+run_until 成對紀律,ADR-0007) | vl-sim, vl-syntax, rcs-diag | —(自建) | spec_gate 4 測 |
| `spec-i2c` | 乾淨室 I2C master demo:RTL+golden model+匯流排驅動器+4 協議不變量+負向變種偵測(publish=false) | vl-spec, vl-sim, rcs-diag | OpenCores i2c(GPL,唯讀啟發零拷貝) | scripted/隨機 20 事務乾淨;LSB-first/no-ack-release 負向全被抓 |
| `vl-vcd` | VCD 單一核心:writer(IEEE §18 正典,4 慢字串介面;`$version` 身分自 C3 起可參數化 `program_name`,預設 rverilator 不變=金樣零漂移,riverilog 傳自標識=rivD1)+容忍式 reader(E1xx/W1xx,GTKWave 啟發子集)+SVG 渲染器(ADR-0005:確定性 XML 轉義)+`vcd2svg` bin | 0 | —(自建;GTKWave gw-vcd-loader 為研究參照) | reader+render 單測 15;golden=tests/goldens/counter.svg byte-diff(CLI/wasm 同字節);零漂移=e2e expect.vcd byte-diff;writer 雙身分單測(writer.rs:253/260) |
| `vl-values` | 4-state 值域共享葉:`Bits` 兩臂 repr(N/W)+limb 引擎+IEEE §5 表+N-vs-W property 機檢——自 vl-sim::value 提取(riv-r2-design §2),vl-sim 與 riv-kernel 共用 | **0**(純 std) | —(自建;兩引擎共用值域軌) | `cargo test -p vl-values`;提取零漂移=e2e 59/59+全量 cargo test 綠 |
| `vl-ops` | Lowered-op 應用語義**單一源**(IEEE §5 coercion+真值表):adjust/parse_const/apply_{bin,bit,bit_xnor,bit_not,reduce,logic_and/or/not,cmp,case_cmp,shift,sel,concat,replicate}——riv-kernel 運行時(step_thread+eval_range 兩執行點)/riv-elab const 解釋器/vl-sim AST 求值**三面共呼**(ADR-0009 U-2);bitwise/bitnot 帶 signed=按整體表達式符號延展(雙 oracle 實證);logic x 表=IEEE 5-28 | riv-ir, vl-values | —(自建) | `cargo test -p vl-ops`(10 測:x 表/signed 延展/進位保留);U-2 七探針(verilator+iverilog)雙引擎逐位一致 |
| `riv-ir` | RIV-1:自有 versioned 文字 IR(var/thread/func 槽定址+每啟用幀;字面量 (width,signed,base,digits) 載入期才變 Bits)+手寫 writer/reader(零 serde) | **0**(純 std) | vvp 的 .vvp 角色(格式主權自有,OD1;out.riv 前拉 R6→R2=riv-r2-design §9-1) | writer→reader→writer 往返 byte-equal 單測;riverilog `-o` ↔ rivvp 載入貫通 |
| `riv-elab` | AST→設計裝配:平層 Phase A(top 偵測/decl/常量/寬度符號 lowering 烘焙/display fmt 編譯);結構層(例化/generate/array/閘)=R4,未支援構造=帶 `milestone R<x>` 硬錯(err.rs 分類) | vl-syntax, rcs-diag, riv-ir, vl-values | ivl 的 Parser/Elaborator(平層部分) | riv e2e 臂 42/59;17 個 pending 全帶 R4 指針(即本 crate 的硬錯面) |
| `riv-kernel` | 事件內核:分層隊列(active/inactive/NBA/monitor)+時間輪+顯式 PC 執行緒+force-overlay 層+每啟用幀呼叫+DumpSink(VCD 經 vl-vcd) | riv-ir, vl-values(**零 vl-sim 依賴**,PLAN 9.8-2) | vvp 直譯器(分層隊列=iverilog 契約) | riv iv 臂 8/10 byte-exact(含 03_nba 層次序/04_force 殘值);riv-difftest 7 單測 |
| `riv-cli` | bin `riverilog`(pp→parse→-y fixpoint→riv-elab→RIV-1 write;-t null=免寫;`-E`=共享 vl-syntax::pp 渲染器路由 a.out/-o/stdout;pp 錯=exit 1/parse=exit 2 分類;`-D` attached+separate;rivD4 `-o -` 編譯目標硬錯)+bin `rivvp`(載入 RIV-1 直譯;iverilog 形 $finish 橫幅/VCD info 行;-N=stop exit 1;-q=stdout 全靜 VCD 照寫;-l=stdout 全量鏡像;-M/-m=R7 指針硬錯;-i=受理 no-op,緩衝語義 batch 不可觀測) | riv-elab, riv-kernel, riv-ir, vl-syntax, rcs-diag, vl-vcd | iverilog+vvp 兩進制(流程相容,檔案自有) | cli_gate 31 測(R0 契約 8+01_hello/03_nba 真流程+U-3 前半:-t null 真行為/-E 路由/-D 兩形/-y transitive+-Y/-t null 免寫/pp=1 parse=2/缺模組 exit 2/-o- 硬錯/-i 指針/rivvp -N -q -l -l- -M-m+U-3b:-g 登記冊 255/feature 受理/-T 驗證/-W 已知未知/-S 指針/-P 非致命回退/-f-c cmdfile/-M depfile 兩倍庫條目/-i 真實作/-o 寫檔失敗 255/rivvp -l - 鏡像) |
| `riv-difftest` | riv 差分 harness(dev-only):`--e2e --corpus`(正規化+看門狗+ledger 判定)與 iv 臂(case.json enforced/pending)**按 corpus 路徑尾段選臂**;UNLEDGERED/DRIFT 文字訊號,執法在 regress grep | 無 lib(子程序驅動兩 bin) | —(自建) | 7 單測(正規化敵意 fixture/看門狗/DECL 兩形/臂選擇);regress 兩臂精確數基線 |

| `ry-ir` | RTLIL 對象模型+行導向 lexer/遞迴下降 parser+`write_rtlil -readable` 打印機(ADR-0010 IR 主權層;twines/$priv@/parameter real=Y1 硬錯) | **0** | yosys kernel/rtlil_{frontend,backend} | roundtrip 閘 55 例 oracle .il byte-exact(含 P-bearing 層) |
| `ry-cli` | bin `ryosys`:兩層 registry(`-H` 普查 306 條=登記層 stub 265+實現層 41[M3 續弧增 log/help/clean/opt_balance_tree;批量收編第二批 2026-11-02 增 async2sync/pmuxtree/dffunmap/setattr/torder/edgetypes][M0 五命令+M1 增 hierarchy/read_json/simplemap,M2 增 proc 家族 11+opt_expr -keepdc,M3 增 opt 編排器+opt_clean/merge/muxtree/reduce/dff/share/lut/lut_ins/demorgan/ffinv;餘 Y2=6:optbarriers/opt_hier/opt_mem 族×4],Y0..Y3)+`-p` 尾分號切分契約+check/stat/write_json 移植面(dict/pool 逆插入序契約;boxed 模組靜默跳過;M1-c 組合環=celledges.cc 全 dispatch+參與條件;M1-d simplemap=mapper 家族+FfData slice/emit+hashlib swap-and-pop 序;wand/wor 多驅動解析=hierarchy.cc:1364-1481 段+1485/1524/1526/1517 二進制行號錨;M2 續弧=IdString **intern 首見序**法條[rtlil.h:413 operator< 比 intern index 非 String 字典序;Design.intern_seq 檔案序登記+autoidx_base 分界派生建名序,消費者=opt_expr TopoSort]+opt_expr 除零規則[opt_expr.cc:1786-1808]+proc_clean hoist range-insert-at-begin[proc_clean.cc:72]+proc_dlatch per-module rules_db 作用域[proc_dlatch.cc:145-146]) | ry-ir | yosys driver+passes/cmds+backends/json | cli_contract 45 測(2026-11-02 實測 45 passed;M3 續弧批量收編=log 三形+七旗標 Y2+help bare/子形 Y2+clean 三形/selection D-ys10+;;; 端到端+obt -arith/-logic Y2;批量收編第二批=六命令 bare/未知旗標 cmd_error/已知旗標 Y2/selection D-ys11/setattr decode 逐字);ys-e2e **469/479** 四件套(M3 -pbo 層 54 例+第二批 anchors 9 遷入 8 入 compare 面[decode 錨 expected.rtlil 面外];known-gap 10 例=opt_dff 反饋面[D-ys8c 面1 內層 pattern_t 序新子面]+D-ys8c 面2 hash 序+D-ys8 序差,名單逐字入 regress S4)+LIVE ys=456/456 |
| `ry-difftest` | ryosys oracle 差分 harness(dev-only):`--record` 全量兩層錄制+`--corpus` 四件套對拍(exit/stdout 歸一化/stderr 逐字/rtlil/json);歸一化敵意 fixture 單測釘死 | 無(子程序驅動) | —(自建) | 7 單測+regress「ryosys」塊精確數 ys-e2e 450/460+10 例 known-gap 名單逐字+LIVE 456/456(基線歷史:M1 弧末 402/402→M2 實作輪 401/406[5 known-gap]→M2 續弧收平 406/406→M3 實作輪 418/460[42 known-gap]→續弧第 2 輪 429/460[31 known-gap]→續弧第 7 輪 450/460) |
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
| -E preprocessor parity(iverilog -E vs riverilog -E,oracle-optional)| regress.sh | e2e 59 例正規化同位 59/59(剝註解+移除全部空白;字符串引號、based-literal 底線=契約,31/60 探針釘死);正規化收據=RIVERILOG-PLAN §5 U-3b 第三刀 |
| agent toolbox smoke(`scripts/rv-sim/rv-diff/rv-wave/riv-run.sh`)| regress.sh | 四支 agent 腳本各對一個入庫 golden 實跑:rv-sim=byte-exact、rv-diff=正規化 MATCH、rv-wave=SVG 帶 `<svg`、riv-run=iv 金樣 byte-exact;正典=`.agents/skills/rv-lab/SKILL.md`(agent→skill→script→CLI 層) |
| `cargo test -p rv-xls-sim` + regress「xls adapter」 | 每次提交/交付 | 21 測(10 unit+11 contract);argv add3 byte-exact、syntax check 靜默、JSON ok、`.sv` 拒收;正典=docs/XLS-ADAPTER.md |
| `scripts/xls_harvest_selftest.sh`(regress) | 每次交付 | manifest sha256+adapter syntax 閘、adopt 兩階段(byte-identical/idempotent/normalization/竄改全拒);無網路 |
| `scripts/xls-cpp-gate.sh`(`WITH_XLS_CPP=1`) | opt-in(需網路+protoc+abseil) | pinned google/xls header 下載→protoc→對真 header 編譯→10 項 harness(add3 byte-exact/巨集/include/語法錯/`.sv`);in-tree Bazel 未驗=明標 TODO |
| difftest `--min-pass 100`(regress.sh) | 每次交付 | e2e 精確數閘(80% 門檻曾讓 55/57 藏身綠燈,1023 教訓) |
| lint parity 基線 83(regress.sh) | 每次交付 | M8 五輪基線(ADR-0008);閘曾攔下 78 誤算(ADR-0006 備忘) |
| riv-cli/riv-difftest(riverilog R0→R3) | riverilog/rivvp 契約面 goldens byte-exact(cli_gate 31 測=R0 契約 8+01_hello/03_nba 真流程+U-3 前半+U-3b 旗標長尾契約面;vl-cli cli_gate 5 測=--version/--help/-E pp golden/-I-D smoke/-V=VL-E030 錯誤路徑)+ riv-difftest 7 單測(正規化敵意 fixture/看門狗/DECL 兩形/臂選擇=corpus 尾段) | RIVERILOG-PLAN.md §5/§8;rivD1-6 預載+rivD7 撤回+rivD8 於 §6/本簿 |
| riv-difftest 兩臂精確數閘(regress.sh) | 每次交付 | `SUMMARY e2e=58/59 iv=0/0` 與 `SUMMARY e2e=0/0 iv=21/22` 逐字 grep(基線隨能力同輪上調,AGENTS 10.9);UNLEDGERED/DRIFT 計數==0;runner exit 恆 0,執法全在 grep |
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
| D16 | rVPI vc 排水節奏雙面:legacy Sim=槽邊界三排水點(寫爆發摺疊窗=槽),統一引擎 USim=kernel 單漏斗每 commit post-settle 即排水(摺疊窗=單 commit);CbInfo.time 兩面皆=提交 tick,值語義(last-wins)不變 | 2026-10-26 U-4 slice-2 實測(usim_vpi_gate usim_firing_order_parity 對 vpi_gate firing_order_and_nba_visibility) | 結構等價面=註冊面/觸發點/host 讀寫同源;節奏差隨面走,ADR-0003 D5 修訂 | usim_vpi_gate 3 測;ADR-0003 D5 |
| D17 | rVPI 声明初值事件:統一引擎把 Reg 聲明初值降為 t0 initial 線程(@* 喚醒語義,21_always_comb 金樣依賴),x→init 是真實變化→cbValueChange 可見;legacy 面 build 時靜默套初值,t0 寫 no-op→不可見 | 2026-10-26 U-5 slice-1 探針(lfsr wasm vc=9 首事件 0:ac;usim 1-bit 面 2-state 投影 x≡0 寫 0 no-op 故不可見) | 統一引擎事件語義為準(初值=t0 事件);wasm features gate 期望更新為 9 | wasm features 2/2;ADR-0003 D5 |
| D16 | 混合 timescale 檔序 | TIMESCALEMOD 警告預設致命(檔序相關) | 不報(通過) | 語料面已修(XLS 生成檔注入 timescale);引擎面對齊候補 | corpus-external/xls;REGISTRY 1023 |
| D18 | XLS 適配器 `.sv` | XLS 預設 iverilog 以 `-g2012` 受理部分 SystemVerilog | 明確拒收(exit 1) | 範圍外=硬錯,milestone;XLS 測試若要 SV 先擴引擎,不在適配器靜默改寫 | rv-xls-sim contract `systemverilog_is_refused_explicitly` |
| D19 | XLS 適配器診斷流 | XLS 介面把 iverilog 錯誤放 stderr | CLI 維持 Verilator 全-stdout;適配器把診斷導到 stderr、設計輸出留 stdout | 只有適配器這張臉分流;CLI 契約不動(regress e2e 仍 byte-exact) | docs/XLS-ADAPTER.md;contract `parse_error_goes_to_stderr` |
| rivD1-6 | riverilog 預載分歧(VCD header/$stop 互動/效能/`-o -`/sorry 文案/目標插件) | iverilog 13.0 | 政策見預載表 | SSOT=RIVERILOG-PLAN.md §6(2026-10-24 預載);落地時逐條回填本簿;rivD4 ✅ 2026-10-25 兌現=編譯目標 `-o -` 硬錯 exit 1(`-E -o -`=stdout 正當;oracle 實況=真的建出檔名 `-` 的檔,政策性分歧 cli_gate 釘死) | RIVERILOG-PLAN §6 |
| rivD7 | 48_frac_delay 延遲捨入(**預登記撤回**) | 設計期預測 iverilog 按 precision 捨入(1500/1750) | 同 | **2026-10-25 live probe 反證**:iverilog 13.0 數據行 `t=2000/t=2000` 與 verilator 金樣逐字一致,僅 finish 橫幅時刻(1750)剩差且屬 rivD8 丟行;48 現狀 pass、無需 ledger;編號留墓碑,32_proc_order 未來分歧=rivD9 | 本 session 探針(vvp -n 輸出 vs expected.stdout diff 僅 1 行);RIVERILOG-PLAN §6 |
| rivD8 | riv e2e 臂 stdout 正規化:丟 riv 自家工具行 | e2e 金樣(verilator 錄製)無 iverilog 形工具行 | rivvp 印 iverilog 形 $finish 橫幅/`VCD info: ` 行 | 丟行白名單=vl-difftest 四前綴 ∪ 兩工具行形,禁內容改寫;敵意 fixture(`-7div2=-3`、`tb.v:6: ` 設計行)單測釘死;iv 臂不丟(工具行=金樣) | riv-difftest/src/main.rs:241-285,669-714;riv-r2-design §9-3/§10 注 1 |
| rivD9 | ✅ 2026-10-25 兌現:e2e oracle-ref.vcd=verilator 2-state 錄製,與 riv(iverilog 形 4-state)不同側——t0 值域(q=0 vs x)與參數傾印(D3 家族);交叉閘以 iverilog 腿另錄金樣(oracle-ref.iv.vcd 入庫),樣本點交叉=vcd_gate 形+rivD1 正規化 | 51_vcd_counter 雙腿實證 | regress vcd-xcheck 閘(6/6 var 50 樣本點 0 失配) | RIVERILOG-PLAN §6/§8.1 |
| rivD11 | $random 序列分歧:iverilog 13.0 首二值 303379748/-1064739199,riv=LCG 1103515245/12345;IEEE 明文序列為 implementation-specific | r5probe.v 探針 | 行為面(可重現+種子 inout)實作;含原值案例不入語料 | RIVERILOG-PLAN §6 |
| rivD10 | 未寫入記憶體元素:iverilog 讀 x vs verilator 金樣 2-state 讀 0;riv 匹配 iverilog,53_readmem ledger 化 | 53_readmem expected.iv.stdout L2(xxxx vs 0000) | riv-pending.txt ledger(登記分歧不入紅燈);後續記憶體案例須全初始化或預期此分歧 | RIVERILOG-PLAN §6 |
| rivD12 | -E 缺檔:oracle ivlpp segfault(exit 139,印 partial 錯誤後 crash)vs 我方乾淨 pp 類錯 exit 1——oracle bug 不復刻;正常編譯缺檔 U-3 前半 pp 分類修正後兩側皆 exit 1 對齊(訊息文字自由=rivD5),不再列分歧 | 2026-10-25 U-3 探針:iverilog -E missing.v → "missing.v: No such file or directory"+Segmentation fault: 11 | 我方=VL-E010 乾淨 pp 類錯 exit 1(cli_gate pp_class_errors_are_exit_1) | RIVERILOG-PLAN §6/§8 |
| rivD13 | 寫檔失敗類別:**已對齊(2026-10-26 U-3b 兌現)**——ENOENT/EACCES/EISDIR 三形再實測 oracle 皆為 `<path>: <errno 文>` + `error: Code generator failure: -1` 兩行、**exit 255**,我方 write_failure 同形同碼;errno 文映射 2/13/21,未知 errno 維持舊 exit 2 形 | 2026-10-25 P6 探針+2026-10-26 三失敗形複測(iverilog -o /nope_dir|noperm|dir) | 已對齊(cli_gate output_write_failure_is_exit_255);行留檔=對齊收據 | RIVERILOG-PLAN §5 |
| rivD14 | `vvp -l -` 契約 **已釘死並實作(2026-10-26 U-3b)**:stderr 收到 stdout 內容的**全量副本**(非 man page 的「-=stderr」);`-q -l -` 時 stdout 靜、stderr 副本存活;無 `-` 檔建出。我方=eprint 輸出緩衝至 stderr,實作即此模型 | 2026-10-26 雙探針(vvp -l - 與 vvp -q -l -,stdout/stderr 分捕) | 已實作(cli_gate rivvp_dash_l_dash_mirrors_streams;舊硬錯測 rivvp_dash_l_dash_is_hard_error 隨能力翻轉移除) | RIVERILOG-PLAN §6 |


### ryosys 分歧(RYOSYS-MVP §4.3;oracle=suite 內 yosys 0.70+179)

| # | 領域 | oracle | 我方 | 政策 | 證據 |
|---|---|---|---|---|---|
| D-ys1 | 版本/構建串 | banner 版本行/Generated-by/creator/Logfile hash/MEM/Time spent 含構建路徑與哈希 | 歸一化剝除;`ryosys -V` 自標識 `ryosys <version>` 不冒充 | 機器相關字段,錄製/對拍兩端同剝 | ry-difftest tests/normalize.rs |
| D-ys2 | 未實現命令 | 已知命令(306 條普查)能跑 | 登記層命中=`ERROR: <cmd>: not implemented in ryosys (milestone Yx)` exit 1 | 「未支援=帶里程碑硬錯」政策本身,隨弧回收;普查外名字逐字重現 oracle 錯(非分歧)。註:`read_sverilog` **不在普查 306 內**(0.70+179 併入 read_verilog)=普查外名字逐字 No such command,非 Y3 stub——RYOSYS-MVP §1.2/§1.3 舊映射文字為 M0 筆誤,以 §7.0-1 勘誤為準 | ry-cli cli_contract.rs(read_sverilog_is_not_a_survey_command) |
| D-ys3 | write_rtlil 默認形 | twines 壓縮形輸出成功 | Y1 硬錯,只承諾 `-readable` | M0 只釘傳統全名形 | cli_contract write_rtlil_default_form_is_y1 |
| D-ys4 | 檢查器路徑 | check 錯誤內嵌 `__FILE__` 構建路徑 | 若入 golden,歸一化剝 `/work/_builds/…` 段 | 同 D-ys1 | (預防性登記;M0 語料未觸發) |
| D-ys5 | check 組合環警告的**環起點旋轉** | 環內節點序依 0.70+179 內部序(interning/hash),8 探針實測:同名集 {x,y}→恆 x 起、{a,b}→恆 b 起、{aa,zz}→隨宣告朝向翻轉——v0.70 clone 的 TopoSort(std::map 節點序)+check.cc 無法推導 | 我方輸出同一環的**正確內容**(cell/edge/wire/source 行)但起點旋轉可能不同 | M1-c ✅(2026-10-27 補完,第二輪):偵測+內容實現=check.cc 參與條件+celledges.cc 全 dispatch(ff 無 D→Q 邊/arith 精確邊/detail_costly fallback);環 fixture 不入 SUMMARY 語料(避免以非 byte-exact golden 假綠);完整旋轉復刻=需建模二進制內部序,後置。**M2 續弧連動注記(2026-10-29)**:opt_expr 族已建模 IdString intern 首見序(ry-cli opt_expr.rs intern_key),但 check 環起點**不隨之收斂**——探針 {a,b}→恆 b 起與純 intern 序矛盾(a 先 intern,起點若=intern 最小應 a 起),環起點屬另一內部序,登記保持 | 本輪 8 探針掃描(a/b,c/d,x/y,m/n,b/a,y/x,aa/zz,zz/aa)+loop2not/loop3and/loopsrc 三 golden 錄製實證;M2 續弧評估=RYOSYS-MVP §8.8-4 第 5 點 |
| D-ys6 | 空設計 stat/check/hierarchy(2026-10-28 對抗審 finding) | 空設計(無 read_* 先行)下 `stat`/`check`/`hierarchy` 均 rc=0 成功:stat 印 `1. Printing statistics.`、check 印 `Executing CHECK pass`+`Found and reported 0 problems.`、stderr 0 字節(2026-10-28 兩端探針實錄) | rc=1+自創 `ERROR: No design object available (run 'read_rtlil' first) while executing command `<cmd>'`(main.rs:174 `{cmd}` 通形+:306 hierarchy 字面形)——文案 clone v0.70 全樹 grep 零命中=ryosys 自創,oracle 無此錯誤路徑 | 登記在案先於繼續(AGENTS 10.5);語料 402 例全部先 read_rtlil、golden 面不觸,非 M1/M2 驗收阻塞;**M3 已清償(2026-10-30)**:main.rs 自創文案全刪(rg exit=1 負向閘),空設計=Design::default() 語義,oracle rc=0 形+empty_design fixture 在檔(cli_contract::empty_design_three_commands_rc0);兩端逐字母本=RYOSYS-MVP §9.0-P9 | 兩端探針:oracle rc=0×3+stderr 0B vs ryosys rc=1×3+逐字 ERROR;`grep -rn "No design object available" vendor-reference/yosys/`=0;本行落檔前 REGISTRY/RYOSYS-MVP/PROVENANCE 對該文案 rg 查空 exit=1(登記欠賬本體,本行即清償) |
| D-ys8 | opt_clean/opt_merge 多刪 swap-and-pop **幸存序差**(2026-10-30 M3 實作輪;2026-10-31 續弧第 7 輪部分清償) | -pbo 序差類案例:被刪 cell/wire **集合與 oracle 完全相同**,剩餘元素在 dict entries 陣列中的排列不同(logic 等價、dump 序不同)。**第 7 輪兩個子面已復刻清償**:①delete_wires 批次序=wire_at 逆掃收集→pool 填入→Module::remove 以 pool 迭代 erase 的**雙反轉=檔案序(插入序)升序**(rotate-pbo 814 條 dump 序 Python 雙向模擬各釘一端,opt.rs delete_wires 排序鍵已改升序);②multiplier-pbo 根因=**設計內模組亦是 known cell**(celltypes.h:50-60 setup_design;模組型輸出端口=子模組 wire port_output)使 exact 分支先行 promote——非序差,opt.rs exact_cells/tracing 兩消費面已擴,該例自本家族除名(logic-diff 清償) | 殘差=loops/memory/mem_arst 的 rtlil dump 序差(stdout 已綠;loops 殘差另有 proc_mux 建樹編號序面) | 單相 swap-and-pop 四假設不達(原探针)+雙反轉檔案序模擬 byte-exact(第 7 輪);**殘差維持禁硬磕**(oracle 多輪 signals 迴圈×刪中插 $delete_wire$N×swap-and-pop 的完整動態=重寫二進制內部態) | regress S4 10 例名單;probe 數據=PROVENANCE §P11+續弧第 7 輪節 |
| D-ys9 | opt_dff $dlatch 的 **AD 常數→ARST 分支靜默+前移**(2026-10-30 續弧第 2 輪) | 0.70+179 二進制:$dlatch 的 AD 全常數分支無 log 且先於 inactive/active 分支(ARST 措辭隨後);clone 28ba3cb 同分支有 log `Changing const-value async load to async reset` 且居於 inactive/active 之後 | 我方按二進制行為實現(靜默+前移),clone 行號只作定位 | 最小探针矩陣(EN×D 的 const/wire 四象限+雙極性,8 例實錄:PROVENANCE §P11 續弧節);$dlatch 二進制語義=AD 常數時按 arst 模型處理(EN active→const driver/EN inactive→q=init/x/EN wire→arst_to_aload 回 $dlatch+constbits 折疊) | regress S4 10 例 known-gap 名單;ry-cli opt_dff.rs optimize_aload 頭註 |
| D-ys8c | opt_reduce/opt_dff 的 **pool hash 迭代序**(2026-10-30 續弧第 6 輪;2026-10-31 續弧第 7 輪面拆分)| 三面拆分:**面3(logic-diff,已清償第 7 輪)**=opt_pmux 建名記帳 cell=NEW_ID(N)/wire=NEW_ID(N+1) 每對吃 2 個 autoidx(opt_reduce.cc:131/:137;golden 錨=constmuldivmod-pbo cell$134→wire$135),opt.rs opt_pmux_reduce 已改雙號;**面1(internal-order,維持登記)**=patterns_t=pool(pattern_t) hash 迭代序(binary=0.70+179 sha1 9534a7f4d;clone v0.70=set 確定序,結構差=源碼不可導出的證據);**面2(internal-order,維持登記)**=opt_pmux grouped_b_to_s dict hash 序→\B/\S 拼接序 | 我方面1=DFS 確定序、面2=首見序(hash 序=二進制內部態,禁硬磕) | constmuldivmod 實錄(第 7 輪):修面3 後 3.1/3.2/3.5/3.7 面對齊、殘差=3.4 New ctrl vector 拼接序+3.11 二輪行為差(面2 下游);**面2 成立例=constmuldivmod/usb/values/fsm**(首分歧=New ctrl vector 拼接序,實跑在案;**fsm 補名=2026-11-02 批量收編第二批規劃輪**,見下句勘正注);**fsm/subbytes 第 7 輪初稿誤歸面2,對抗審修訂歸 (a) 類**(首分歧=3.8 OPT_EXPR 變換集差:3.1-3.7 兩端逐字同含 suppressed、fsm 我方 ~5 vs golden ~1、subbytes 我方 +~2 golden 無行,面早於 opt_dff/patterns 階段;fsm 另帶 never-active EN 塌縮=connecting-SRST vs Adding-EN 級聯,logic-diff 面)——**勘正注(2026-11-02 批量收編第二批規劃輪,三層疊加調和)**:初稿「誤歸」裁定只對**首分歧層**(3.8 變換集差歸 (a) 類)成立、不變;2026-11-02 規劃輪 fsm 對拍親證其**另帶第三層**=3.11 OPT_REDUCE「New ctrl vector for $pmux cell」拼接序差(oracle {opt_pmux$105, 16_CMP, 24_CMP, 67_CMP} vs 我方 {67, 24, 16, 105})=**面2 同形態**(grouped_b_to_s dict hash 序→拼接序),故 fsm 補入面2 成立例名單——「fsm 曾裁定不屬面2」指其非純面2 案例(首分歧在前層),「fsm∈面2 成立例」指 3.11 層面2 序差在案,兩語並存=分層表述非自相矛盾;subbytes 親證無 New ctrl vector 行=面2 不觸發,不補名);面1 登記本體=patterns pool 迭代序面(subbytes 的 Adding-EN 行序差類);**sincos/i2c 的 pattern 內容差自面1 摘出=獨立 logic-diff 面(對抗審分類統一,與 regress (a) 類/PROVENANCE 第 7 輪一致)**:oracle 深 else 鏈多收集 ctrl 鍵(o5 實錄 $ne$177 A 位數不同),行動=後置修我方(opt_dff.rs find 分支遍歷),非禁硬磕——**內容面已清償(2026-11-02 批量收編第二批 slice-2)**:opt_dff path_else 三點手術(循環外建+循環內 push 累加+A 臂傳累加版,oracle simple.cc:149/:156/:168;path_else_dummy 刪除)修後 i2c stdout 翻綠(殘差僅 rtlil/json)、sincos 殘差縮至 1 行+編號級聯;**殘差=面1 同族 hash 內部態(禁硬磕維持)**:外層 patterns_t pool 迭代序(既有登記)+**內層 pattern_t 迭代序(新子面,二進制探針仲裁)**=pattern 序律四候選(intern 升序/lt_by_name 字典序/正插/逆插)無一通殺(genref 探针同碼路徑異序:{sa,sb} 正插形 vs {sc,sa} 逆插形;sincos {procmux$107_CMP,start} autoidx 形——探针+判據=corpus/ys-batch2-anchors/probe-pattern-order/),hash 桶序=二進制內部態,案例維持 known-gap(461/471 基線不翻,RM11-1 分類軌) | regress S4 10 例名單;opt.rs opt_pmux_reduce(面3 已修);opt_dff.rs try_merge_ce(面1) |
| D-ys8b | opt_clean compare_signals 尾分支序(**2026-10-31 續弧第 7 輪清償**;原登記=internal-order 歸因「oracle 真序=建名序」)| **歸因修正(二進制證偽)**:oracle compare_signals 私名-私名對尾分支(wires.cc:142)呼 lt_by_name(rtlil.h:386-403)=Substrings memcmp **byte-wise 字串序**(interned 名=整串),非 intern 序亦非建名序;五個受控實驗(vA..vE)+9/9 覆蓋案例兩腿實跑收斂字串序(attrib04 vE 鏡像實驗逐字重現 oracle 選擇);public 對先走 isPublic(rtlil.h:548 begins_with("\\\\"))分支 | 我方原用 intern_key(檔案序)+check_public_name 兩缺陷 → 已修:尾分支改 n2.as_bytes()<n1.as_bytes()、public 述語改 starts_with('\\\\') | **已清償**:attrib04_net_var/retime/aes_kexp128/always03/arrays01/mem2reg_bounds_tern/scopes/fiedler-cooley/func_width_scope 九例兩腿翻綠(golden 零重錄);另破 case=autoidx 同步(W.autoidx 進出場同步 optimize_* 引用遞增)收平 attrib04/aes/always03 的 autoidx 尾差 | 實驗+9/9 兩腿實錄=PROVENANCE 續弧第 7 輪節;**同族字串序第三例(2026-11-02 批量收編第二批)**=torder `TopoSort<IdString, sort_by_id_str>`(torder.cc:81;sort_by_id_str=lt_by_name,rtlil.h:794)=自指定字串序,torder.rs 全鏈 byte-wise 名序禁 intern_key,carryadd 錨 byte-exact |
| D-ys10 | M3 續弧批量收編四單元**旗標/子形/selection 行為面**(2026-11-01 實作輪;§10.2 逐項裁定)| ①`clean <selection>`:extra_args(select=true 預設,register.h:102)**接受 selection 並正常執行** rc=0(本輪 `clean carryadd/w:*` 親證);②`help` 子形:未知名/-foo=`No such command or cell type: X` **rc=0**(log 非 error)、`help <名>`印全文、`-all`/`-cells`/兩參 `-dump-*-json` 真實功能(register.cc:1067-1164);③`log` 七旗標(-stdout/-stderr/-nolog/-n/-header/-push/-pop)真實旗標語義全 rc=0(本輪七形親證:logcmd.cc:66-114);④opt_balance_tree `-arith`/`-logic` 真實型別置換(opt_balance_tree.cc:377-385) | ①=cmd_error rc=1(D-ys2 同形);②③④=Y2 硬錯(milestone 契約形;§10.2 邊界=子形全文管線/cell 幫助表/-push -pop pass 計數器本弧不收)——fixture 钉我方硬錯形(cli_contract clean_cmd_selection_divergence/log_flag_*_y2_error/help_subform_y2_error_probes/opt_balance_tree_flags_stub_y2_probes),oracle 實錄形與我方硬錯永不相等=禁實錄 golden | 屆時收平入口:①=extra_args selection 機制(select 語料);②=-H 全文管線+cell 幫助表(§10.2);③=-push/-pop pass 計數器;④=五型置換旗標。全部=獨立續弧 slice,非本弧範圍 | 本輪探针實錄=PROVENANCE「M3 續弧·批量收編普查」節(P1⑥/P3/P5/P7) |
| D-ys11 | 批量收編第二批六命令**selection/已知旗標行為面**(2026-11-02 實作輪;§11.1-8/§11.2 逐項裁定,同 D-ys10 族) | ①六命令 selection 裸字(如 `async2sync MyMem/*`、`torder top/*`):oracle 經 extra_args 接受並執行 rc=0(本輪 async2sync MyMem/* 親證);②已知旗標真實語義:async2sync `-nolower`(跳過 chformal -lower,rc=0)、dffunmap `-ce-only`/`-srst-only`(+雙投=互斥 `ERROR: Options -ce-only and -srst-only are mutually exclusive!` rc=1,空跑親證)/torder `-stop <type> <port>`(cell 行集變,rc=0 親證)/`-noautostop`(rc=0);③`torder -stop` 單投(缺參)=oracle cmd_error rc=1(語法面一致但走 cmd_error 非 Y2 形) | ①②=Y2 硬錯/我方 cmd_error(milestone 契約形;selection=cmd_error rc=1、已知旗標=Y2 文案)——fixture 钉我方硬錯形(cli_contract batch2_six_* 組),oracle 實錄形與我方硬錯永不相等=禁實錄 golden | 屆時收平入口:①=extra_args selection 機制;②=旗標真實語義(-stop 雙參 TopoSort 停止面/-nolower chformal 面/互斥檢查);③=cmd_error 對齊。全部=獨立續弧 slice | 本輪探针實錄=PROVENANCE「批量收編第二批普查」節+§11.0 Q8 |

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
| 10-24 | 2026-10-23 的 `~`(bitnot)「IEEE Table 5-22 自決定」修復實為 **oracle 分歧**:legacy 面 `~4'b1000` 於 8-bit unsigned ctx 出 `00000111`,兩 oracle(verilator 5.050+iverilog 13.0)皆 `11110111`;U-2 求值合流時雙引擎並排七探針揭發 | 修復證據=自家 lockstep golden(I2C demo)而非 oracle,違 AGENTS §1(oracle 贏 LRM)未察覺;語料無 `~` 案例,金樣無從仲裁 | U-2 單一語義源(vl-ops)統一裁定:~ 與 bitwise 皆 context-determined(運算元按整體表達式符號延展後運算,嵌套吃 ctx 進位保留);legacy 翻案回歸;riv 同輪補 IR signed 域(Op::Bit/BitNot +signed),附帶抓兩真 bug=riv bitwise 恆零延展(雙 signed 探針 z2)+riv logic-op 遇 x 直接 abort(oracle=x) | AGENTS §1「修對了還是修偏了必須對 oracle 讀」;vl-ops 10 單測釘死四形;vl-sim eval.rs 註解翻案記錄;七探針 /tmp 可重現(w/t/u/ws/z2/m/lx) |
| 10-26 | regress 的 cargo test 間歇 FAIL(`parse_error_goes_to_stderr`;單跑 3 次才重現 1 次) | Scratch 目錄名僅 pid+nanos,並行 contract 測試同 tick 撞名→共用同一份 `top.v`,一測的 Drop 還刪掉另一測的目錄(可能解析到別人的合法設計) | scratch 名加 process-local AtomicU64 序號+`create_dir`(撞名即錯);新增 `scratch_directories_are_unique` 單測;contract 40 連跑零失敗 | rv-xls-sim 契約測試 + regress |
| 10-26 | U-5 slice-2 遷移 spec 層後 I2C lockstep 全癱(FSM 恆 IDLE、時間推進但 state/cnt 恆 0) | 三層疊加:①USim rVPI 表→VarId 映射按宣告序建,但 VpiState::build 按名排序→句柄全體錯位(寫 prescale 落 cnt、clk 落 scl_i);②kernel 無 run_until,stepwise 移植時 Ready 播種被鎖進 started 首跑分支(put 跨 run 喚醒的線程沒有隊列);③kernel vpi_write_var 不按宣告寬截斷(64-bit put 落 1-bit reg,legacy vpi_write_bits 有截斷)——探針鏈:probe-store 證 store 路徑通但 waiters=0→probe-var 證無重名→probe-arm 證 sens var 0 vs 寫 var 3=映射錯位 | ①映射改從排序後行序推導(排序即凍結序);②run_steps 每次入口播種 Ready+deadline=絕對調度點(外部時鐘無事件也推進 now);③vpi_write_var 經 Limbs::padded+pack 截斷;負面閾值按統一引擎重測重釘(negative_random 33@160cycles,scenario 維持 ≥40) | usim_vpi_gate/lockstep/negative_random/scenario 四閘;臨時探針(probe-wake/store/arm/var)五層剝洋蔥全數拔除 |
| 10-27 | ryosys 上輪自動化中斷留下 ys-e2e 265/314 紅燈(49 例全 stdout FAIL;regress 基線已寫 314/314=必紅) | 環偵測初版只認六種簡式 cell、其餘全走 fallback 全連通——`$sdff` 的 D→Q 被建邊,next-state 回授(`count<=count+1`)誤判組合環,16 base 案例×3 層報假環+`Warnings:` 彙總行;另 log_signal_bit「offset 0=裸名」規則錯,寬 32 的 `\out3 [0]` 誤印裸名 | ①check.cc:352 參與條件(cell_evaluable||mem/barrier||is_builtin_ff,不滿足不建邊)+celledges.cc 全 dispatch(ff_op 邊緣觸發 ff **不建 D→Q 邊**;arith/shift/mux/compare/mem 精確;detail_costly≥1024 fallback);②log_signal_bit 裸名僅當 wire 寬 1(dump_sigchunk:chunk 寬==wire 寬&&offset 0);cells.rs 增 cell_evaluable/cell_is_builtin_ff(newcelltypes.h 機械轉錄) | SUMMARY 精確數基線 grep(錯即紅)+LIVE 腿雙向釘(修復後 ys=314/314、LIVE ys=314/314);PROVENANCE 補完實錄 |
| 10-27 | regress 裸跑 FAIL(rv-diff smoke×2+agent toolbox;e2e 59/59 與 ys 314/314+live 均綠——紅面恰全在需 C++ 鏈接的腿;RYOSYS-MVP §7.0-4 實錄:修法前 FAIL/DEVELOPER_DIR=CLT 對齊後同命令 PASS) | 默認 `xcode-select` 指向 Xcode 26.6(17F113)、CLT 27.0.0.0 並存:Xcode 26.x 的 ld 不認 CLT 27 SDK .tbd 的新架構 token(arm64e.x1 等),`verilator --binary` 的 make 鏈接段報 `ld: multiple errors: tapi error: malformed file … MacOSX27.0.sdk/usr/lib/libSystem.B.tbd:4:20: unknown architecture`;`--lint-only` 無需鏈接故 lint parity 不受累 | regress.sh 頂部 3 行防禦:`DEVELOPER_DIR` 未設且 `/Library/Developer/CommandLineTools/usr/bin` 在場才 export CLT(顯式設置尊重不覆蓋=用戶主權;無 CLT 機器 `[ -d ]` 短路零影響,RM1-6 未模擬設計保證);驗收兩路徑=裸跑+`env -u DEVELOPER_DIR` 形(RM1-5:顯式設壞鏈=預期 FAIL 非 bug;RM1-7:半損壞 CLT 被 `-d` 判定誤選=紅燈可診斷非假綠,登記在案) | regress.sh 本身(閘)+SKILL ryosys 節工具鏈條目(正典)+AGENTS 10.9-r 工具鏈對齊條(對應節)+本行(證據頁) |
| `vl-difftest --html` | 報告需求 | 生成 docs/e2e-report/index.html |
| `vl-difftest --record` | 語義變更後 | 重錄 oracle 期望(人工讀 diff) |
