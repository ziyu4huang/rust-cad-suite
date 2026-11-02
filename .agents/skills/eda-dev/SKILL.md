---
name: eda-dev
description: rust-cad-suite 开发正典——AI agent 做 EDA/IC 工具(模拟器/综合器类)的行为规则与流程
---

# EDA tool development skill(rust-cad-suite 正典)

适用:在本 repo 重写/扩展 oss-cad-suite 等价物(模拟器、综合器、PnR)。
配套铁则=repo 根 `AGENTS.md`;路线=PLAN.md;组件关系=docs/REGISTRY.md;
目标与证据=docs/OKR.md;本 skill 是 3-tiers+1 中的「正典层」。

## 铁则(执行顺序即优先级)

1. **Oracle 即 spec**。兼容产品的正确性=参考工具可观测行为。语义分歧时
   oracle 赢,即使 LRM 站你这边。期望输出必须 `vl-difftest --record` 录制,
   禁手写(实录:7 例手写期望全败,见 docs/REFLECTION-agent-hardening.md §1)。
2. **双验收轨**:byte-diff corpus 必须 X-clean 且无同边沿竞争(x/z 传播与
   IEEE 非确定序不可与 2-state oracle 对拍);4-state 算子正确性走 IEEE §5
   真值表单测轨。竞争 TB 改中段写(范式=corpus/e2e/26、43)。
3. **不支持=带里程碑硬错**:`error ... (milestone Mx)`,exit≠0。禁静默跑偏。
4. **恢复必保前进**:每个解析器/执行器恢复循环带「pos 未动→bump」守卫,
   并配一个会触发的挂死 fixture(实录:task ANSI integer 端口死循环)。
5. **算子与真值表同生**:新 4-state 算子同轮写 x/z kind 保全单测;
   格式化(%d/%b/%t)是契约,golden 钉死。
6. **未解析即硬错**:名字/模块/函数解析未命中=报错带 span;禁盲字符串前缀。

7. **归一化=契约**:只用精确前缀过滤 oracle 输出(`- `/`-Info`/`-Warning`/
   `-Error`),裸 `-` 是设计输出;录制带 `--trace`;re-record 后逐 case 读
   `git diff --stat corpus/e2e`。
8. **竞争免疫模式**:`@(posedge clk); #1; <写>;`——corpus_lint R1 机械执法,
   `// race-ok` 显式豁免。
9. **robustness 轨独立**:fuzz_smoke 常驻;pass-rate 绿≠健壮。
10. **计划项必带 LRM 条号**:条文已说明实现精确时,工作项作废(§5.1.4 实录)。
11. **分歧/事故先登记**:docs/REGISTRY.md 的 Divergence / Incident 两簿。
12. **三重锚,各锚各的**:oracle 锚「行为一致」、golden model 锚「宣称意图」
   (ADR-0007 spec 层:lockstep+invariant)、invariant 锚「协议性质」且
   **指名道姓**(负向变种的断言必须点名对应 invariant,整批抓到≠归因)。
   实证(2026-10-24):I2C RTL 的 ANSI 埠範圍繼承讓 we/sda_i 合法變寬——
   oracle 錄製照樣一致,golden 按 1-bit 建模才暴露;**oracle 一致≠符合自身
   spec**。取樣來源選「性質所屬方」的輸出(master 側性質取 master 驅動輸出,
   不取被 slave 污染的匯流排);取樣點鎖「準位整窗恆定」的相位(SCL 高半),
   低半取樣跨邊界(REFLECTION §16)。

13. **公共型別表示變更,欄位存取面普查先行**:`rg -c '\\.field'` 全 workspace
    普查;讀取面寬=struct 殼+私有 repr enum(欄位讀取零改動),禁裸 enum
    (實錄:enum Bits 炸 30+ `.width` 讀取點並與 AST `NumLit.width` 撞名)。
    結果一律 canonicalize 回快臂,熱路徑不見 heap 形(REFLECTION §17.2)。
14. **Limb/大數契約四律**:①符號中間值按 width 遮罩(padding 垃圾跨輪複合)
    ②補數表達式以 care 遮罩(遮罩輸入會被 `!d` 展開回去)③同號比較不反轉
    (二補數 pattern 序=值序;帶號單測必蓋符號四象限)④大數常數 limb-wise
    構造(`vec![1u64; n]`=[1,1,1,1] 不是 1)。機檢=value.rs 的 N-vs-W
    property 測試(全運算×寬度掃描),新 limb 代碼必過(REFLECTION §17.3)。
15. **CLI/語料契約實測枚舉**:行為類只認實跑輸出(exit codes/錯誤文案逐字),
16. **帳務手術須 assert 命中**:以 python/腳本改 PLAN/OKR/REGISTRY 等帳務文檔時,replace 前必須 assert 錨點在檔中,靜默失敗=帳務宣稱與實際不符(commit message 說了但檔沒改)。實錄:2026-10-25 五輪 PLAN.md 帳務積欠皆此因。
    清單類只認上游文檔,兩者皆無=釘「未知=硬錯」不發明清單;expected.* 禁
    機器相關字串,錄製 cwd=案例目錄(契約寫進 harness);能力未到的期望先錄
    =chase-ledger(pending 錨「里程碑硬錯」契約,翻 enforced=同輪 commit,
    無 ledger 編輯的提前成功=DRIFT 紅燈)(REFLECTION §17.4-17.6)。

## 标准循环(语义开发)

```
1. 写/改 case(corpus/e2e/,X-clean、race-free、oracle 可终止)
2. cargo run -p vl-difftest -- --record        # oracle 录期望
3. cargo run -p vl-difftest -- --only <case>   # 我方对拍
4. 失败→最小复现(/tmp 可弃,关键复现进 corpus)→修实现→cargo test
5. 全绿:bash scripts/regress.sh → 提交(精确 add)
```

## 定位工具箱

- 挂死:`sample <pid> 1`(macOS)看栈;`vl-difftest --only <case>` 二分。
- 语义分歧:最小 .v 并排跑 `rverilator --binary` vs
  `verilator --binary --timing`;oracle 输出即答案。
- /tmp 会被外部清:关键复现移入 repo。

## 3-tiers+1(改规则时执行)

任何新铁则必须同轮落四层,缺一不算数:
①机械闸(scripts/regress.sh:E2E 差分精确数 100%/lint parity 基线/wasm 差分+
  功能+vcd_parse/demo serve smoke+corpus 白名单/ffi 三闸含 vcd_read+
  riv-cli goldens/riv-difftest chase-ledger——或 vl-difftest 内断言)
②本 skill 的正典条文
③AGENTS.md 对应节
④证据页/报告(docs/e2e-report/)可核。

## rVPI 增補(2026-10-23,ADR-0003)

rVPI(`crates/vl-sim/src/vpi.rs`,Rust 原生 VPI 結構等價)新能力必須四證同落:
①vpi_gate 單測(firing-point 斷言)②two-paths-vs-corpus(VPI 取樣==oracle 錄製行)
③rVPI×VCD 內部交叉 ④REGISTRY 分歧行。C-VPI 語義差異先判「語義 vs 嚴格度」,
結構性差異(觸發點/投递語義)登記分歧而非硬仿;禁向 C ABI 兼容漂移(ADR-0003 圍欄)。

## wasm/host facade 增補(2026-10-23,ADR-0003 延伸)

模擬器對外三面(native CLI / vl-wasm 瀏覽器 / vl-ffi host FFI)的新能力,
四證同落(繼承 rVPI 四證制):
①wasm 差分+功能閘+渲染閘+spec 層閘(`wasm-test/run|features|vcd_parse|vcd_svg|spec.mjs`,regress 自動;spec=ADR-0007 閉環:correct 0 違例/負向指名/BigInt/確定性)
②host FFI 差分(`bun-example/run.mjs`+`spawn.mjs`+`vcd_read.mjs`,regress 自動;dylib 缺=閘略過並提示)
③demo serve smoke(資產 200+no-store+corpus 白名單探針,regress 自動)
④**互動頁交付前必真瀏覽器目檢**(點 Run→stdout→canvas 波形;截圖存
docs/e2e-report/,curl 不算目檢)。

**VCD 正典(ADR-0004)**:讀寫一律走 `vl-vcd` 單一核心——CLI/wasm/FFI/difftest
共用同一 reader;新 tier 手搓 VCD parser=違例(JS 手搓版已退役);
新容忍度先入 docs/GTKWAVE-VCD-STUDY.md 採用/拒收表+ADR-0004 修訂;
writer 值介面=4 慢字串,零漂移錨=e2e expect.vcd byte-diff。
**SVG 波形正典(ADR-0005)**:波形渲染一律走 vl-vcd render(確定性 SVG、
值進 title、XML 轉義),vcd2svg CLI/vcd_to_svg wasm/rcs_vcd_svg FFI 三面同源;
GUI 產物=可 diff SVG(golden byte-diff),點陣 canvas 波形=違例;
時間參數跨層=u64(BigInt)+flags,禁 f64 哨兵。

契約備忘:wasm 回調只收 `{time,value,valueHex}`(JS→sim 中途重入=禁,
讀值在 run 前後);u64 跨界=BigInt;`$readmem` 在瀏覽器一律走虛擬注入
(`inject_file`/`run_verilog_with_files`),不碰真 FS;js_sys 表面一律
`#[cfg(target_arch = "wasm32")]`(host 構建用 stub)。

- **linux-only 上游工具的正典路徑(2026-10-26,XLS 弧定調)**:上游二進制只有 linux-x64 時,**CI=linux oracle**——workflow_dispatch 自助觸發 ubuntu runner 實跑(收割/樹內 bazel build),artifact 自帶授權、本機 `scripts/xls_adopt.py` 驗收;機械閘=in-tree job+regress 的 adapter/selftest 行,證據=workflow run + 快照 manifest。本機脫機路徑=Apple `container`(macOS≥26+Apple Silicon;**x64 二進制經 Rosetta 跑 amd64 容器,必須配 glibc 基底影像[ubuntu],alpine/musl 缺 ld-linux 必敗**;暖跑實測 0.83s);OrbStack/colima 為替代,XLS-ADAPTER.md 有配方與花費實錄。
- 雙 oracle 紀律(2026-10-24,riverilog R0 起):iverilog 13.0/vvp=第二 oracle;期望錄製要帶 `-n`(防互動態);iv-e2e 案例一律自寫 TB+錄製,iverilog 主樹 ivtest/=GPL-2.0 唯讀挖掘永不入 git;iverilog 特有行為(分層隊列/$stop 四行橫幅/sorry 語義)以 corpus 釘死,分歧入 rivD 簿。

## riv runner 增補(2026-10-24,C4)

- **正規化=丟行白名單,禁內容改寫**:riv-difftest e2e 臂丟 vl-difftest 四前綴(`- `/`-Info`/`-Warning`/`-Error`,字串前綴匹配與 vl-difftest 逐字一致)+ riv 自家工具行形(`<file>:<line>: $finish called at `/`VCD info: `);設計輸出 `-7div2=-3`、`tb.v:6: <非 finish 設計行>` 必須存活——敵意 fixture 單測釘死(riv-difftest 內 `#[cfg(test)]`)。
- **e2e ledger 檔**:預登記分歧=案例目錄 `riv-pending.txt`(rivD7 預登記 2026-10-25 撤回:live probe 證 iverilog 13.0 與 verilator 金樣數據行逐字一致,48_frac_delay pass 無需 ledger;機制常駐,首個真分歧時啟用)。分類:diff+ledger=pending、pass+ledger=DRIFT 紅燈(ledger stale)、diff 無 ledger=UNLEDGERED 紅燈、編譯/執行帶 `milestone R` 指針=DECL pending(兩種訊息形——無括號 R0 形與 §5-5 括號形——子串 `milestone R` 通吃)。
- **regress 執法=精確數基線**:`SUMMARY e2e=42/59 iv=0/0` 與 `SUMMARY e2e=0/0 iv=8/10` 逐字 grep(C4 定盤 28/59、4/10→2026-10-25 R2/R3 弧末 42/59、8/10;落地能力時同輪上調基線與翻 case.json/ledger),UNLEDGERED/DRIFT grep==0;runner 本身 exit 恆 0,执法力全在 regress grep。

## ryosys 增補(2026-10-27,M0;ADR-0010)

- **RTLIL-first 鐵則**:ryosys 輸入一律 oracle 產的 RTLIL 文本(`-readable` 形);golden 從 `vendor-reference/suite-dl/oss-cad-suite/bin/yosys` 實跑錄製(`ry-difftest --record`,cwd=案例目錄相對路徑),測試運行零 oracle 依賴。細節分歧**以二進制實跑為準**(v0.70 clone 落後 179 commits;stat 表頭 2 行形/boxed 模組靜默跳過/`Warnings:` 彙總行皆二進制釘死)。
- **兩層 registry**:`-H` 普查 306 條逐字入 crates/ry-cli/src/registry.rs+PROVENANCE;普查外名字=逐字重現 oracle `No such command` 錯;登記層命中=`ERROR: <cmd>: not implemented in ryosys (milestone Yx)`(Y0..Y3 映射見 ADR-0010)。**`-p` 切分=token 尾分號**(`clean;clean` 無空白=單一未知命令;`;;`→追加 clean、`;;;`→clean -purge;`#` 註釋;引號內 `;` 不切)——暗角形 fixture 組在 ry-cli cli_contract.rs,擴命令面前先釘。
- **迭代序契約(hashlib 實測)**:dict/pool 正向迭代=**逆插入序**(check/stat 的模組/cell/wire 迭代與警告序);write_rtlil dump=插入序;write_json 先 `design->sort()` 名序——兩出口順序禁互相推導。cell 端口名以 `\Y` 轉義形存儲(yosys ID::Y 內部即 `\Y`),方向表比較在 unescape 域。
- **IdString intern 首見序法條(2026-10-29,M2 續弧)**:`IdString::operator<` 比 **intern index**(rtlil.h:413-415;global_id_index_ 首見遞增),**不是 String 字典序**——凡以「名序」迭代/排序的面(TopoSort 的 compare_ptr_by_name 等)一律 intern 序。9534a7f4d 二進制實測律=**建名時刻升序**(autoidx 名=創建序;literal 名同律);禁以 v0.70 源碼 autoidx 負索引編碼當律(`-(autoidx++)` 推出「後建者在前」與二進制實測相反)。ryosys 落地=Design.intern_seq(read_rtlil 檔案序登記 module/wire/memory/cell/process 五類)+autoidx_base 分界(pass 建名 `$…$N` 以 N≥base 派生=N 序=創建序);消費者=opt_expr TopoSort(roots+入邊)。機檢=ys-e2e 406/406(subbytes/loops/constmuldivmod byte-exact 釘死;M2 -pb/-pbo 分層時點,現行閘=461/471)。**字串序例外(2026-11-01,M3 續弧批量收編;第二例)**:opt_balance_tree 的 TopoSort=`TopoSort<Cell*, IdString::compare_ptr_by_name<Cell>>`(opt_balance_tree.cc:271;utils.h:132 node_to_index=std::map<T,int,C> 以 C 排序根遍歷+edges set 同 C+DFS 後序)=**yosys 自指定 String 字典序**,異於 intern 序律——ryosys 落地=byte-wise 名序比較(禁 intern_key);仲裁=carryadd obt 錨 byte-exact。第一例=D-ys8b lt_by_name(compare_signals 尾分支)。凡新名序消費面先分類「本 pass 自指定比較器 vs 默認序」,禁套單一律。**第三例(2026-11-02,批量收編第二批)**:torder 的 `TopoSort<IdString, RTLIL::sort_by_id_str>`(torder.cc:81;sort_by_id_str=lt_by_name,rtlil.h:794)=自指定字串序——torder.rs 全鏈(node_to_index 迭代序/edges set 序)按 byte-wise 名序(禁 intern_key),carryadd 錨 byte-exact。前端分界實況:read_rtlil 檔案序登記五類;jsonin/hierarchy 前端**不登記 intern_seq**——此時僅**無數字尾**的未登記名=(0,MAX) ties 回落 cells 向量序(stable sort,確定);**jsonin 的 autoidx_base=1 使 `$<N>` 數字尾名(yosys JSON 私有名 `$abc$123` 形)落入建名域 (1,N) 按 N 序、不回落**——語料面不觸(-rj 腳本不跑 opt_expr;4 anchors 實測無 `$N` 字面名),擴 read_json→opt_expr 面前先修此邊界(jsonin 補登記 intern 或重定義 base 語義),禁按「未登記必回落」舊句排查。
- **opt 家族契約(2026-10-30,M3 實作輪)**:`opt` 編排器=序幕
  `opt_expr`+`opt_merge -nomux` → do{muxtree,reduce,merge,[share(-full)],
  dff,[hier(-hier)],clean,expr}while(scratchpad `opt.did_something`)→
  `Finished fast OPT passes.`——`Rerunning OPT passes. (Maybe there is
  more to do..)`/Finished 皆 log_header 占編號(golden 內容);changed
  置位面=`rg 'opt.did_something'` 全表(PROVENANCE P8,禁腦補)。
  opt_clean 階段序=remove_temporary_cells→cells(intern 序刪)→signals
  迴圈→init;`selected_modules()` 跳過盒模組;SigMap::add(bit)=代表元
  promote。**多刪 swap-and-pop 幸存序=D-ys8**(集合同、序不同;四單序
  假設模擬不達,禁硬磕;known-gap 名單=regress S4 逐字)。dump 模型
  補註:rtlil backend 印 `reversed(module->cells_)`=**陣列正序**(與
  dict 正向=逆插入是兩個面,勿混)。**M3 續弧批量收編(2026-11-01)四單元**:log(拼接+未知 -token 原樣
  打印+七旗標 Y2,D-ys10)/help(HELP_SHORT 306 條錄製表=gen_help_desc.sh
  機械生成落 help_desc.rs 禁運行時排序;bare=312 行歸一化形 vs oracle
  -H 剝面零差;子形=Y2)/clean(run_opt_clean 拆 (verbose,purge) 兩參;
  cmd_error 全形=register.cc:174-192 同構 stderr 三行+stdout Syntax 塊;
  process 面=預過濾靜默異於 opt_clean 迴圈內 warn;`;;;` 端到端不佔
  編號)/opt_balance_tree(worker 同構移植;**hashlib 逆插入序五消費點**
  [sources/sinks/BFS 載荷/consumed/sig_to_sink pool;begin()=size-1];
  NEW_ID 建名配對 leaf cc:97/wire:108+combine cc:132/wire:143,combine
  wire 被內部 clean -purge 折疊刪名;experimental 機制首消費者=
  log_experimental 去重+driver.cc:721 flush 行;尾 run_pass("clean
  -purge")=yosys.cc:880 橫幅不佔編號)。**0.70+179 新形 cells dbg
  verbose 門控**(cells_temp/cells_all/inits 三處 if(verbose);clean 形
  零計數——min2 錨抓出)。閘=ys-e2e 461/471(450+anchors 11)+LIVE
  456/456;registry 271 stub/35 實現(餘 Y2=6)。
- **批量收編第二批六命令(2026-11-02,§11 實作輪)**:async2sync(FF 異步
  信號同步化;selected_cells 值拷貝=**逆插入序** `iter().rev()`;NEW_ID 錨
  =clone v0.70 與二進制**零偏移**[async2sync.cc 全檔行號=二進制行號:
  arst 253/257/262、aload 220/221/225/226、sr 155..197、latch 285..345;
  rtlil helper wire 錨=二進制觀測 Not 3201/And 3249/Or 3250/Mux 3337,
  禁以 clone 推 rtlil.cc 行號——clone 與二進制在 rtlil.cc 漂移非線性])/pmuxtree
  (recursive_mux_generator 遞歸+or_generator 四臂;建名=`$auto$pmuxtree.cc:
  65:recursive_mux_generator$N`/`:100/:101:execute$N`/`:35/:37:or_generator$N`
  實名形)/dffunmap(unmap_ce=ff.cc:592/:594、unmap_srst=:612/:614,FF 全變
  $dff 靜默面)/setattr(-mod/-set/-unset 全收;值語法=雙引號剝引號/十進
  32-bit/`N'b`/`N'd`;失敗=`ERROR: Can't decode value '<v>'!` rc=1;執行體
  **零 header 不占編號**;do_setunset=wires+cells+memories rev)/torder(
  header+`module <名>`+`  cell <名>`;autostop=Q/CTRL_OUT/RD_DATA+
  $memrd(_v2) DATA)/edgetypes(四元組 type 形字串+pool 去重+sort 正序;
  **無 header 不占編號**;multibit=`type.port[index]` 下標形,手寫小錨彌合
  =edgetypes-multibit)。**錨輸入面律**:錨腳本前置鏈禁踩 known-gap 案例
  的 `proc;opt` 鏈(D-ys8c 面2 cascade 不可收斂)——pmuxtree/dffunmap 錨=
  `proc;` 直跑形;decode 錯誤形錨=expected.rtlil 面外(compare 跳過),
  契約由 cli_contract fixture 逐字執法。registry 收口=stub 265(Y0 257/Y2 6/
  Y3 2)+實現層 41=306;**help 全文常量六份**=scripts/gen_help_batch2.sh
  機械轉錄 help_desc.rs(禁手寫);cmd_error 全形六消費面=stderr 三行+
  stdout header+Syntax 塊+help 全文(selection 裸字=cmd_error=D-ys11)。
  閘=ys-e2e 469/479(461+anchors 8 入 compare 面)+LIVE 456/456。
- **歸一化**(ry-difftest):stdout 結構切(prologue/epilogue;**末尾換行分裂的空元素非內容行**)+.il 剝首行+JSON creator;stderr 零歸一化——成功路徑兩端 stderr 必空(多打一行 eprintln=紅)。敵意 fixture 單測釘死(tests/normalize.rs)。
- **regress 執法**=`SUMMARY ys=469/479` 逐字 grep+known-gap 名單逐字(批量收編第二批基線 2026-11-02=461+anchors 8 例入 compare 面[decode 錨面外];known-gap 10 例維持;歷史鏈:M1 弧末 402/402=M0 層全量+87 -hier[wandwor-hier 已隨 M1-d wand/wor 移植補錄]+87 -rj+87 -sm→M2 實作輪 401/406[5 known-gap]→M2 續弧收平 406/406→M3 實作輪 418/460[42 known-gap]→續弧第 2 輪 429/460[31 known-gap]→續弧第 7 輪 450/460→M3 續弧批量收編 461/471→批量收編第二批 469/479[path_else 修補後 sincos/i2c 殘差=D-ys8c 面1 hash 內部態(內層 pattern_t 序新子面)不翻,錨 9 遷入 8 入面];LIVE 腿=456/456 record() 可再生面[anchors 24 例手寫面外])。
- **序差类收平流程(2026-10-29,M2 续弧立法)**:golden byte-diff 残差为**深树形/序差类**(cell 树形/输出序/计数差,非缺语义规则)时,动手前两步强制:①**定点读 oracle 对应 pass 的源码**(只做定位;序差结论一律二进制实跑轨迹仲裁——9534a7f4d 不可得时以 v0.70 28ba3cb 对读,禁以源码静态推导定案[autoidx 负索引编码类推律已证伪]);②**可复刻性分类**:**logic-diff**(机制可用我方建模复刻)=修 crates/ry-* 收平,golden 零重录;**internal-order**(依赖不可推导的二进制内部序,先例=check 环起点 D-ys5)=在 REGISTRY 登记分歧行(源码级证据+探针实证)不硬磕,同轮翻 regress.sh 执法块精确数与 known-gap 名单(名单=防回归锚,与 difftest 实跑 FAIL 集逐字一致)。分类前禁按旧文档归因动手——实作轮续弧指南四条入口三条被对读证伪、一条精确化(RYOSYS-MVP §8.8-2/§8.8-4;本轮五例全为 logic-diff 收平=流程正例)。
- **工具鏈對齊(2026-10-27,M1 slice-1/§7.0-4 實錄)**:默認 `xcode-select` 指向 Xcode 26.6 而 CLT 27.0 並存時,Xcode 的 ld 讀 CLT SDK 的 .tbd 報 `tapi error: malformed file`(arm64e.x1 等 token),一切需 C++ 鏈接的腿(rv-diff/toolbox)必紅——lint-only 無鏈接不受累。防禦=regress.sh 頂部 3 行:`DEVELOPER_DIR` 未設且 `/Library/Developer/CommandLineTools/usr/bin` 在場才 export CLT;顯式設置尊重不覆蓋(用戶主權,設壞鏈=預期 FAIL 非 bug,RM1-5);無 CLT 機器 `[ -d ]` 短路零影響(RM1-6:未模擬,設計保證)。驗收兩路徑=裸跑+`env -u DEVELOPER_DIR` 形。
- **commit message 数字律(2026-10-30,收口弧反省立法)**:commit message 中的行数/计数一律取**提交时刻** `git diff --cached --stat` 实测;起草于提交前的数字(规划时点统计)禁直接写——起草后 diff 范围再变(如换人复审修订并入)必须同步改 message 再 commit。实录=3a696ba(2026-10-29 20:37:47,message「183 行纯插入」)24 秒后被迫 amend 为 df54db3(「310 行新增…4 行删除」)。AGENTS §8 commit 条同句援引。

## 分析器服務增補(2026-11-02,S1..S5;ADR-0011)

- **server=新 face,先落 core(10.7 同款)**:全部語義能力(`analyze`/`Analysis` 投影/ASP/1 codec/§13 報告編碼)落 `analyzer-core`;`analyzerd` 只做「接收→解碼→調 core→編碼→回寫」,零語義;未來 wasm/FFI 面直呼 core 零複製。
- **ASP/1 編解碼律=契約,禁兩端各自演化**:正典寫律/禁項見 ANALYZER-SERVICE §5.2/§5.3;單一事實源=`clients/analyzer-ts/fixtures/codec-vectors.json`(生成器首錄=analyzer-core tests/codec_vectors.rs 的 `regen_fixtures` #[ignore] 測試,雙端斷言維持;analyze 響應向量由**真編碼器**產出,穩定性測試釘 stale=重錄收據)。TS 側三禁(bun 1.4.2 實測放行行為):禁 `JSON.parse`(浮點/指數/重複鍵/越 2^53 全放行)、禁 `JSON.stringify` 含 Int(number 丟真/bigint 拋 TypeError;Int 恆 `BigInt.toString(10)`)、禁 `toString('utf8')` 做校驗(U+FFFD 靜默替換)——decode=`TextDecoder('utf-8',{fatal:true})`+手寫遞歸下降。
- **UDS 路徑與殘留**:路徑字節 ≥100 硬錯 exit 1(macOS sun_path 104 含 NUL 留 4 餘量);啟動先試連——連通=活實例硬錯 exit 1、連不通=unlink 重綁(煙測冪等的前提);綁後 0o600。
- **超限幀先檢查後分配**:讀到 4 字節前綴即 `check_len`,違約回 E_PROTO 幀後關連線,不讀負載不分配;client 對稱律=編碼後寫出前檢查(RangeError 不發幀)。
- **macOS accepted fd 繼承 listener O_NONBLOCK(實錄)**:non-blocking listener 的 accept 產物流在 macOS 繼承非阻塞態,conn 線程首讀 EAGAIN(os error 35)勿誤判 Eof——accept 後立即 `set_nonblocking(false)`。懷疑掛死照鐵則 §4 `sample <pid> 1` 看棄直覺。
- **測試腳手架**:spawn 的 server 無 shutdown 觸發不會自行退出——收尾必 kill+wait(Guard Drop);固定 socket 路徑在 cargo 並行測試下互踩(R1-13),每測 tmpdir+唯一短路徑;測試 tmpdir 清理上收 Guard,stale-socket 類測試的殘留檔=受試物時以 `std::mem::forget` 保全。
- **bench 資訊性禁閾值**:smoke 打印 per-op 延遲與啟動延遲,不引 criterion、不設 pass/fail 時間斷言(脆弱閾值=假紅源);數字入 REGISTRY/PLAN 行(一次性先例形)。
- **新 crate 同輪 REGISTRY 登记**(`analyzer-core`/`analyzer-server` 兩行已入 Crates 表;閘=api/codec 向量/loopback/smoke 四層)。
- **regress 執法面(2026-11-03 冒煙閘收編)**:S5 `analyzer smoke` 塊=regress.sh 內既有塊(雙守衛降級形:crates/analyzer-server/Cargo.toml 在場+`command -v bun`;在場時 `bash clients/analyzer-ts/run-smoke.sh` exit≠0 即整閘紅;向量閘由 run-smoke 內建 `bun test` 步承載,regress 一層調用即覆蓋,無需雙跑)。凡測試計數入文檔(§17/PLAN/REGISTRY)=**當輪實跑取數**,同弧後續片增測試必回頭重測重寫(S2 行 36≠分解 32 實錄,收平輪立法)。
- **CLI 優先級解析律(收平輪立法)**:旗標值解析(`parse_args`)只記 Option,優先級落地(旗標>env>默認)收斂單點(resolve 函數,env 注入式)——**禁在旗標 setter 內硬編碼默認值搶跑 env**(單獨 `--max-frame` 繞過 `ANALYZER_SOCK` 實錄);優先級回歸=單測三象限+集成真 env 兩腿。
