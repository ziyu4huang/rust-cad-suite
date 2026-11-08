# rust-cad-suite REGISTRY(組件登記簿 = 本 repo 的 CRM:組件×能力×閘×證據的關係表)

> 單一事實來源原則:此表只放「關係與指針」,不放瞬時值(通過率看報告頁,
> 由 `vl-difftest --html` 重生成)。改動組件職責必須同輪更新本表。

## Crates

| Crate | 職責 | 依賴 | 對應上游角色 | 證據/閘 |
|---|---|---|---|---|
| `rcs-diag` | spans/source map/診斷/caret 渲染(suite 共享) | 0 | verilator 的 %Error 輸出層 | 渲染 golden 測試 |
| `analyzer-core` | 統一分析 API(`analyze`+`Analysis` 投影視圖)+ASP/1 codec+§13 報告編碼(10.7 core-first 語義層) | vl-syntax,ry-ir,rcs-diag | —(自建,無 oracle) | `cargo test -p analyzer-core` 34 pass+1 ignored(json 17+api 13+codec 向量 4;2026-11-03 收平輪實測);`bun test`(同 fixtures 12);ADR-0011 |
| `analyzer-server` | bin `analyzerd`:UDS 面(握手/幀循環/ops/停機/殘留清理),零語義 | analyzer-core | —(自建) | `cargo test -p analyzer-server` 15 測(單測 3+回環 12;2026-11-03 收平輪實測);regress S5 `analyzer smoke` 塊 |
| `rtl-spec` | RTL 行為規則書提取(reset/fsm/handshake/timing/arithmetic/counter/memory/protocol/param 九類)+破口掃描 G1..G7+結構投影摘要+測試計劃 tb_plan 與模板 TB(tbplan.rs,C 弧 C-S2;TbError 十一條硬錯 milestone TB1)+手寫 JSON writer(只寫不讀,鍵序=寫出序 byte-exact)+markdown 渲染+D 弧 spec 前端(spec.rs parse_spec_md/spec_lint 七查/spec_json/spec_lint_json 三面 byte-equal 單一源;cli bin `rtl-spec`=agent 唯一接口,ADR-0014);語義姿態=永不 panic、錯誤進結果;走 analyzer-core 逃逸口,**不擴前弧**(ADR-0012 D1) | analyzer-core,vl-syntax,ry-ir,rcs-diag | —(自建,無 oracle;質量閘=V1..V6 機械判據+獨立評審員) | `cargo test -p rtl-spec` 81 pass(2026-11-08 D-S6 實跑套件行:lib 14[cli 13+1 spec 子命令測]+spec_tests 13+rtlil 2+sv_rules 41+tbplan 11);`bash plugin/rtl2spec/run-spec-cli-parity.sh`(CLI↔wasm byte-equal 17 腿)+`bash plugin/rtl2spec/run-spec-eval.sh`(6 腿對 expected 逐字+mutprobe 3 條全紅+證據頁保鮮;兩閘 regress S7 收編,ADR-0014),含 G8 dead_branch/S8 確定性錨);`bash plugin/rtl2spec/run-rtl-validation.sh`(13 檔 V1..V6 全過,Σrules=325);`bash plugin/rtl2spec/run-golden-regression.sh`(37 檔×2 golden 逐位元組)+`bash plugin/rtl2spec/run-gap-fixtures.sh`(G1..G7 矩陣;兩閘 regress S11 收編)+`bash plugin/rtl2spec/run-realworld-eval.sh`(353 檔分層評測→docs/RTL2SPEC-EVAL.md);正典=docs/RTL2SPEC-PLUGIN.md |
| `rtl-spec-wasm` | wasm face(cdylib):11 導出(JSON 字串進出,analyze/extract/scan/tb_plan×sv|rtlil 面+spec parse/lint[D-S3]+version;C-S2 7→9,D-S3 →11),零 js_sys,`--target web`+顯式位元組 init(ADR-0012 D3) | rtl-spec,wasm-bindgen | —(自建) | `cargo test -p rtl-spec-wasm` 3 測(host 面;D-S3 +spec 導出測);smoke.mjs bun+node 雙運行時各 18 斷言、9 路 wasm↔native byte parity(2026-11-07 C-S2 實測:新增 tb_plan sv 正樣/TB1 負樣/.il 三路);dist 裝載由兩插件閘驗(D8:閘驗出貨物) |
| `vl-syntax` | pp+lexer+parser+AST(V2005);ANSI 端口組解析=新組重置 net/reg/signed/range、僅方向跨組繼承(D-c2 修復,ADR-0013 D-C13;七向量前端單測 frontend.rs) | rcs-diag | VPreproc/V3Parser | lexer 12 + frontend 18 測試(含 d_c2_ansi_group_reset_seven_vectors;2026-11-07 C-S2 實測);E2E 全語料過解析閘 |
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
| `bash plugin/rtl2spec/run-mcp-e2e.sh`(總閘成員;regress S7 起雙守衛塊) | 每次交付 | MCP stdio 回環 19 斷言(=實跑 PASS 行數,iverilog 在場時,§19 C-S2 起;定義點同步=AGENTS 10.10/OKR KR1.11/run-mcp-e2e.sh/mcp-client 頭注):initialize 回顯/tools list 恰 5 工具/extract_rules 正樣(rules≥3)/scan_gaps Latch 正樣(@* 形)/負樣三形(檔案缺失 isError+path,source 雙缺 isError+未知名 -32601)/⑧ D-r1a max_rules 效果四調用(i2c 不傳/顯式 0→152 零截斷、顯式 100→100+truncated、batch→152)/⑨ rtl_tb_plan 正樣(ok+clocks/resets≥1+協議標記+模板與 fixture `iverilog -g2005 -t null` 同編零警告,iverilog 不在場 SKIP)/⑩ parametric→TB1/⑪ .il→TB1;wasm 陳舊檢查含全部傳遞依賴目錄 | 
| `bash plugin/rtl2spec/run-rtl-validation.sh`(總閘成員;regress S7 起雙守衛塊) | 每次交付 | 13 檔 in-git 語料 V1..V6 機械判據全過(反 doc-dump 三鎖閘端鎖;snippet 逐位元組對拍=checker 重讀源檔);validation-report.md 機械生成入庫=證據頁 |
| `bash plugin/rtl2spec/run-golden-regression.sh`(總閘成員;regress S11 起雙守衛塊,S9 實作) | 每次交付 | 釘選 49 檔×rules+gaps=98 份 golden 逐位元組比對(收尾弧 S3 起 37→49:+12 個 conformance staging ADMITTED 設計;釘選集 SSOT=RTL2SPEC-PLUGIN §20.5 全表(S4 清償 §16.1.1 欠條),run-plugin-check ⑤e 逐檔名對拍、⑤f 欠條已清;golden-check `EXPECTED_PINNED=49` 精確數斷言,清單增刪未同輪上調=exit 2)(wasm 腿=dist 同一產物,D8);--record 顯式重錄(錄前進程內每檔每模式 3 跑自檢,兩階段防部分錄製)+PROVENANCE.md 機械重寫;re-record=語義事件,錄後 git diff golden/ 逐案對帳;紅路實證=golden 篡一字節必紅(DIFF+PROV-DIFF 雙層)+提取器 message 改一字必紅(S9 輪在案);§19 C-S1 起尾部打印 `STATS …` 資訊行(scripts/golden-stats.mjs,不斷言;文檔對賬數字一律引此行)+golden 重錄片標準程序 G0..G5(G3 對賬工具 scripts/golden-leafdiff.mjs 葉級 diff+--allow 越界斷言) |
| `bash plugin/rtl2spec/run-gap-fixtures.sh`(總閘成員;regress S11 起雙守衛塊,S10 實作) | 每次交付 | G1..G7 正負樣**條級 (module,class,confidence) multiset 集合相等**(正樣 14 條含 G6/G7 共報全集+G4 無復位臂新檔 suggested_test 逐字名 ra)+msgSubs 關鍵詞抽點+終止性守衛(gap_hang_repro.v 12 跑逐字不變+G5 恰 4 條)+selfcheck 判別力自證(正樣誤配負樣必紅);expected findings=斷言規格手寫合法,與 golden 快照劃界(D15);§19 C-S1 起 +D-r1c 五檔(rst_first_async/twin/compound/loadsig/rst_only_edge)破口面檔級+規則面 guard 兩矩陣各一行+順序不變性+兩 selfcheck(修前 wasm 紅路 4 FAIL 在案),檔級矩陣 13 行 |
| `bash plugin/rtl2spec/run-conformance-eval.sh`(C 閉環弧 C-S3 交付;總閘直調**不進 regress**,評測層分鐘級;條文四層=§19 C-S6) | 每次交付(直調) | 六設計 T 腿 Q1..Q9(編譯零警告/原版 2 跑==expected.txt/DONE 收尾零 WATCHDOG/X-clean/協議文法/靜態約束含 TB-R1x/verilator 跨模擬器/判別力鐵則 P-rule≥1 killed+控制組存活+record 雙向 DRIFT/覆蓋聲明)+selfcheck(分腿正規化敵意向量、X 掃描、同檔 R1 三向量 parity、TB-R1x 跨檔五向量 fixtures/conformance-lint/、手術 count==1)+證據頁 conformance-report.md byte 比對;`--record --slice <C-Sn>`=錄前快照+record-diff+yosys 僅錄製期存活裁決;首錄(C-S3)SUMMARY designs=6/6 admitted=6 mutants=439 eligible=412 killed=210 equivalent=27 unruled_killed=13;C-S4 起 4 設計 tb_leg=directed(凍結 LLM 產物,協議=CONFORMANCE-PROTOCOL.md;T 腿模板轉基線腿同矩陣,D/T 分列禁合併;入庫門=形族底線+不劣於基線;C-S4 SUMMARY killed=347 gain +137:i2c 201/255、fsm 42/42、sincos 76/82、acc_pipe 10/10;收尾弧 S3 起 T 腿擴面到 34 設計(staging `--fixtures-root` 預錄 51 候選→ADMITTED 28 收編,REJECTED 23 登記於 CONFORMANCE-PROTOCOL §9;主根斷言 specPath∈golden 釘選集;C-S8-2 SUMMARY designs=34/34 admitted=34 mutants=863 eligible=794 killed=544 equivalent=69 unruled_killed=84;SUMMARY-EQ adjudicated=311 equivalent=69 unproven=242 adjudication_failed=0 killed_selfcheck total=552 equivalent=0;默認模式 3:08;收尾弧 S4 起默認模式另斷言入庫 record 的 provenance.wasmSha256==dist wasm,不等=DRIFT);C-S5 起診斷層入閘(record 新增 rulebookDefects.defLint+leaveOneOut 欄=DRIFT 比對;F3 閘斷言 i2c 缺陷必含 cnt_end;rulebook-defects.{json,md} 全集跑生成;LOO rate 實測 acc 3/10 fsm 6/42 i2c 8/201 se 7/10 sincos 9/76 subbytes 3/8,v1 無下限);sincos 76/82、acc_pipe 10/10);紅路四路(expected 篡一字節→Q2/killed 錄改 survived→Q8 DRIFT/刪 SCENE 頭→Q5/手改 tb.v 含註釋→TB-SHA-ALARM)在案;收尾弧 S0 起 TB-SHA-ALARM=record.tb.sha256/磁碟 tb.v/本輪主腿 TB 三方核對(默認 REJECTED、錄製拒錄,exit 1;換檔唯一路徑 --promote,CONFORMANCE-PROTOCOL §5)+WAIVE 標籤集 tblint WAIVE_LABELS(補 unadjudicated,集外 Q9 硬紅)+協議模板硬同步(proto-template.mjs,v2 104d13f1…;selfcheck 加枚舉逐集合核對與篡改負向量);收尾弧 S1 起等價裁決腿(協議 §7;--record 專屬,默認模式零 yosys 依賴):強形命令+輸出位數空真守衛+eqverdict 三值+gold-vs-gold/空真探針/地面真值/killed 全母體/存量重驗任一不過=拒錄+WAIVE↔record 併入 Q9;默認模式 eq-verify(schema+counts+waiveTable);C-S7 重錄 SUMMARY-EQ adjudicated=84 equivalent=27 unproven=57 adjudication_failed=0 killed_selfcheck total=355 main=347 unproven=354 failed=1 equivalent=0,主 SUMMARY 逐值不變,expected.txt 0 行差;度量行為復原力,非意圖正確性 |
| `bash plugin/rtl2spec/run-frame-diag.sh`(收尾弧 S2;**輔助腿,非閘**,不入總閘;`--validate` 驗診斷工件) | 需要波形診斷時直調 | counterexample VCD(wavecap 第二 top,tb.v 零改,帶探針輸出==expected.txt 硬斷言)→ vl-vcd vcd2svg 統一窗 SVG → sips/qlmanage PNG(查看手段,SVG 仍是交付形)→ 剝 title 畫布+PNG 雙檢定 modality → 視覺 LLM 讀幀或機械 text-fallback;寫面=build/conformance/{frames,diag-vision}/;條文=CONFORMANCE-PROTOCOL §8;2026-11-07 六設計雙檢:fsm/sincos/acc_pipe/i2c_master 可 vision,subbytes/signedexpr 強制 text-fallback |
| `bash plugin/rtl2spec/run-realworld-eval.sh`(總閘成員直調,**不進 regress**;S12 實作) | 每次交付 | 353 檔分層評測(L1=93 全提取+破口/L2=sv-tests 260 容錯面);閘級硬 FAIL=trap/V4/V5-①②④/V5-③ 指紋重複/V6 覆蓋;報告=docs/RTL2SPEC-EVAL.md 機械生成+自洽斷言(判定與 failures 同源,gen-report R3-③ 先例);首錄實測=37 design-rules/117 sparse/193 skip-perr/6 skip-tb/trap=0/指紋 672 unique/80ms |

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
| 11-03 | rtl-spec G5 掃描 scopes.v(63 行小檔)雙面掛死:native emit 4s+ 未終止(sample 棧=bfs_path→HashMap::get)、wasm 腿秒級 `RuntimeError: Unreachable` trap;開弧規劃誤判為「圖規模爆炸」 | **bfs_path 死循環真 bug**:防重用 prev.contains_key 但 start 不在 prev——組合圖含自環邊(adj[start] 含 start,scopes.v 函數/命名塊遮蔽產生自賦值)時 prev[start]=start 自映射,路徑重建 `while let Some(&p)=prev.get(cur)` 沿自映射永不停;實測圖僅 nodes=2/edges=7(規模假說證偽) | bfs_path 加 visited 集(起點入 visited=永無 prev 映射,重建 `while cur!=start` 必終止);wasm32 腿的 panic(unreachable)=死循環在 wasm 的表現形,兩腿同根因;gap_hang_repro.v 最小復現形入 plugin/rtl2spec/fixtures,單測 gap_comb_loop_self_edges_terminate_and_report 釘死(自環×2+雙向環×2 精確+3 跑 byte-identical) | rtl-spec S8(gaps.rs bfs 修復+單測)+probe-determinism.mjs(37 檔×12 跑+調用序擾動,flapper 12 檔清零)+§16.0 F2 記錄勘正(真根因=死循環非規模,守衛保留防未來真大圖) |
| 11-07 | 終審 R5-1/R5-3:fixtures/conformance/ 內滯留 4 個未登記 LLM 盲寫 TB 殘留目錄
  (目錄名=描述串含 `/` 被建為嵌套路徑;各僅 tb.v 222/377/707/270 行;無 case.json 不入閘枚舉;
  mtime 09:07-09:28=C-S6 窗口;與已登記腿產物均不匹配=無來源樣本)滯留 10.12 授權入庫子樹 | 
  寫面衛生:某輪腳本以描述串當路徑建目錄且未經 try/record 對賬流程;寫面聲明審計缺
  「fixtures/ 樹逐輪 find 差分」步 | 刪除 4 目錄(證據已固定於終審報告與 /tmp/r5-evidence/*.tgz;
  修後 find 零殘留);閘不受影響實證=run-conformance-eval PASS 6/6(殘留無 case.json 不進枚舉) |
  AGENTS 10.12 寫面聲明+§19.24 補記;終審報告為源記錄 |
| 11-07 | 終審 medium(收尾弧 S0):入庫 directed 腿 tb.v 手改後閘仍全綠(實錄:fsm/tb.v 追加一行註釋,磁碟 sha 1871b0…≠record 6792d5…,`run-conformance-eval.sh fsm` exit 0) | 默認模式只比模擬輸出與變異矩陣,對註釋無感;record.tb.sha256 只寫不驗。同族洞:WAIVE 標籤錯字在該規則另有 COVERS 時被舊正則靜默略過 | TB-SHA-ALARM 三方核對(record.tb.sha256/磁碟 tb.v/本輪主腿 TB;默認 REJECTED、錄製拒錄、寫後核對,皆 exit 1;--promote 不洗白)+tblint WAIVE 行文法外 Q9 硬紅 | conformance-eval selfcheck(TB-SHA 12 向量、WAIVE 一正一負)+端到端驗收實跑(手改紅/還原綠/promote 不誤報)+AGENTS 10.12+CONFORMANCE-PROTOCOL §5 |
| 11-07 | 收尾弧 S3:conformance 主根新增的「specPath 必在 golden 釘選集」斷言是空操作——負向探針把 always01 的 specPath 換成 staging 快照、或把它移出釘選清單,默認模式仍 ADMITTED exit 0 | 斷言只 `reasons.push`,沒有進 `admitted` 判定(admitted 只看 Q1..Q9/deterministic/tLegCheck/dGate/TB-SHA/等價失敗/eq-verify 旗標) | 新增 `specPinned` 旗標並併入 admitted;同輪逐條核對其餘 reasons.push 皆有對應旗標或提前返回 | finish-s3 驗收 P1/P2 實跑必紅(REJECTED exit 1)+AGENTS 10.12「判定旗標必須進 admitted,新斷言配負向探針」 |
| `vl-difftest --html` | 報告需求 | 生成 docs/e2e-report/index.html |
| `vl-difftest --record` | 語義變更後 | 重錄 oracle 期望(人工讀 diff) |

## 復原力弧增補(2026-11-05;S0..S5;PLUGIN.md §18 為記錄 SSOT)

- **新閘行**:run-reimpl-eval.sh(復原對拍評測;iverilog 守衛缺場 exit 2;
  23 場景 6+6+11;A/B 兩組分列禁混報;寫只落 build/reimpl-eval/;直調不進
  regress——盲寫腿=輔助證據永不進確定性閘,機械面=編譯成敗+TB 逐場景 diff;
  裝備=fixtures/e1+fixtures/e1-reimpl,協議=REIMPL-PROTOCOL.md)。C-S3(2026-11-07)起場景切塊與子進程執行遷用
  scripts/lib/tbproto.mjs(帶看門狗);verdict.json 結構/matched/exit 凍結,同時刻 parity=legs/segments 深等+runs/out_*
  14 檔 byte 相等;scenesParsed 只落 sidecar build/reimpl-eval/scene-protocol.json。
- **M7/M8 編號命名空間注記**:MVP-NEXT R2 的 M1..M8=復原力弧提案材料編號
  (M1 多標簽臂/M2 default 臂/M3 else 包膜/M4 G8 容器/M5 missing_reset/
  M6 incomplete_case/M7=PROVENANCE 手記抹除/M8=殘文候選十行),與 AGENTS
  10.8 的 M8(多 limb 表示變更弧)同名異軌——引用時須帶弧前綴,禁裸引。
- **基線上調處(S0..S5 累計)**:DEFAULT_MAX_RULES 128→256(schema.rs SSOT
  +golden-check/check-rulebook/eval-all/probe-determinism 四處調用點);
  插件 semver 0.2.0→0.3.0(plugin.json/CHANGELOG/SERVER_VERSION 三點;
  內核 ABI 軌 workspace 0.1.0 獨立);V1-total 506(13 檔)/eval 指紋 1212
  dup=0;parse_err 殘文 192→193(PLUGIN.md:1400/:1508+eval-all.mjs:7)。
- **Crates 表**:rtl-spec 行測數增至 sv 35 測(含 S1-S4 條級 fixture 斷言);
  新增 lint.rs(N5 詞法/集合差,3 單測)。

### 復原力弧終審已知未決(2026-11-05 終審發現,2026-11-06 提交前登記;10.5 先登記再繼續;修復歸 C 閉環弧前置切片)

- **D-r1a max_rules 上調未達 MCP/batch 面**:DEFAULT_MAX_RULES 256 的調用點
  實況=6 處已達(golden-check.mjs:80/check-rulebook.mjs:39/eval-all.mjs:140/
  probe-determinism.mjs:81,85/reimpl-eval.mjs:37),3 處仍 128——
  plugin/rtl2spec/dist/server.mjs:150(MCP rtl_extract_rules 默認)與 :219
  (batch 掃描硬編碼,用戶不可繞)、scripts/gap-fixture-check.mjs:215(本弧新寫
  漏改)。後果=i2c_master(152 條)經 MCP 默認/batch 仍截 152→128;上行
  「四處調用點」計數以此實況為準訂正。修法=server.mjs 兩處與
  gap-fixture-check 對齊 SSOT(面隨核心對齊,非語義新增)。
  - **關閉(2026-11-07,§19 C-S1;工作樹未提交)**:普查口徑=`rg -n '\b(128|256)\b'`
    插件面(dist/server.mjs+scripts+skills)+native emit.rs+wasm crate——修前插件面 128 實為
    5 處(server.mjs:139 描述/:150 回退/:219 batch、gap-fixture-check.mjs:215、
    rtl-rulebook SKILL.md:15 文案;上行「3 處」漏列 :139 與 SKILL)+native 面
    emit.rs:10/:16 兩處(§19 RV2-10);修後插件面 128=0 處。修法=server.mjs 回退與 batch
    傳 0(wasm 契約 max_rules==0 取核心默認,面委託核心不複製常量)、:139/SKILL 文案
    改「省略或 0=核心默認 256」、gap-fixture-check→256、emit.rs 改引
    rtl_spec::DEFAULT_MAX_RULES;非面(rtl-spec-wasm lib.rs:98/110/111 單測、smoke.mjs:53/:80
    對稱 parity)按 §19.2 不改。機械防回歸=run-plugin-check ⑤(新 scripts/ssot-drift-check.mjs:
    (a) 字面量∈{0,N} 8 處/(b) 文案數字==N/(c) 回退 token 錨/(d) emit.rs 常量名+分派段無數字缺省;
    紅路 5 路各 exit 1、原樣 exit 0 在案)+mcp-client ⑧ 四調用(i2c 不傳/顯式 0→152 零截斷、
    顯式 100→100+truncated=[i2c_master]、batch→152;HEAD server.mjs 副本紅路 ⑧a/⑧b/⑧d FAIL
    =128+truncated 現形在案)。驗收命令實跑:run-plugin-check PASS、run-mcp-e2e PASS(14 斷言)、
    run-gap-fixtures PASS(既有 24 行輸出與 HEAD 腳本[:215=128]逐行相等)。
- **D-r1b PLUGIN §18.2 S3 對賬數字不可復算**:文檔稱 guard+async=12/
  敏感列表證=29/seq+when=296/去重復活=175;2026-11-05 終審+主會話實測
  golden 全樹=async:true 59、guard 帶敏感列表第二證 85、sequence+when 476
  ——無口徑能對上,疑為 delta/子集口徑未注明(計數一律實測條違例)。
  修法=C 弧內按實測口徑重算訂正或注明統計口徑。
  - **關閉(2026-11-07,§19 C-S1)**:勘誤表=RTL2SPEC-PLUGIN §19.2 D-r1b 表(§18.2 原文按
    歷史快照保留不改,讀者以該表為準)。防復發=新 scripts/golden-stats.mjs(唯讀單行
    `STATS …`,run-golden-regression.sh 尾部資訊行,不斷言);D-r1c 修前實跑(HEAD golden 與
    G0 副本)=`STATS rules=843 seq=539 rel=196 guard=85 trans=23 async_true=59 guard_2ev=85
    seq_when=476 truncated=0`,與勘誤表 HEAD 列逐值相等;D-r1c 重錄後位移見 §19 C-S1 實作記錄
    (勘誤表不回改)。此後文檔對賬數字一律引 STATS 行原文。
- **D-r1c mem_arst 復位前置敏感列表 async 分類洞**:`always @(negedge
  Reset_n_i or posedge Clk_i)`(首沿=復位信號)形 async 分類整類失明——
  sv/mod.rs:752 `trigger = edges.iter().find_map` 取敏感列表首個沿,而復位臂
  判定(:790-808)要求 trigger.signal≠reset_sig;golden/yosys-simple/
  mem_arst.rules.json 實測零 async 欄位(4 條全 timing/sequence,異步復位被
  表述為 latency-1)。HEAD 既有缺類非本弧回歸(舊版同形);普查全語料受害面
  =僅 mem_arst 1 檔(其餘 11 檔雙沿形 clk 前置分類正確)。修法=trigger 改取
  「非復位信號的邊沿」+run-gap-fixtures 補 mem_arst 形正負 fixture+同輪
  golden 重錄對賬。
  - **關閉(2026-11-07,§19 C-S1;兩面同片同改)**:共用函數 `select_trigger(edges,
    reset_sig)`(crates/rtl-spec/src/sv/mod.rs,緊鄰 reset_cond_signal)=單一語義源,
    規則面 extract_edge_process 與破口面 gaps.rs 復位臂識別皆只調它(修前破口面註釋
    自述「與規則提取面同一判準」實為各寫一份,正是洞根);語義=首個訊號 ≠復位候選的
    pos/neg 沿、無則回退首沿(既有形零漂移);訊號比較鍵兩面統一=render(邊沿表達式)
    (破口面舊用 lvalue_base,裸標識符恆同;in-git 語料 rg 非標識符邊沿表達式 0 例)。
    受害面普查訂正【實跑 rg 釘選 37 檔】:雙沿 always=11 檔 18 行,復位前置**僅
    mem_arst.v:20**,其餘 10 檔時鐘前置(上行「其餘 11 檔」多計 1;fsm.v:16 位於
    未定義 `ifdef ASYNC_RESET 分支,文本命中不入解析)。紅綠路實證:2 單測
    (c_s1_d_r1c_reset_first_sensitivity_is_async_guard/c_s1_d_r1c_sensitivity_order_
    invariance)對 HEAD 工作樹副本跑=2/2 FAIL、對修後碼=2/2 PASS;gap-fixture-check
    兩矩陣各+5 行(規則面 guard multiset+破口面檔級,期望以 dist wasm 修前實探定案
    build/c-s1/probe-pre.txt)+兩 selfcheck+順序不變性斷言,對修前 wasm 紅路 4 FAIL、
    修後 41 PASS;golden G0..G5 重錄(允許清單恰 mem_arst.rules+gaps 兩檔:R001/R002
    sequence→guard×2 async=true trigger=Clk_i[mem_arst 非 TB 樣,'bx 值如實保留]、
    G4-MyMem-001 消失、G3 不變,leafdiff 40 葉在檔 build/golden-leafdiff-C-S1.txt)。
    **RTLIL 面 rtlil.rs:197-209=潛在同形第三處,本弧不改**(兩序 .il 樣皆 0 規則無現形;
    改動無判別力 fixture 違鐵則)——留待有現形樣的弧配正負 fixture 修。

### C 弧已知未決(2026-11-07 §19 C-S1 首錄;10.5 先登記再繼續;企劃=RTL2SPEC-PLUGIN §19,決策=ADR-0013)

- **D-c1 net 宣告賦值未提取(i2c `cnt_end`)**:`wire cnt_end = (cnt >= prescale - 1);`
  (corpus/e2e/59_i2c_master/i2c_master.v:68)落入 `extract_module`
  (crates/rtl-spec/src/sv/mod.rs:641)的 `_ => {}` 臂(:686;只認 Always/Initial/
  ContAssign)——規則書對該 net 零規則;復原力弧 post-S4 i2c 重寫腿 e1 TB 全 11/11 而
  yosys equiv 留 `cnt_end` unproven 的根因(§19.0-3,【材料】)。狀態=未修;修復立項時點
  =用戶裁決(§19.13-6)。**證據已補強(2026-11-07 C-S5)**:conformance 閘 F3 斷言
  (i2c_master 缺陷必含 cnt_end)PASS;build/conformance/rulebook-defects.{json,md} 收 P-unruled killed
  兩變異(OP-HOLD/OP-ARITH@:68,D 腿 TB 六場景全殺)+def_lint-U1 分類(net 宣告賦值洞,
  extract_module `_ => {}` 臂同形)。
- **D-c2 ANSI 端口宣告屬性跨組繼承**:crates/vl-syntax/src/parser/module.rs:132-171 的
  ANSI 端口組解析把 cur_net/cur_reg/cur_signed/cur_range 綁在組循環外,新組(任一
  宣告限定詞開頭,parser/mod.rs:449 peek_starts_port_spec)不重置——範圍/符號/種類
  三族屬性跨組繼承;oracle=IEEE 1800-2017 §23.2.2.3 同向,iverilog 13.0(-g2005/-g2012)
  與 verilator 5.052 七向量三方一致(§19.3.2 表,規劃輪實跑)。現形【本輪 dist wasm 實探
  f689132b…】:fixtures/fsm_counter.v contract `busy` 報寬 4(源碼 `output reg busy`
  1 位,繼承前組 `[3:0]`);partsel_test006/007 `dout` 報 signed=true(繼承前組
  `input signed [4:0] n`)。定量普查(規劃輪 /tmp 補丁實驗,【材料】)=釘選 37×2 same 70
  diff 4=24 葉(loops 4/mem_arst 2/memory 2/partsel 16)+L1 非釘選 retime 1 葉;
  analyzer-core tests/api.rs:128 把缺陷語義釘成正確(ADR-0013 D-C13 修訂 ADR-0012 D1)。
  狀態=未修;修復=§19 C-S2 首步(步 a,G0..G5)。
  - **關閉(2026-11-07,§19 C-S2 步 a;工作樹未提交)**:crates/vl-syntax/src/parser/module.rs 組循環頂
    重新綁定 cur_net/cur_reg/cur_signed/cur_range(新組四變量重置,僅 cur_dir 跨組);七向量前端單測
    d_c2_ansi_group_reset_seven_vectors(crates/vl-syntax/tests/frontend.rs,斷言到 analyzer-core 投影;
    vl-syntax 新增 analyzer-core dev-dependency=僅測試面的 dev 環)對 HEAD 解析器副本 FAILED(v1 d=4)、
    修後 PASS;api.rs:128 期望 4→1 與 :119-121/:58、module.rs、ast.rs Port doc、sv.rs:132-134 註釋同片訂正
    (ADR-0013 D-C13,D1 唯一例外;API 簽名零改)。golden G0..G5:G2 DIFF 恰 {loops,memory,mem_arst,
    partsel}.rules.json、PROV-DIFF 0;G3 leafdiff=24 規則書葉(loops 4/memory 2/mem_arst 2[ports/5 WR_i
    width 0→1+parametric true→false]/partsel 16[全 signed true→false])+PROVENANCE 13 行,對賬工件
    build/golden-leafdiff-C-S2.txt;gaps 37 檔零變;G4 74/74;STATS 零位移(contract 面不入規則計數)。
    L1 非釘選 retime 1 葉與 corpus/sv-tests 零差=規劃輪材料,本片未單獨複核(EVAL 指紋 1212/dup=0 不變)。
- **D-c3 corpus_lint R1 同行 `#` 不關窗 — 已清償(D-S0,2026-11-08;D-S6 終核)**:crates/vl-difftest/tests/corpus_lint.rs
  的 `closed` 空操作死代碼(原 :80-95,空循環/自賦值/空 if)整段刪除;同行 `#`/`@(` 關窗=
  `window_open = cut == tail.len()`(crates/vl-difftest/tests/corpus_lint.rs:85),跨行擴展循環
  包進 `if window_open`(:88)——同行 `#` 前語句仍在窗內,既有 bad 形 `@(posedge clk); ld = 1;`
  仍 1 finding。雙 fixture 判別力(10.12 鐵律):正樣 race_lint_same_line_delay_closes_window_for_following_lines
  (`@(posedge clk); #1; ld = 1;` 後仍有 `q = 3;` 行→零違例;修前此形 1 假陽性)、
  負樣 race_lint_in_window_race_still_red(同行寫/後續行窗內寫各恰 1)。負向探針實跑=臨時還原
  無條件擴展→正樣測試 FAILED(exit 101,finding=`line 5: blocking write to `q``)後還原。
  閘=`cargo test -p vl-difftest --test corpus_lint` 5 測試全綠(既有三測試零 regress;corpus
  現狀零違例零擾動)。效果注記:Rust 側換行形自本修復起=0,與 C 弧 TB-R1x 文檔語義(第五向量 0)
  一致;tblint.mjs:10「換行形=已登記差異 D-c3」注記自此陳舊(JS selfcheck 判 JS 側值不受影響),
  正典注記 D-S6 統一清償。
- **D-c4 contract 負數下標定寬範圍報 parametric**:partsel_test006/007 的 `[31:-32]`
  (corpus-external/yosys-simple/partsel.v:92/:94/:104)在 contract 報 `width 0+
  parametric:true`(實為 64 位定寬)【本輪 dist wasm 實探】;tb_plan 對其以 TB1 拒收=
  安全側(§19.3.2)。狀態=未修;修正時點=用戶裁決(§19.13-9)。
- **D-c5 等價裁決的 X 語義盲區(收尾弧 S3 實錄)**:constmuldivmod 的 3 個變異(m0003 `A % 8'd0`→`A % 8'd1`,m0033/m0064 同形)
  TB 殺得掉,yosys 強形卻證 equivalent——除零在 iverilog 是 x,yosys 把未定義當任意值,1 是合法取值。killed 全母體自檢當場報紅
  (equivalent=3),該設計另因 Q4/Q7 被拒,未入庫。含義:proven≠無條件行為等價有了實例;含 x 行為的設計若入場,等價腿需先區分
  X-refinement 類 proven。狀態=未修,交等價裁決腿 eqverdict 分類面(RTL2SPEC-FINISH-PLAN §11)。
- **D-c6 空真探針的 memory 盲區(收尾弧 S3 實錄)**:探針全名單取自 `proc; flatten; write_json`(未跑 memory),記憶體映射後的
  FF 名不在名單,arrays01 探針留下 64 個 unproven 而非 `No $equiv cells`,偵測力斷言因此紅。方向保守(拒設計,不假放行);
  arrays01 另因 Q4/Q7 被拒。狀態=未修;memory 類設計入場前修探針名單(探針模板=CONFORMANCE-PROTOCOL §7,改動=協議事件)。
- **D-c7 行內賦值延遲抽象等價=已入庫設計的 TB 強化死鎖(收尾弧終審實錄,主會話登記)**:yosys 視行內賦值 `#delay`
  為不可觀測抽象,oracle(iverilog)有可觀測差異——i2c_master_tests 的 5 個 OP-CONST 變異(m0002/m0004/m0006/m0008/m0010,
  record.json eqverdict=equivalent eqCells=2[主會話已核])被等價腿判 equivalent;終審員以 iverilog 13.0 最小 TB 證可觀測
  (gold t=17000 al=1 vs mut al=0;/tmp 探針已清,未主會話復現,標 unconfirmed),並實錄「強化 TB 經 --record --promote
  提交→killed 全母體自檢紅(REJECTED exit 1)」——與 eqverdict 恆矛盾的變異永不可殺,**已入庫設計的 TB 永久不可強化**
  (殺傷率表面 3/5、實際可殺 3/10)。與 §20.8 operators `$pow` 待裁是同一政策問題(能力邊界時等價腿是否不適用)且多出
  「入庫後凍結」維度。狀態=未裁,方向保守(不假放行);裁決前該類設計維持現狀,新強化 TB 逐案對賬。
### D 弧已知未決(2026-11-08 D-S6 終核登記;10.5 先登記)

- **spec invariant 形文法未立**:parse 遇 `check(invariant)`=finding ⑤(form ∉
  四收形),文法待有語料再立(立時=ADR-0014 修訂事件)。
- **「## behavior ≥1」未入 spec_lint 七查枚舉**:零條目 spec 可過 lint
  (①..⑦ 無此項);是否收查=ADR-0014 開放項待裁。
- **spec-eval 證據頁命令行=canonical 模式無關形**(D-S6 裁,沿 conformance
  同款);--record 與默認模式產 byte 相同頁面。
- **i2c mutprobe 不可觀測錨教訓**(D-S4 實錄):oe=0 時 o 值翻轉不影響總線
  電平=TB 不可觀測(11/11 未紅),換 scl_oe 使能翻轉後 7/11 紅——「變異落點
  可觀測性」先於「變異語義」。
