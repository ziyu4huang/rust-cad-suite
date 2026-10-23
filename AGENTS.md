# AGENTS.md — rust-cad-suite 开发守则(所有 agent session 载入)

本 repo 用纯 Rust 重写 oss-cad-suite 工具集;Phase 1 = `rverilator`
(Verilator 等价模拟器)。以下铁则来自 2026-10-21 M0→M5+E2E 弧的实錄
(详证 = docs/REFLECTION-agent-hardening.md),违反会产出「看起来绿、
语义错」的工具。SSOT:PLAN.md(路线)、docs/adr/(决策)。

## 1. Oracle 即 spec(最高优先)

- 兼容产品的正确性标准 = 参考工具的可观测行为。语义分歧时**oracle 赢**,
  即使 LRM 站你那边(实录:超长 sized 字面量 LRM 说截断,verilator 报错 →
  跟 oracle)。LRM 只用来解释行为,不用来覆盖行为。
- 期望输出一律 `cargo run -p vl-difftest -- --record` 从 oracle **录制**,
  禁手写期望值。手写只允许错误路径形态(exit code / 里程碑指错信息)。
- oracle 实例:本机 `/opt/homebrew/bin/verilator`(5.050),仅测试用,永不
  随产品 ship;上游 test_regress 文件(LGPL/Artistic)禁入 git。
- 改了语义(求值/调度/格式化)后必须重录受影响 case 并读一遍 diff——
  期望变了要看是「修对了」还是「修偏了」。

## 2. 差分语料纪律(两条验收轨分立)

- byte-diff corpus(X-clean 政策):禁 x/z 传播样例、禁 z 字面量初始化、
  禁同边沿调度竞争(两个进程对同一边沿一写一读=IEEE 非确定序,不是测试)。
- 4-state 运算符正确性走独立轨:IEEE 1364-2005 §5 真值表单测(value.rs),
  与 oracle diff 无关、不受其 2-state 性拖累。
- 调度确定性契约:process 序=宣告序(ADR-0001);依赖未决顺序的 TB 按
  无效测试处理,改写或标 known-diff。
- corpus 文件必须过 oracle 的 warnings-as-errors(警告=代码质量信号,
  先修样例再录)。
- 每个新 case 落地三查:语义 X-clean?顺序确定?oracle 与我方都能终止?

## 3. 不支持的功能=带里程碑的硬错

未实现构造必须 `error ... (milestone Mx)` 硬错(exit 非 0),禁静默跑偏、
禁空实现。E2E 失败因此自分类:报错带 Mx=范围外;崩溃/数值错=bug。
新支持某功能时:删对应 corpus 的 fail、补 PASS case、更新 PLAN.md 里程碑表。

## 4. 错误恢复必须保证前进(带挂死 fixture)

- 每个恢复循环(sync_stmt/sync_item/模块循环/块循环)必须带
  「pos 未动 → 强制前进」守卫;结构关键字(end/endtask/…)既不消费也不
  终止外层循环 = 死循环(实录:`task one(input integer n);` 挂死)。
- 每个恢复点配一个会触发的挂死形 fixture 进 corpus/tests。
- 疑似挂死:先 `sample <pid>` 看栈,再改代码;E2E 运行带每例看门狗。

## 5. 4-state 值语义:算子与真值表同生

- 每个算子落地时同轮写 x/z kind 保全单测(~x 把 x 翻成 z、>>> 按值宽而非
  64 位填充、merge 的 0 强迫须双方皆 0——三例都在案)。写法:IEEE §5 表
  先行,算子后行。
- 格式化是契约:`%0d`(极小)/`%d`(类型宽空格填充)/`%0Nd`(零填充)/
  `%t`(precision 单位、缺省宽 20)…… 全部 golden 钉死,oracle 仲裁。
- 位宽/符号传播按 §5.4/§5.5:上下文决定操作数强制到表达式宽(符号仅当
  整体 signed);自决定子上下文(比较两侧/移位量/拼接/条件)各自开新上下文。
  加法进位保留是验收项(`wire [8:0] s = a + b`)。

## 6. 未解析即硬错

名字解析走作用域链,未命中=报错(带 span)。禁盲字符串前缀——第一版
盲前缀靠扁平命名空间碰撞蒙混过关,把 ANSI 端口未建 cell 的真洞埋了两周。
每个声明形态(含 ANSI 端口、generate 作用域、函数静态局部)都必须注册。

## 7. 语义敏感手术纪律(承主 repo 教训)

- 改文件一律脚本化手术+命中计数断言(`count==1` 才写);脚本改过的文件
  再用 Edit 前必须重读(staleness 拒写是常态不是事故)。
- 跨目录引用(include_str/相对路径)先数层级:tests/ 到 repo 根=三层。
- 语义重写(求值器/调度器级)完成后,先跑全量 cargo test 再跑 E2E;
  两者都绿才算「改完」。

## 8. 交付闸(合并前必过)

1. `cargo build` 零 error 零 warning;
2. `cargo test` 全绿——其中 `vl-difftest` 回归闸断言 E2E 通过率 ≥80%
   (期望已入库,无需本机装 verilator;当前基线 31/32=96.9%,唯一 FAIL
   = 30_gen_mux 存储器数组 M7 硬错,属声明范围);
3. 新语义改动附:对应 diff 修复说明(哪条 oracle 证据)或新 case。
- git 提交单文件精确 add;commit message 禁反引号与 `$(…)`。

## 9. 定位工具箱(实录验证)

- 挂死:`sample <pid> 1`(macOS)看栈,别猜;E2E runner 自带
  `--only <case>` 二分。
- 语义分歧:直接在 /tmp 写最小复现,`rverilator --binary` vs
  `verilator --binary --timing` 并排跑;oracle 输出即答案。
- /tmp 会被外部清理:关键复现文件放 repo corpus/,复现脚本可弃。

## 10.5 第二輪補強(2026-10-22,M6/fuzz 弧實錄)

- **歸一化=契約**:diff 期望的過濾只用精確前綴(`- ` 報告塊、`-Info/`
  `-Warning`/`-Error`);裸 `-` 開頭是設計輸出(`-7div2=-3` 實錄)。歸一化
  由單測釘死(含敵意 fixture)。錄製必帶 `--trace`(oracle 才執行
  \$dumpvars,否則混入 `-Info ignored`)。
- **re-record=語義事件**:錄完必讀 `git diff --stat corpus/e2e` 逐 case 對帳;
  期望變了而模擬器沒變=必須能解釋,否則停。
- **競爭免疫模式機械化**:TB 中 `@(posedge..)` 語句窗內對「被邊沿 always 讀取
  的變量」的阻塞寫=違例(corpus_lint R1;43 號兩改一漏實錄)。正典寫法
  `@(posedge clk); #1; <寫>;`。`// race-ok` 顯式豁免。
- **代碼片段跨語言插入必走檔案**(Write 片段檔→插入),禁把 Rust 代碼內嵌在
  Python 字符串字面量裡(r#"…"# 誤用實錄)。
- **gate 鏈裡的計數 grep 加 `|| true`**(`grep -c` 零匹配 exit 1 斷鏈,
  PLAN.md 漏提交實錄);commit 後終檢 `git status --short` 必須為空。
- **分歧登記簿**:凡是「我方與 oracle 已知且接受的不同」(參數不入 VCD、
  競爭序=宣告序、2-state X、超長字面量報錯跟 verilator……)記入
  docs/REGISTRY.md「Divergence register」,帶證據與政策;新分歧先登記再繼續。
- **事故登記簿**:工具鏈開發中的每次 panic/hang/假綠按五欄
  (日期/症狀/根因/修法/硬化層)記入 docs/REGISTRY.md「Incident register」。

## 10. 质量体系 3-tiers+1(与 OKR/登记簿联动)

- 任何新铁则必须**同轮落四层**,缺一不算改规则:
  ①机械闸(`scripts/regress.sh` 或 vl-difftest 内断言,exit≠0 才有执法力)
  ②正典(`.agents/skills/eda-dev/SKILL.md` 条文)
  ③本文件对应节
  ④证据页(`vl-difftest --html` 生成的 docs/e2e-report/index.html,机械生成禁手改)。
- 质量目标与证据指针=`docs/OKR.md`(KR 证据只指可重跑命令/工件,反灌水);
  组件×能力×閘×證據的關係表=`docs/REGISTRY.md`(改組件職責同輪更新)。
- 提交前必跑 `bash scripts/regress.sh`;`--with-record` 轮之后必须人工读
  `git diff corpus/e2e`(期望变更=语义变更的收据,不许盲提交)。

## 10.6 探針插拔手術紀律(2026-10-23 立法;點星 regex 吃掉 GenBlock+三臂實錄)

- 調試探針(eprintln/debug print)的**插入與移除都是文字手術**:錨點必須含
  探針完整獨有內容(含其閉括號行),移除後必 `cargo build` 斷言零錯才續行。
- **禁 `[\s\S]*?`/`.*` 點星 regex 跨行刪除**——點星會跨過探針自身的閉括號
  吃到下一個同縮排 `}`, collateral 是整段函數臂。刪除錨=「探針首行…探針尾行」
  精確字面量;探針寫成單行或用唯一標記註解(`// probe-xyz`)包裹,刪除按標記配對。
- 外部語料收編(lint-diff)已入策略:每里程碑收編一批外部測試檔,
  accept/reject 差分=結構性 bug 最快探針(REFLECTION §14.1)。

## 10.7 三面 facade 紀律(2026-10-23 立法;rVPI+wasm/FFI+XLS 弧實錄)

- 模擬器三個對外面(native CLI / vl-wasm 瀏覽器 / vl-ffi host FFI):
  **新能力先落核心(vl-sim)再露面**,禁只在單一面實現($readmem 注入
  落 Sim 一處三面同受益=正面先例;REFLECTION §15.1)。
- rVPI(`crates/vl-sim/src/vpi.rs`)能力變更:回調觸發點/投递語義的改動
  必同輪更新 ADR-0003 觸發點表+vpi_gate 斷言+REGISTRY 分歧行;
  禁向 C ABI 兼容方向漂移(非目標圍欄在 ADR-0003)。
- wasm 面:js_sys 表面必 `#[cfg(target_arch="wasm32")]`(host stub);
  u64 跨界=BigInt;回調只收 `{time,value,valueHex}`、JS→sim 中途重入禁;
  `$readmem` 瀏覽器一律虛擬注入(`inject_file`/`run_verilog_with_files`)。
- host FFI 面:ABI 用行式 manifest(零 serde);返回串以 `ERROR: ` 前綴
  分支;`rcs_free` 配對釋放。
- 外部工具產物收編(XLS 類):正規化(always_ff→always/timescale 注入)
  在收割管線內做+raw 保留+記在快照 README;上游工具介面衝突以
  `--helpfull`/stderr 為準,禁盲猜(REFLECTION §15.3)。
- 互動頁交付:serve smoke(資產 200+no-store)與真瀏覽器目檢
  (點 Run→stdout→canvas)兩者不可互替;截圖入 docs/e2e-report/。
- VCD 讀寫一律走 `vl-vcd` 單一核心(ADR-0004):CLI/wasm/FFI/difftest 共用
  同一 reader;**新 tier 手搓 VCD parser=違例**,JS 手搓版已退役;
  新容忍度先入 GTKWAVE-VCD-STUDY 採用/拒收表+ADR-0004 修訂,禁 quiet tweak;
  writer 零漂移錨=e2e expect.vcd byte-diff,值介面=4 慢字串(勿改回 Bits)。
- **GUI 憲法(ADR-0005,2026-10-23 使用者定調「GUI 我們都用web做,需要的底層都用
  rust寫,然後cli 與web都共用 web透過wasm 共用,波型生成要用SVG」)**:GUI 一律 web;
  GUI 所需底層一律 Rust 核心先落;CLI 與 web 共用核心,web 經 wasm;波形渲染=
  vl-vcd render 出 SVG(確定性+XML 轉義鐵律,golden byte-diff 釘死),
  **點陣 canvas 波形=違例**;時間參數跨層=u64(BigInt)+flags,禁 f64 哨兵;
  機械閘=regress 的 wasm vcd_svg+ffi vcd_svg+cargo render golden。
- 三面能力變更的機械閘=regress.sh 內(E2E 精確數 100%/lint parity 基線/
  wasm 差分+功能+vcd_parse/demo serve smoke+corpus 白名單/ffi 三閘含
  vcd_read)必綠;條文正典=SKILL「wasm 增補」節。
