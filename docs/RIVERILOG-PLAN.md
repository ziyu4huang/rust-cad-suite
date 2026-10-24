# riverilog — Icarus Verilog 等價模擬器企劃書(正典)

> Suite 路線圖 **P3**(PLAN.md §1)。2026-10-24 立案:planner 出案 → 對抗審 →
> 合成正典;oracle=本機 Icarus Verilog 13.0(stable,/opt/homebrew/bin,僅測試)。
> 本文檔本身即本階段交付物;實作按 §8 里程碑排序,動工前先過 §12 open decisions。
> 命名:**riverilog**(專案)/`riverilog`(編譯器二進制)/`rivvp`(執行期二進制)。
> riv- 前綴與 rverilator 同理:不冒名 `iverilog`/`vvp`,免毒化自家 oracle(PATH 安全,
> PLAN §8 #2 先例)。

## 0. 已驗證事實(2026-10-24 本機;本文一切 CLI/行為宣稱的證據基礎)

- **Icarus Verilog 13.0 已安裝**:`which iverilog vvp` → `/opt/homebrew/bin/iverilog`、
  `/opt/homebrew/bin/vvp`。PLAN.md §0 的「iverilog 未裝(difftest 自動跳過)」**已過期**,
  R0 第一件事=更新該行。
- `iverilog -V` exit 0,橫幅列**四個組件**:driver + Icarus Verilog Preprocessor(ivlpp)
  + Parser/Elaborator(ivl)+ `FLAGS DLL vvp.tgt`(VVP Code Generator)——**編譯器是
  多進程+動態載入目標插件的架構**。
- `iverilog -h` **exit 1**(usage 印 stderr 後即退);`vvp -h` exit 0。契約不對稱,釘死。
- oss-cad-suite 實船 iverilog:`vendor-reference/oss-cad-suite-build/default/scripts/iverilog.sh`
  在列(同船還有 verilator.sh、cocotb.sh、slang.sh);**cocotb.sh 內 `rm -rf .../Makefile.verilator`**
  ——套件內 cocotb 以 icarus 為主模擬器的信號(動機屬推斷,信號本身是實測)。

### 0.1 核心行為證據塊(全部實跑輸出,非文檔轉述;對抗審註:逐字輸出依探針形狀——觸發方式/輸入——而變,如 nba.v 的 now=0/1、gates 的 t=10=0 均為其探針的實例值,序與語義才是釘死面;探針源 R0 歸檔)

**hello-world 與 .vvp 檔案**:
```
$ iverilog -o hello.vvp hello.v && vvp hello.vvp   # exit 0/0
Hello from Icarus 42
hello.v:4: $finish called at 0 (1s)
$ file hello.vvp
a /opt/homebrew/Cellar/icarus-verilog/13.0/bin/vvp script text executable, ASCII text
$ head hello.vvp
#! /opt/homebrew/Cellar/icarus-verilog/13.0/bin/vvp
:ivl_version "13.0 (stable)" "(v13_0)";
:ivl_delay_selection "TYPICAL";
:vpi_time_precision + 0;
:vpi_module ".../system.vpi";        # + v2005_math.vpi/v2009.vpi/vhdl_sys.vpi 等
S_0x1028715d0 .scope module, "main" "main" 2 1;
 .timescale 0 0;
T_0 ;
    %vpi_call 2 3 "$display", "Hello from Icarus %0d", 32'sb0...101010 {0 0 0};
    %vpi_call 2 4 "$finish" {0 0 0};
    %end;
    .thread T_0;
:file_names 3;
    "N/A";
    "<interactive>";
    "hello.v";
```
`.vvp` = **文字格式 thread-code**(shebang 可直執);`$display/$finish` 編譯成
`%vpi_call` 指令——**系統任務走 VPI** 是 vvp 的結構事實,不是實作細節。

**exit codes**:`語法錯=2`(附 "I give up.")、`elab 錯=2`(附 "N error(s) during
elaboration." + missing-module 清單)、`未知旗標=1`("iverilog: illegal option -- -")、
`-h=1`、`$finish 後 exit=0`、**`$error 系統任務不改 exit code`**(仍 0)、
`vvp -N` 於 `$stop` → exit 1、`-n` → 0。

**$stop 語義**:default 下 $stop 印 `** VVP Stop(0) **` 等四行,stdin EOF 後
**續跑模擬**(實錄:3 秒跑到 279M ticks);`-n` 使 $stop≡$finish。

**分層事件隊列(nba.v 探針)**:
```
ACTIVE sees a=0        ← a<=1 後,active 區讀舊值
post-#0 sees a=0       ← inactive 區仍讀舊值
other proc #0 sees a=0
a changed, now=0 a=1   ← @(*) 在 NBA 區提交後才觸發
```
IEEE 分層隊列(active→inactive→NBA→monitor)在 iverilog 是**可觀測契約**;
rverilator(vl-sim)的排程是 Verilator 形——這是兩引擎語義契約分岔的核心證據(§4)。

**$strobe 延後**:源序 `$strobe(...); a=1; $display(...)` → 輸出 display 先、
strobe 後(時間槽末)。

**X 傳播**:`a=4'b10x1` → `a+1=xxxx`(整 x)、`a&4'b001z=00xx`(逐位)、
`a|b=1011`、`if(a[2])` 走 else。**force/release 可用**;release 後 reg 保持強制值。

**-g 階梯**:`-g2005` 拒 `join_any`(exit 4);`-g2005-sv` 全通;`-g2012` 跑通
package/typedef enum/struct packed/always_ff/always_comb/st.name(),附
`sorry: constant selects in always_* processes are not fully supported`
——SV 支援是**明確的子集+sorry 語義**,這本身就是 iverilog 的相容面。

**延遲**:`nand #6`/`buf #2` 閘延遲可用(t=3 時 nand=x 未到期,t=10=0);
**`wire #6`(net delay)→ `sorry: Net delays not supported.`**——上游 13.0 自己都不支援,
riverilog 同位聲明即可。specify 預設 parsed-but-ignored,`-gspecify` 才 elaborates
(路徑端點必須是埠,實測報錯)。UDP 編譯執行可用。

**效能基準(perf.v:1ns 時鐘 #5 + always @(posedge) 計數)**:
```
$ time vvp perf.vvp
cyc=1000000 ... (0.21s user)
```
≈ **4.8M 週期/s(~10M 事件/s 量級,估計值)**。vvp 是 C 寫的 thread-code 直譯器,
Rust 直譯器合理目標=同量級的 1/2~1/5(§8 R10 訂門檻)。

**跨工具分歧實錄(二元 `~&`,單輸入三工具對照)**:
| 工具 | 結果 |
|---|---|
| iverilog 13.0 預設 | **接受**,c=1101 |
| iverilog `-gno-icarus-misc` | 拒:"The binary NAND operator is an Icarus Verilog extension",exit 1 |
| Verilator 5.052(本機現裝;PLAN.md:10 寫 5.050 已過期) | 拒:%Error,exit 1 |
| rverilator(本 repo) | 拒:VL-E020,exit 1(D6 對齊其 oracle) |

同一份源碼,**兩個正確答案**——rverilator 與 riverilog 的 spec 契約必須分家,
此為 §3 架構裁決的第一證據。

**CLI 細節**:`-E -o -` 預處理輸出流 stdout(可用);**`-o -` 對 vvp target=字面建出
名为 `-` 的檔案**(exit 0,怪癖);`-t null`=語法檢查 exit 0;`-t foo`=dlopen
lib/ivl/foo.conf 失敗(目標=插件);`-Mprefix=`/`-Mall=` 檔案格式實測("M mon.v" vs
"mon.v");`-c` 指令檔可用;`vvp +PLUSARGS` 只認 `+` 前綴(其餘走 vpi_get_vlog_info);
`IVERILOG_DUMPER` 環境變數+`-vcd/-fst/-lxt2` 延伸參數(man 實證);`-l logfile`
鏡像輸出;`-q` 抑制 stdout。

### 0.2 VCD 觀測(iverilog 側)
`$dumpfile/$dumpvars` 輸出:`$version Icarus Verilog`、`$timescale`=源碼 precision(不變式;無 timescale 的源碼實測印 1s)、
**`$comment Show the parameter values. $end`**、`$dumpall` 段、#0 `$dumpvars` 快照;
開檔時 stdout 印 `VCD info: dumpfile dump.vcd opened for output.`。
與我方 vl-vcd writer 的差異 → §6 正規化契約 + 分歧登記 rivD1。

## 1. 定位:為什麼需要第二個模擬器

### 1.1 角色矩陣(iverilog vs Verilator;全部有本 repo 證據)

| 維度 | Icarus Verilog(iverilog+vvp) | Verilator |
|---|---|---|
| 執行模型 | 事件驅動、分層隊列、**解釋執行** | 編譯為 C++、循環級、(--timing 前)零延遲 |
| 值域 | **4-state(0/1/x/z)全傳播**(§0.1 實證) | 2-state(D1:1x00+1=1001) |
| 時序 | `#delay`/閘延遲/transport-inertial/specify(-g)/SDF | --timing 子集;綜合視角 |
| 測試台 | full testbench:fork/join/wait/named event/force/release/$monitor/$strobe | 綜合子集為主 |
| 擴展 | **VPI/PLI 一等公民**(系統任務本身就是 vpi 模組,§0.1) | VPI 子集,以 C++ 模型為主 |
| 語域 | -g1995..-g2012 全階梯+icarus 擴充 | 進度上更貼 SV 綜合子集 |
| 生態位 | XLS 唯一內建模擬器(docs/XLS-VERILATOR-STUDY.md);套件內 cocotb 主力 | 快速回歸/lint |

**oss-cad-suite 兩者同船**(iverilog.sh + verilator.sh 並列)→ **parity requires both**。
rverilator 蓋 Verilator 的坑,riverilog 蓋 iverilog 的坑;二者是**互補角色**,
不是內部競爭:verilator 語料(綜合子集、2-state)流不進 iverilog 的測試台世界,
iverilog 的世界(X 傳播、延遲、VPI 測試台)verilator 語義上就接不了(§0.1 `~&` 三對照)。

### 1.2 對本 repo 的獨特價值(非 duplicate 的論證)

1. **REFLECTION §7 的單一 oracle 風險收口**:"iverilog as a second oracle would
   catch Verilator-specific quirks being copied as spec"——riverilog 落地後,
   rverilator 的 e2e 語料可加跑 iverilog 第二腿(三角驗證),抓「Verilator 特有
   行為被當成 IEEE」的盲區。
2. **雙引擎共存=只有本 repo 能做的產品**:riverilog↔rverilator 同進程 lockstep
   (§9.2)——「X-sensitive 對照」:同一 TB,4-state 腿與 2-state 腿在哪裡分歧
   =復位覆蓋/X 污染地圖。這是 ADR-0007 spec 層的天然延伸。
3. **XLS 整合的正字標記**:XLS 的 `VerilogSimulator` 介面唯一內建實作是
   iverilog 子流程(XLS-VERILATOR-STUDY.md:寫 top.v → `iverilog -o` →
   `vvp` → 收 stdout/stderr;**XLS 只執行 vvp、從不解析 .vvp**——自有格式不破壞介面;介面另含 RunSyntaxChecking 語法檢查面)。riverilog 填同一介面比 rverilator 更「原味」。

### 1.3 非目標(顯式)

- **不做** .vvp 位元組級相容(除非使用者翻案,§12 #1):`:ivl_version` 隨版本漂、
  格式無穩定契約文檔,追逐成本無上限;取**流程相容**(兩段式 CLI 形狀)。
- **不做** Verilog-AMS(-gverilog-ams 上游也幾乎不支援)、lxt/lxt2/fst dumper
  (VCD 為正典,FST=獨立 ADR 排程)、互動式 vvp 提示符的完整仿真(只對拍 $stop
  橫幅+EOF 續跑行為)、VHDL 目標(本地安裝內 vhdlpp/vhdl.tgt 在場證明連上游都
  只是局部——出範圍條件見 §9.7)、cocotb 全相容(R10 前不承諾,§12 #4)。
- **不重複** rverilator 已有面:lint 產品軌、tier-B 編譯、wasm playground 殼
  (riverilog 複用其模式,§9.6)。

## 2. Clean-room 與 oracle 方法論移轉

1. **oracle=本機 iverilog 13.0**(/opt/homebrew/bin,僅測試用永不 ship),
   AGENTS §1「oracle 即 spec」全文適用:期望一律 `--record` 錄製,禁手寫;
   LRM(IEEE 1364-2005 / 1800)只解釋行為不覆蓋行為——iverilog 的 `sorry:`/
   嚴格度差異(如 Net delays 不支援)本身就是 spec。
2. **語料授權(已逐項查證,2026-10-24 gh api)**:

   | 來源 | 狀態(實測) | License | 政策 |
   |---|---|---|---|
   | steveicarus/ivtest | **archived,OBSOLETE**,末推 2023-03;2607 .v(PLAN §7.5 口徑) | GPL-2.0 | 唯讀挖掘,永不入 git |
   | steveicarus/iverilog 主樹 `ivtest/` | **測試套件已併入主 repo**(master):4,140 .v = ivltests 3940 + vpi 87 + blif 44 + vhdl_tests 38 + fpga_tests 24 + contrib 7 | GPL-2.0(全庫) | 同上——**「iverilog 自家 testsuite」與 ivtest 同源同證**,無更寬鬆的第二來源 |
   | YosysHQ/yosys tests/simple | 已收編=corpus-external/yosys-simple(87 檔) | ISC | ✅ 入 git;**riverilog 第一批 e2e 語料直接重用**(同檔案、iverilog 錄第二份期望) |
   | cocotb / slang / oss-cad-suite-build | — | BSD-3 / MIT / ISC | 架構研究與生態對接乾淨 |

   - **GPL 防疫鐵則**(沿用 test_regress 先例+升格):GPL 源只准「看→自寫新案例→
     oracle 錄期望」;期望檔是行為事實的錄製(非 GPL 文本的衍生);**任何 .v 原文
     落進 corpus = 事故**,REGISTRY incident 五欄入帳+全量掃描。
   - 挖掘優先序:ivltests/ 按主題抽角案例(X 傳播/延遲/UDP/VPI/PLI 區域),
     vpi/ 87 檔是 R7 的黃金素材;yosys-simple 先行(R1 就能跑)。
3. **分歧登記簿**:riverilog 起獨立 rivD 前綴(§6 預載 rivD1-rivD6),
   「先登記再動工」同 REGISTRY 紀律。
4. **核心 crates 零新依賴**(workspace 既有政策);riv-* 新 crate 全部零外部 dep
   (vl-cli 慣例:手寫 flags)。
5. **雙 oracle 三角**:rverilator 既有 e2e 語料(X-clean 子集)加跑 iverilog 腿=
   `tri_gate`(§8 R1);兩 oracle 一致的行為=硬通貨,分歧=逐條入分歧簿
   (預期分歧區:X 傳播/排程/~&——都是已登記的 D1/D2/D6 家族)。

## 3. 架構選項分析(核心裁決)

### 選項 A:復刻 ivl→vvp 兩進制流,含 .vvp 形序列化設計+threaded-code VM
- 優:CLI/生態形狀 1:1(編譯/run 分離、編譯一次多次跑、XLS 的兩段式介面零改動);
  .vvp 是文字可 diff;系統任務=VPI 的上游結構可直接映射。
- 劣:**.vvp 格式本身是上游內部契約**(`:ivl_version` 帶版本、無規格文檔、
  隨版本演進)——位元組相容=無上限的追逐;threaded-code VM 是第二個執行表示,
  首個里程碑就要付雙倍表示成本。
- 裁決:**取其 CLI 形狀,棄其檔案格式**。

### 選項 B:兩個入口(iverilog/vvp 形 CLI 面)共享記憶體內 IR+直接事件驅動解釋器,無序列化
- 優:單一執行表示;直譯器=ADR-0001 樹遍歷先例的 iverilog 版;核心語義一次做對;
  與 rverilator 共享葉子最大化(§3.1)。
- 劣:`vvp` 入口沒有東西可讀——兩段式流程退化(要嘛同進程、要嘛序列化)。
- 裁決:**為主幹,補一件小事**——R6 加**自有格式** `out.riv`(versioned、文字、
  確定性 byte 輸出)作為兩進制的流通物:流程相容上游生態,格式主權在自己手裡。

### 選項 C:擴展 vl-sim 到 4-state/delays(誠實的失敗分析)
表面誘人(vl-sim 已是 4-state 值+事件喚醒),但**三層結構性失敗**:
1. **語義契約對撞**:rverilator 的 spec=「match Verilator」(AGENTS §1+D1-D11
   分歧簿全體),riverilog 的 spec=「match iverilog」。同一輸入兩個正確答案的
   實證:二元 `~&`(§0.1 三對照表)、分層隊列(nba.v:#0 區讀 NBA 前值)vs
   vl-sim 的 Verilator 形 settle+NBA 排程、X 傳播差異族。要一個引擎同時滿足
   兩契約=調度器全面方言化,比兩個薄引擎共享葉子**更貴更險**。
2. **進程模型不相容**:vl-sim 進程=segments+顯式掛起點+組合收斂 settle 迴圈
   (UNOPTFLAT 上限=Verilator 哲學);iverilog 語義=協同執行緒(每執行緒
   自帶 PC+局部)+分層區域+閘/net 慣性延遲+force 覆蓋+具名事件+fork/join 扇出。
   在 interp.rs(3323 行)原地改造=把 58/58 e2e 的地基掀了重澆。
3. **帳務風險**:兩產品線共用一個多義引擎,每次語義修復都要問「這改的是誰的
   spec」——分歧簿會爆炸。
- **C 的遺產仍然成立**:失敗的是「引擎復用」,不是全部——可復用面=§3.1。

### 3.1 共享 vs 新建(crate 級清單,推薦案)

| 資產 | 處置 | 依據 |
|---|---|---|
| `vl-syntax`(pp/lexer/parser/AST) | **共享**,加 dialect profile | 手寫前端本就分層;iverilog 差異=關鍵字集(-g 階梯)+icarus-misc 開關(`~&` 實證)+SV 階梯;**禁 fork 第二 parser**(兩份前端=漂移面,REFLECTION §14.1 反面教材) |
| `vl-sim::value`(Bits 4-state,IEEE §5 表) | **提取為新 crate `vl-values`**(零依賴),vl-sim 與 riv-kernel 共用 | ADR-0004 vl-vcd 同款「提取+零漂移 e2e 證明」配方;值域軌本就 oracle-獨立(單測) |
| `vl-vcd`(writer/reader/SVG) | **共享**:riverilog 的 dump 走 vl-vcd writer;oracle VCD 對拍走 reader 正規化 | ADR-0004/0005 直接延伸 |
| `rcs-diag` | 共享 | 診斷分層本就通用 |
| `vl-spec`(ADR-0007) | 共享模式:LockstepSpec 泛化為可指向雙引擎(§9.2);R0 不動,R10 再議 | 避免提前抽象 |
| vl-sim 調度/求值/展開 | **不復用**(選項 C 裁決) | — |
| rVPI 設計(ADR-0003) | **先例移轉非代碼複用**:riv-kernel::vpi 照 D1-D5 形狀重做(句柄=凍結名表、回調存 Sim、take-drain) | 觸發點拓撲不同(分層區域 vs 槽邊界),直接搬會錯 |
| difftest/fuzz 慣例 | 模式複製:corpus 錄製/精確數閘/watchdog fuzz | PLAN §7 + REFLECTION §10 |
| 新 crate | `riv-kernel`(事件內核+執行緒模型+vpi 模組)/`riv-elab`(AST→設計裝配:例化/解析/net 構造)/`riv-cli`(bin riverilog+rivvp)/`riv-difftest`(dev-only) | 命名=riverilog 專屬面;共享葉維持 vl-/rcs- 名 |

### 3.2 推薦案總結
**B+(推薦)**:`riverilog`(編譯:vl-syntax[profile]→riv-elab→設計 IR)+ `rivvp`
(執行:riv-kernel 直譯)兩個 CLI 面,共享葉子(§3.1);R6 起序列化為自有
`out.riv` 打通兩段式與冪等。A 的 CLI 形狀全部保留(-o/-t/-g/-E/-M/-c/-y/-s/
-P/-T/-W…;vvp:-n/-N/-q/-l/-M/-m/+plusargs/IVERILOG_DUMPER),**檔案格式主權自有**。

## 4. 語義契約:riverilog 的 spec=「match iverilog 13.0」

差異化語義清單(=rverilator 沒有/不同的;全部要 oracle 對拍錄製):
1. **X 傳播**:算術整-x(與 IEEE §5.1.4 同)、逐位 x/z 表、條件 x→else、
   casez/casex。riverilog e2e 語料**不禁 X**(與 rverilator X-clean 政策相反
   ——兩引擎都 4-state,byte-diff 直接可用)。
2. **分層事件隊列**:active/inactive(#0)/NBA/monitor(+未來 PLI 區);
   nba.v 探針為釘死 fixture。
3. **#delay 全語義**:連續賦值延遲、閘三元延遲(rise,fall,turn-off)、
   min:typ:max(-T 選擇+`:ivl_delay_selection`)、intra-assignment
   `a = #5 v`(阻塞延遲)與 `a <= #5 v`。
4. **force/release/procedural assign-deassign**(§0.1 實證在案)。
5. **fork/join/join_any/join_none/disable**(SV 階梯)、**wait**、**named events**
   (->ev/@ev)、`@(*)`/`@(a or b)`。
6. **$monitor/$strobe**(槽末語義,源序 vs 輸出序實證)、$display 家族格式契約
   (含無 timescale 時 "(1s)" 時間單位渲染)、**$finish 訊息帶 file:line**
   (rcs-diag span 全程攜帶)、$stop 四行橫幅+EOF 續跑、$info/$warning/$error
   前綴+Time/Scope 續行(且不改 exit)。
7. **VPI-first**:系統任務=vpi 模組(§0.1 結構事實)→ riverilog 的內建系統任務
   實作為 riv-kernel::vpi 註冊的 Rust 模組(與 rVPI 同構),**VPI 不是外掛是骨架**。
8. **sorry 語義**:上游不支援處(Net delays、always_ff 常量選擇)同位輸出
   `sorry:` 形訊息——「上游的邊界」是相容面的一部分。

## 5. CLI 契約(釘死面;全部 §0.1 實證)

- `riverilog` 接受 iverilog 的旗標面:`-E -g<level> -D -I -M[mode=] -c/-f -o -s -t
  -T -y -Y -l -m -P -W -B -N -S -u -i -pflag=value`;`-t` 起步只做 `vvp`(自有
  riv)+`null`(語法檢查),blif/fpga/vhdl/vlog95 目標=**顯式硬錯+指針**(上游角色
  是綜合網表輸出,屬 P2 yosys-equiv 的鄰土,不搶)。
- exit code(對抗審訂正:實測枚舉,非三分類):**0** 綠;**1** 用法錯(未知旗標、
  -h)**與 sorry 類**(`wire #6` net delay 實測 exit 1);**2** 語法/elab 錯;**4**
  -g 階梯拒收(join_any 實測);**255** `-t` 未知目標("Code generator failure",
  實測)。riverilog 逐類對拍復刻;`-t` 未知目標的 exit 碼對位在 §12 增補裁決。
  `rivvp`:$finish=0、-N+($stop)=1、-n+($stop)=0;-h=0(不對稱照抄)。
- `rivvp` 旗標面+延伸參數:+plusargs(僅 `+` 前綴進 $plusargs)、-vcd/-fst
  (fst=硬錯+指針)、IVERILOG_DUMPER env。
- 診斷到 stderr、模擬輸出到 stdout、-l 鏡像、-q 抑制(全實證)。
- **已知不復刻的怪癖**:`-o -` 建名叫 `-` 的檔案(rivD4:改為硬錯,footgun
  不遺傳);其餘可觀測行為以復刻為預設。

## 6. 語料與差分策略

- 語料樹:`corpus/iv-e2e/<case>/{*.v, expected.stdout[, expect.vcd]}`,
  錄製=`riv-difftest --record`(oracle=iverilog;錄製時自動帶 -n 防互動態)。
- 比對規則:
  - stdout=**byte-diff**(白名單前綴正規化沿用 vl-difftest 契約);
  - VCD=**vl-vcd reader 樣本點交叉對拍**(vcd_gate 同款),**禁裸 byte-diff**
    ——iverilog VCD header 的 `$version "Icarus Verilog"`/`$date`/`$comment
    Show the parameter values` 為註冊分歧(rivD1),我方 writer 誠實自標識
    `riverilog <ver>`(PLAN §8 #2 同精神)。
- 精確數門檻:`--min-pass 100` 同款(1023 教訓:比例閾藏假綠)。
- 分歧簿預載(動工前登記):

  | # | 領域 | oracle | 我方政策 |
  |---|---|---|---|
  | rivD1 | VCD header(version/date/comment/dumpall 段形) | Icarus 特有 | reader 正規化比對;writer 自標識 |
  | rivD2 | $stop default 的互動行為 | EOF 續跑 | 復刻(橫幅+續跑);互動提示符不出。**測法**(對抗審 P1:標準 e2e 錄製帶 `-n`,覆蓋不到 default 路徑)=另立 `riv_stop_default` 專用 golden(無 -n、stdin=/dev/null,斷言四行橫幅+續跑+exit 0),不入標準 e2e |
  | rivD3 | 效能 | vvp C thread-code | 不入差分;R10 另立 perf gate |
  | rivD4 | `-o -` 建檔名 `-` | 是 | 硬錯(footgun 不遺傳) |
  | rivD5 | sorry 邊界的逐字文案 | "sorry:" 措辭 | 類別+行為對齊,文案自由(同 D8 慣例) |
  | rivD6 | `:ivl_delay_selection`/目標插件機制 | dlopen .tgt/.conf | 單體 Rust;`-t` 名義面相容 |
  | rivD7 | 48_frac_delay 延遲捨入——**預登記後撤回**(2026-10-25 live probe) | 設計期預測「iverilog 按 precision 捨入會印 1500/1750」;iverilog 13.0 復位後實測數據行=`t=2000`/`t=2000`,與 verilator 金樣逐字一致,僅 finish 橫幅時刻(1750)剩差——且該行 e2e 正規化本就丟棄(rivD8) | 無需 ledger,48 現狀 pass(riv e2e 臂實測);rivD7 編號留墓碑不復用;設計 §7「32_proc_order 若分歧=rivD8」順延為 rivD9 |
  | rivD8 | e2e 臂 stdout 正規化:丟 riv 自家工具行(2026-10-25 落地) | e2e 金樣=verilator 錄製,錄製期已被 vl-difftest 丟掉 `- ` 工具行,金樣無 iverilog 形 finish/VCD info 行(riv-r2-design §10 調和注 1:字面「whitelist prefixes only」不可實現) | 丟行白名單=vl-difftest 四前綴 ∪ `^[^: ]+:[0-9]+: $finish called at `/`^VCD info: ` 兩行形,禁內容改寫;敵意 fixture(`-7div2=-3`、`tb.v:6: ` 開頭設計行)由 riv-difftest 單測釘死;**iv-e2e 臂不丟**(iverilog 工具行=金樣本體,01_hello 逐字在案) |
  | rivD9 | **預登記**(R4 弧接 riv_vcd 閘前,登記先行):e2e 既有 oracle-ref.vcd=verilator 錄製 2-state——t0 值域(q=0)與參數傾印(D3 家族)皆與 iverilog 形(q=t0 x、`$comment Show the parameter values`)不同側 | verilator 錄製 refs(51_vcd_counter oracle-ref.vcd 實證) | riv VCD 交叉閘**以 iverilog 腿另錄金樣**(iverilog 13.0 已復位),禁用 2-state refs 直接對拍 4-state 引擎;樣本點交叉=vcd_gate 形,rivD1 header 正規化之上再收斂值域差異 |

## 7. VPI 子集=一等公民(R7;上游差異化核心)

- **移轉 rVPI 先例**(ADR-0003):凍結名表句柄/回調存 kernel+take-drain/
  facade 不向上依賴/put_value=同路徑賦值——形狀照搬,觸發點按分層區域重排
  (ValueChange 排水對齊槽邊界;ReadWriteSynch/ReadOnlySynch 對映到 monitor 區
  前後=rVPI 切片 2 的 iverilog 對位)。
- **內建系統任務=vpi 模組**(上游結構事實,§0.1):$display 族先以 vpi 註冊面
  實作,驗證「VPI-first」不是口號;外部 Rust 模組 `-m`(替代 C .vpi)走同一註冊面。
- **C-ABI .vpi dlopen 橋=cocotb 之門,獨立決策**(§12 #4):vl-ffi C-ABI 先例
  證明技術可行;範圍與時點由使用者裁。
- 驗收:vpi_gate 同款(句柄/回調/值讀寫/children/時間)+ivltests/vpi 87 檔
  挑 8-12 例語料化(GPL 防疫:自寫 TB+錄期望)。

## 8. 里程碑 R0..R10(每項:範圍/語料/閘/粗規模)

> 閘全部接入 `scripts/regress.sh` 的新段 `[riv]`,oracle 存在時
> (`command -v iverilog`)生效,期望常駐 git 使無 oracle 環境也能跑
> (vl-difftest 同款「錄製期才需要 oracle」)。規模欄=kLOC 估算(粗估,非承諾)。

| R | 內容 | 語料/閘 | 驗收(機械) | 規模(估) |
|---|---|---|---|---|
| **R0** ✅2026-10-24 | 立項:PLAN §0 過期行更新;`riv-cli`/`riv-difftest` 骨架;`riverilog -V/-h` 契約面(版本橫幅自標識);corpus/iv-e2e 首批 10 例(hello/mon/nba/force/xprop/fork/dump/plusargs/udp/gates=本文 §0.1 探針全部語料化);oracle 探針證據歸檔 | `riv_cli`(goldens,零 oracle 依賴) | exit code/usage/版本面與 goldens byte-exact;e2e 期望全錄製 | ~0.5k |
| **R1** ◐部分 | 共享葉整備:`vl-values` 提取(零漂移=e2e 58/58+全部套件綠證明);vl-syntax dialect profile(-g1995..2012 關鍵字表+icarus-misc);`-E` 面;**三角閘**:yosys-simple 87 檔+既有 e2e(X-clean 子集)加跑 iverilog 腿 | `riv_pp`(-E 差分)、`tri_gate`(雙 oracle 三角;預期分歧=已登記家族) | -E 輸出與 oracle 正規化比對 ≥95%;三角分歧 100% 可歸因(入分歧簿) | ~1k |
| **R2** ✅2026-10-25 | **事件內核+平層 elaboration**(對抗審 P1:平層裝配原本無里程碑認領):分層隊列(active/inactive/NBA/monitor)、時間軸+timescale/precision(全域 precision 收斂,§0.1 "(1s)" 渲染)、#delay/#0/邊沿/電平事件、執行緒物件(PC+locals);riv-elab 的**平層部分**(net/reg/cont-assign/initial 裝配)在本里程碑認領,層次化例化在 R4 | `riv_kernel`(單測:區域序/真值表)+`riv_e2e` 首批 ≥25 例 | nba.v/mon.v byte-exact;`--min-pass 100` | ~2k |
| **R3** ✅核心 2026-10-25 | 行為語言核心:fork/join(+SV 階梯鈎子)/wait/named event/force/release/assign-deassign/task/function/while-repeat-forever/$display 全家/$monitor/$strobe/$finish 族(含 file:line+單位字串)。**VPI-first 落法**(對抗審 P1:$display 全家在 R3、vpi 註冊面在 R7 的打結):R3 內嵌**最小 vpi 註冊 shim**($display 族經它落地),R7 只補外部模組面(-m)與客戶端語義——§7 的「VPI-first 不是口號」自證在 R3 就完成 | riv_e2e 擴至 60+;`riv_sys`(系統任務 goldens) | §0.1 全部行為 byte-exact;ivltests 行為子集首輪挖掘(10 例入庫) | ~2.5k |
| **R4** | 結構層(2026-10-25 弧計畫修訂=§8.1,對抗審吸收):模組例化(**含 `#(...)` 具名參數覆寫——15/22/29/37/38 五例的硬前置**)、層次引用、**net/連續賦值執行期語義(42 綠例零覆蓋,獨立探針先行)**、閘原語(and..xor 12 種+bufif/notif+三元延遲,**慣性取消窗口探針釘死**)、genvar/generate+unpacked array(收割弧指針實測命中,正式收編)+`$readmemh/b`(R5 前拉,§8.1 偏差帳)、`-y` 語義修正(現誤別名 `-I`,riverilog.rs:91);**強度四級+多驅動仲裁順延 R9**(零語料需求,§8.1 偏差帳) | riv_e2e 逐例翻轉同 commit 改基線;`riv_vcd`(scope 樹 dump+iverilog 側金樣交叉,rivD9);iv 錄製腿+tri_gate(步驟 0);採礦 10 例(GPL 掃描+corpus_lint 閘) | §8.1 逐例終態表(目標 e2e 59/59、iv 9/10、採礦後 19/20);picorv32 點火+yosys-simple 全量腿=**順延下一弧**(§8.1 偏差帳) | ~2k |
| **R5** | IO/傾印系統任務:$dumpfile/$dumpvars/$dumpall/$dumpoff/$dumpon、IVERILOG_DUMPER/-vcd、$readmemh/b(@addr/OOB)、$fopen/$fwrite MCD、plusargs、$random/$time/$realtime 族 | `riv_vcd`(vl-vcd reader 交叉) | oracle VCD 交叉對拍零失配(rivD1 正規化後);dump 語料 8 例 | ~1k |
| **R6** | 序列化+兩進制流:`out.riv`(自有格式:versioned 文字、確定性輸出、含 delay_selection/time_precision 元資料;**格式帶版本+遷移政策,R8/R9 為已知改版點**——對抗審 P1:R6 排在兩大 IR 擴張面之前,凍結必然回改);`rivvp` 完整旗標面(+plusargs/-n/-N/-q/-l/-M/-m/-i)。**R10/XLS 無 out.riv 硬依賴**(XLS 只執行不解析 .vvp,單進程 fallback 可行——out.riv 的依據是 OD2 介面凍結點,不是 R10 的功能閘) | `riv_flow` | 兩次編譯 byte-同;artifact 回放 stdout 與同進程跑 byte-同;XLS 形狀(compile→run)demo | ~1k |
| **R7** | **VPI 一等公民**(§7) | `riv_vpi`(vpi_gate 同款+雙路徑一真相) | ivltests/vpi 挑例語料化 8-12 例;內建系統任務=vpi 註冊面實作的自證 | ~1.5k |
| **R8** | SV 階梯:-g2005-sv(join_any/none/disable、`always @(*)` SV 形)→ -g2009(logic/typedef/always_ff-comb-latch/void 函數)→ -g2012(package/import/enum(+.name())/packed struct/int-byte/常量選擇 sorry 同位) | `riv_sv`(階梯語料;sv-tests chapter-5..9 重錄 iverilog 腿) | §0.1 sv2012/fork 探針 byte-exact;-g 拒收面(2005 拒 join_any exit 4)對拍 | ~2.5k |
| **R9** | 深水區(**誠實標 stretch**):UDP(組合/時序表求值)、specify path delays(-gspecify/-gno-specify 雙態)、`$sdf_annotate`+SDF 注釋器(-sdf-warn/info)、-T min/typ/max、完整強度 | `riv_deep` | UDP/specify 探針語料化;SDF=最後收口,砍線不改前置里程碑 | ~2k+ |
| **R10** | 生態與整合:XLS `VerilogSimulator` 同形包裝(照 iverilog_simulator.cc 形狀);**yosys 閘級網表模擬**(語料=yosys write_verilog 於 picorv32/serv,ISC);wasm 面(ADR-0005 憲法);雙引擎 lockstep 產品面(§9.2);perf gate | `riv_eco`/`riv_wasm`/`cosim_lockstep`;perf=counter8 類 TB ≥1M cycles/s(§0.1 基準的 1/5) | XLS 語料重錄 iverilog/riv 雙腿;wasm 同 goldens;**SV 檔型上游直接 UnimplementedError(study L43),照抄/擴展待裁** | ~1.5k |

**進度註(2026-10-25,R2-R5 設計弧=docs/riv-r2-design.md C1-C6 落地;全部數字=本輪實跑)**:
- **R2 ✅ / R3 ✅核心**:vl-values 提取(C1;零依賴,vl-sim/src/value.rs=shim,e2e 59/59 零漂移)+riv-ir(RIV-1 版本化文字 IR,C2)+riv-elab 平層(C2)+riv-kernel 分層排程(C3)+行為批次(C5/C6:casez/loops/$time+%t/$monitor/$strobe/plusargs/tasks+每啟用幀/fork/force/wait;vl-syntax parser 擴展+vl-sim 錯誤 parse→build 遷移,M7/M8 逐臂保文字,interp.rs:1767-1785)。
- **量尺**:`cargo run -q -p riv-difftest -- --e2e --corpus corpus/e2e` → `SUMMARY e2e=42/59 iv=0/0`(17 個 pending **全部**帶 `milestone R4` 指針:例化 15/16/22/29/37/38/51/54/57/58/59、genvar 28/30/36、unpacked array 52/53/55);`--e2e --corpus corpus/iv-e2e` → `SUMMARY e2e=0/0 iv=8/10`(01-08 enforced 全 byte-exact;09_udp=R9、10_gates=R4 pending)。R4 指針現在**可達且被 17 例實測命中**(riv-elab err.rs 分類面)。
- **R1 ◐**:`-g1995..2012` 全階梯**受理**(riverilog.rs:57,「dialect profiles land at R1/R8」)、`-t vvp`=RIV-1 writer;**未落**=dialect 關鍵字表本體、`-t null`/`-E`(硬錯指針 R1,riverilog.rs:45)、`tri_gate`/`--record-iv-leg`(設計期 iverilog 缺席順延;**2026-10-25 復測 iverilog 13.0 已復位**,可開工)。
- **R3 殘項**:named events(->ev/@ev,零實作零語料;**elab 指針仍寫 R3 但 R3 已 ✅——R4 弧同輪修訂指針歸屬並實作,採礦 family 依賴它**)、`$stop`(expr.rs:1561,同為 R3 指針殘項,2026-10-25 對抗審補登)、VPI-first shim(系統任務=直譯 `Op::SysCall`,riv-ir lib.rs:595,註冊面收斂留 R7)。
- 偏差(out.riv 前拉 R6→R2、最小 dump 前拉 R5→C3、$value$plusargs 前拉 R5→C5、runner 正規化=rivD8、e2e pending ledger 檔機制)=riv-r2-design §9 逐條在帳;48_frac_delay 的 rivD7 預登記已撤回(§6 表,live probe 反證)。

### 8.1 R4 收割弧計畫(2026-10-25 立案;獨立對抗審 15 項全吸收)

> 促級記錄:使用者 2026-10-25 指示收割弧落地後續定下一弧(RK2 逐弧精神:本弧=使用者明示促級第二個 riverilog 弧;M9 tier-B 續停,OD5 不預綁)。本節=下一弧 SSOT;執行序如下,每步的閘與語料同輪。

**步驟 0(先於一切翻轉)=iv 錄製腿+tri_gate 落地**(R1 殘項,非「修 bug」——`--record-iv-leg` 旗標從未實作,對抗審 #7 實證;收割弧 workflow log 的「exit 2」=未知旗標):實作 `riv-difftest --record-iv-leg --corpus corpus/e2e`(iverilog 13.0 已復位)錄 `expected.iv.stdout`/`expected.iv.exit` **入 git**(無 oracle 環境可重放);`tri_gate` 機械三分:iv==vl 而我異=riv bug;iv≠vl=rivD 候選(登記後才準繼續);兩 oracle 一致=硬通貨。**為何最先**:e2e 金樣=verilator 錄製而 riv spec=match iverilog,17 例同時翻轉若無此腿,riv bug 與雙 oracle 分歧將混帳(rivD7 撤回=前車之鑑)。

**R4-a 例化+參數+net 語義**(最大包,先落):模組例化(ANSI 埠全語料在用;**非 ANSI 埠=零語料,補一例探針**——對抗審 #14)、`#(...)` 具名參數覆寫+參數鏈(`$clog2` 已在)、層次名解析、**net/連續賦值執行期語義=獨立探針例先行**(reg→NBA→assign→`@(posedge net)` 序;42 綠例零 `assign` 覆蓋,核心喚醒序/NBA 提交再評價/埠驅父網全部未測——對抗審 #3,59 的最深水區)、`-y` 語義修正(現誤作 `-I` 別名=CLI 契約分歧,改真庫搜尋或帶指針硬錯)。
**R4-b generate**:genvar for/if/case+嵌套作用域(30=**b+c 跨包**,表內標註)。
**R4-c 記憶體+readmem 前拉**:unpacked array 全鏈(riv-ir arrays 段已在)+`$readmemh/b`(start/finish/@addr/註解)——**R5→R4 前拉,偏差入本節帳**;收割弧 commit 0df3bf1 訊息稱 kernel 已有 readmem **為過度宣稱**(elab expr.rs:1565 實為 R5 硬錯,對抗審 #1 抓正),本項補真實作。
**R4-d 閘+延遲**:12 原語+bufif/notif+(rise,fall,turn-off)三元;**慣性取消窗口探針**(iv-10 現金樣 t3/t13 兩點在 transport/inertial 兩語義下同輸出=金樣不釘死語義,對抗審 #9:補 `#7` 窗內取樣例)+**bufif/notif z 驅動觀測例**(54 現例 en=0 後未印);三元延遲無語料錨=僅在補錄 oracle 探針後宣稱,否則順延 R9。
**R4-e 殘面**:named events+`$stop` 實作(R3 殘項歸位,採礦 family 依賴)+`-P/-Y/-Y` 面指針審;RIV-3 版本 bump+writer→reader→writer roundtrip 測同 commit。

**衛生項**:①riv_vcd 閘=scope 樹 DumpSink(現 flat 名,層次後 51 雙 `q` 撞名=退化 VCD,對抗審 #4)+**iverilog 腿錄 VCD 金樣**(rivD9:2-state refs 錯側)+樣本點交叉(vcd_gate 形);②採礦 10 例(X 傳播角落/delay 角落/force-release/named events)全 enforced 入庫,GPL 相似度掃描工件+corpus_lint R1-R3 同輪(RK3);③perf 探針在 R4-a **之後**(層次改熱路徑,先行數字無預測力——對抗審 #15):counter8 類 TB 測 cyc/s 記 REGISTRY;**<200k cycles/s=RK5 停線重評條款生效,非僅記數**。

**逐例終態表(驗收=機械可推導;每翻轉同 commit 改 regress 基線,殘項必須在本表留名,禁無出處數字——對抗審 #6)**:

| case | 需求 | 終態 |
|---|---|---|
| 15_params | 例化+參數覆寫 | pass |
| 16_hierarchy3 | 例化+層次名 | pass |
| 22_counter_nba | 例化+param+cont-assign | pass |
| 28/36 | genvar(36=嵌套) | pass |
| 29/37 | 例化+param+gen-if/case | pass |
| 30_gen_mux | genvar+array+cont-assign 變址讀(b+c 跨包) | pass |
| 38_param_chain | 例化+param 鏈 | pass |
| 51_vcd_counter | 例化+scope 樹 dump+iverilog VCD 金樣 | pass(+riv_vcd 閘) |
| 52/55 | unpacked array(55=NBA 交互) | pass |
| 53_readmem | array+readmemh/b(R5 前拉) | pass |
| 54_gates | 閘原語+z 驅動觀測例 | pass |
| 57/58 | 例化+cont-assign | pass |
| 59_i2c_master | 例化+net 語義全深度(探針先行) | pass |
| iv-09_udp | R9 | pending(指針不變) |
| iv-10_gates | 閘延遲+慣性窗口探針 | pass |
| 採礦 10 例 | — | enforced pass |

→ 目標基線:**e2e 59/59、iv 9/10(採礦後 19/20)**;UDP 為唯一具名殘項。

**偏差帳(本弧明示重塑 §8 R4 範圍,對抗審 #10)**:強度四級+多驅動仲裁順延 R9(零語料需求);picorv32 行為 TB 點火順延下弧(R4-c 落地後可行,本弧保界);yosys-simple 全量 e2e 腿順延下弧;generate/unpacked array 正式收編 R4(收割弧指針已實測命中);`$readmemh/b` R5→R4 前拉。

排序內依賴:R1→R2 硬依賴(vl-values);R6 依賴 R2-R5 的設計表示凍結度;
R7 依賴 R2(區域拓撲);R8 依賴 R3;R9/R10 可與 rverilator M8+ 交錯。

## 9. 未來性(FUTURE;每項給機制,不給願景)

### 9.1 cocotb 生態接入(VPI 之門)
cocotb(BSD-3)=GPI 載入模擬器 VPI 動態庫。接入三路:①**C-ABI VPI 橋**
(riv-kernel 之上蓋 vpi_user.h 形狀的 dylib,vl-ffi 先例;技術可行、範圍重
=§12 #4 使用者裁);②`riv-cocotb` 啟動器(SIM=icarus 形 flow 包 riverilog/rivvp,
不冒名);③上游貢獻 simulator profile。套件內 cocotb rm Makefile.verilator 的
觀測(§0)顯示 icarus 路徑是 cocotb 在 oss-cad-suite 的正字——riverilog 對位
精準,但**不預支承諾**。

### 9.2 雙引擎 co-sim=本 repo 獨有產品
riverilog↔rverilator 同進程 lockstep(vl-spec LockstepSpec 泛化:Sim trait 化,
黃金模型一魚兩吃)。產品形態:①X-sensitive 地圖(2-state 腿與 4-state 腿分歧處
=復位覆蓋洞);②「兩模擬器一真相」回歸(語料同軌雙跑,任何單邊回歸即抓);
③ADR-0007 spec 層的可達性擴大(golden 對雙引擎分別鎖步=spec 本身的雙源驗證)。
閘=`cosim_lockstep`(R10),種子語料=既有 corpus/e2e。

### 9.3 Formal 定位(SymbiYosys)
riverilog **不在** SBTW 主鏈(solver 是 P5 的事),定位=**反例回放器**:sby 產出
VCD/trace → riverilog 重放斷言現場(4-state 語義比 2-state 更接近反例本源);
外加 yosys 等價檢查流程的 witness 驗證腿。誠實標註:間接角色,不構成 formal
里程碑。

### 9.4 FPGA 工具鏈位置(yosys+nextpnr)
後綜合模擬=yosys synth → write_verilog → riverilog 跑(閘級+延遲是 iverilog
的舒適區,rverilator 的綜合子集語義反而不合適)。語料=picorv32/serv 的
synth 輸出(全 ISC,PLAN §7.5 已列 M7 標的)。這讓 riverilog 成為 P4 FPGA 線的
內建驗證腿——**P3 與 P4 互為需求**。

### 9.5 SV 成長路徑與 slang
slang(MIT)授權乾淨=**架構研究**合法(層次化 elaboration/AST 設計可學,零代碼
依賴——它是 C++,本 repo Rust-only)。決策點:若 R8 後 SV 需求(classes/
interfaces/DPI)超出 vl-syntax 共享前端可承載,**新前端 vs 共享擴展**另立 ADR;
10 年信號(§9.8)預留了前端可替換性(dialect profile 邊界)。

### 9.6 教育與 agentic web(ADR-0005 憲法延伸)
wasm riverilog playground:雙引擎並排跑同一 TB——「為什麼 Verilator 是 2-state、
iverilog 是 4-state」變成可點擊的對照實驗(教育面);vl-ffi manifest+vl-vcd
reader+SVG render 已構成 agent 波形 API,riverilog 加入後 agent 可選
「參考語義」引擎(4-state/X 誠實)。LLM/EDA agent 流沿用 ADR-0007:
golden model 對 riverilog 鎖步=意圖驗證的 4-state 腿。

### 9.7 混合語言/VHDL=顯式出範圍+復訪條件
本地安裝內 vhdlpp/vhdl.tgt/vhdl_sys.vpi 在場=上游自己也是局部支援。
出範圍;復訪條件(both must hold):①repo 內已有 Rust VHDL 前端(GHDL-equiv
屆時裁);②使用者明示需求。永不因「上游有目標」而開工。

### 9.8 十年架構信號(現在**不可焙進**的東西)
1. **執行緒表示必須可序列化**(顯式 PC+locals,禁依賴 Rust call stack 生存)——
   為未來 tier-B 編譯與快照/回放留門。
2. **riv-kernel 對 vl-sim 的依賴=零**(只共享 vl-values/vl-vcd/vl-syntax)——
   語義契約隔離是雙引擎共存的前提。
3. **oracle 版本 profile 化**:13.0 行為差異(如 sorry 清單)不寫死在代碼,
   掛 `-g`/版本探測層——oracle 升級時只有 profile 要動。
4. **VPI 永遠是 facade**(掛在 kernel 鉤子上,不織進調度器)——C-ABI 橋與
   分散式 future 保持 additive。
5. **編譯/run 邊界乾淨**(riv 物件格式)——分布式/重放/沙箱執行不需要重構。
6. **波形永遠 VCD→vl-vcd→SVG**(ADR-0005),引擎不長 GUI 職責。
7. **不冒名**:永遠不自稱 iverilog——生態對接靠流程相容+自標識。

## 10. 風險登記簿(kill criteria 具體化)

| # | 風險 | 緩解 | Kill criterion(kill=停線重評,非棄案) |
|---|---|---|---|
| RK1 | 雙模擬器維護負擔 | 共享葉最大化(§3.1);引擎零互相依賴 | 一季內共享 crate(vl-syntax/vl-values/vl-vcd)變更中 >30% 需要雙引擎分叉內部 → fork 前端/重評共享層,ADR 化 |
| RK2 | 與 rverilator M8/M9/rVPI-slice2 搶工 | 預設排序=rverilator 先(PLAN §1 P3 本就第三);riverilog R0 例外(純增量);**R1 的 vl-values 提取=零漂移重構,非零衝突——與 M8 串行落地(先落或約定窗口)**,其餘純增量 | 任何一週兩線爭同一資源 → rverilator 贏,riverilog 順延;連續兩月順延 → 回使用者重排 |
| RK3 | GPL 污染 | §2 鐵則+語料入庫前掃描(與 ivltests/iverilog 樹 diff 報相似度) | 任何 GPL 原文入庫 → 移除+全量審計+incident 五欄 |
| RK4 | 語義分歧帳務爆炸 | rivD 前綴獨立簿;正規化契約測試釘死(normalization_contract 先例) | 未登記分歧在 e2e 紅燈中占比 >10% → 停新語料,先收斂登記面 |
| RK5 | 直譯器效能 | §0.1 基準錨定;R4 後一次優化輪(時間輪/事件池) | counter8 類 TB <200k cycles/s(>25× 慢於 vvp)且優化輪無效 → 重評執行緒模型,ADR 化 |
| RK6 | 分層隊列正確性(最深水區) | 區域序單測+oracle 探針 fixture(nba.v 釘死);monitor 區語義以 $strobe 序釘死 | 對拍反覆無法收斂的區域序案例 ≥3 → 凍結該族語料,列 known-divergence,不硬 byte-diff |
| RK7 | .vvp 相容被要求(若使用者翻案 §12 #1) | 格式研究一週 spike | spike 內無法穩定解析跨版本樣本 → 回退自有格式決策 |
| RK8 | cocotb/C-ABI 橋範圍陷阱 | 預設不開工;獨立 ADR+獨立里程碑 | 橋開發佔用 >2 週未通第一個 cocotb smoke → 降級為「Rust VPI 模組面 only」 |
| RK9 | 上游 13.x 行為漂移(oracle 升級) | 期望重錄紀律(AGENTS §1 末條);版本 profile | oracle 升級導致 >20% 期望變更 → 升級即語義事件,逐例讀 diff 後才收 |
| RK10 | 平台漂移(本 repo 無 CI,goldens 全錄本機 macOS) | 正規化白名單做平台核查;換平台重錄=登記分歧 | 換平台重錄出現 >5% goldens 變更 → 平台差異即分歧事件,逐例登記 |

## 11. 排序建議(vs 現行 roadmap)

1. **本企劃書落地即 R0 的文檔部分**(PLAN §0 過期行更新+pointer 帳,§13)。
2. **R0(契約釘死)可立即動工**:零語義風險、純增量,且把「iverilog=第二
   oracle」變成現實——rverilator 線本身受益(REFLECTION §7 收口)。
3. **R1(vl-values 提取+dialect)與 M8 並行可行**:不碰 vl-sim 語義,提取的
   零漂移證明反而加固 M8 期回歸。
4. **R2+(事件內核)排 M9 之後**(PLAN §1 P3 原序),除非使用者明示提級。
5. 理由一句話:rverilator 是「已在飛的 Phase 1」,riverilog 是「已立契約的
   Phase 2 首件」——順序紀律是兩線都不翻車的前提。

## 12. Open decisions(使用者裁決;動 R2 前必須定)

| # | 問題 | 建議 | 備選 |
|---|---|---|---|
| OD1 | .vvp 檔案格式相容 | **不相容**;自有 `out.riv`,流程相容 | 位元組相容(RK7 一週 spike 後再議) |
| OD2 | 單一 vs 兩個二進制 | **兩個**(riverilog+rivvp):生態契約是兩段式;riv 物件是介面凍結點 | 單一 `riverilog` 子命令式(較晚再拆) |
| OD3 | SV 雄心水準 | -g2012 子集階梯到 R8 為止;classes/interfaces/constraints/DPI 不進本計畫 | interfaces 隨 M10(rverilator)聯動決策 |
| OD4 | cocotb 優先序 | R10 後評估;先 Rust-native `-m` VPI 模組面 | 提前開 C-ABI 橋(RK8 條款) |
| OD5 | riverilog 是否吃 rverilator tier-B lowering | 不預綁;M9 ADR-0002 後以「§9.8-1 執行緒可序列化」為門評估 | 早期綁定(不建議) |
| OD6 | 命名定案 riverilog/rivvp | 是 | 使用者另定 |
| OD7 | `-o -` 怪癖 | 硬錯(rivD4) | 照抄 oracle(不建議) |

## 13. 帳務:本計畫落地時需同輪更新 pointer 的文檔

| 文檔 | 動作 |
|---|---|
| PLAN.md §0 | 「iverilog 未裝」行更新為 13.0 已裝(oracle #2);§1 P3 行加指向本檔 |
| docs/REGISTRY.md | riv-* crates/閘/rivD 分歧簿骨架行(R0 落地時;表只放關係不放瞬時值) |
| docs/OKR.md | R0 開工時立 Objective「O-riv:iverilog 等價模擬器」+ KR 指針(riv_e2e 精確數/riv_cli/riv_vcd) |
| AGENTS.md | §1 oracle 實例清單加 iverilog 13.0;GPL 段補「iverilog 主樹 ivtest/=GPL 唯讀」行 |
| scripts/regress.sh | 新增 `command -v iverilog` 守衛塊(家法同 L33 的 verilator 塊):riv_cli/riv_e2e/riv_vcd/…逐里程碑接入(段頭照現行 == [N/4] 慣例,無方括號段名) |
| docs/REFLECTION-agent-hardening.md | §7「single-oracle risk」項加收口註記(指向本檔 §1.2-1) |
| .agents/skills/eda-dev/SKILL.md | 「雙 oracle 紀律」條(iverilog 期望錄製要帶 -n;GPL 語料鐵則) |
| README.md | Status 表 R0 後加 riverilog 行;順帶修 Status 表過期的 55/55(實際 58) |
