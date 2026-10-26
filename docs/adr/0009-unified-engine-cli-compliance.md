# ADR-0009:單一統一模擬引擎,CLI 合規雙面(rverilator/riverilog)

> 2026-10-25。狀態:**接受(推翻 RIVERILOG-PLAN §3 選項 C 裁決與 OD2)**。
> 決策者:使用者(「adjust the goal — rverilator and riverilog try to unified
> single engine, but CLI compliance, because I want make rust cad suite to
> unified to Rust ecosystem, make it simple and cross platform」)。
> 脈絡:使用者同時提出對 Verilator 非可綜合面支援的精確化理解(設計定位=
> 可綜合 RTL,XLS 選 iverilog 跑含時序控制的 TB 正因其事件驅動完整)。

## 1. 決策

**一套引擎,兩個 CLI 面。** 模擬核心統一為單一 Rust 引擎(事件驅動、
4-state、完整時序),對外保留兩個合規 CLI 面:

- `rverilator`——Verilator 形 CLI(--binary/--lint-only/--gen-rust 等),
  語義契約=match Verilator(2-state 收斂、即刻 settle,既有 59 例 e2e)。
- `riverilog`/`rivvp`——iverilog 形 CLI(-g 階梯/-E/-o out.riv/plusargs),
  語義契約=match iverilog(分層隊列、#delay、X 傳播)。

方言(dialect)決定語義開關,不決定引擎:**同一核心,按入口方言選擇
排程/求值 profile**(R8-2005 形 vs Verilator 形)。

## 2. 動機(為何推翻)

### 2.1 原裁決的成立條件已消失

RIVERILOG-PLAN §3 選項 C(擴展 vl-sim 到 4-state/delays)被否決的三層
理由,在 R4 收割弧後重新檢驗:

| 原理由 | 現況 |
|---|---|
| 「vl-sim 進程模型=segments+settle 迴圈,iverilog=協同執行緒,原地改造=掀地基」 | 事實仍真——**但方向反轉**:不再改造 vl-sim,而是以 riv-kernel(已完成、經 R4 五弧驗證的 4-state 事件核心,5.7k loc、51 套件綠)為統一核心;vl-sim 的 segments 解釋器退役或降為 tier-B 研究對照 |
| 「一個引擎滿足兩契約=調度器全面方言化,比兩薄引擎共享葉子更貴」 | **部分反駁**:R4-R5 弧證明方言差是「有界開關」而非全面——分層隊列/延遲/X 表是 riv-kernel 固有面,Verilator 形=其上的 profile(2-state 投影+settle 收斂+無延遲快徑),不是第二實作 |
| 「兩產品線共用多義引擎,語義修復要問『改的是誰的 spec』,分歧簿爆炸」 | 認知反轉:**這正是要的**——SV-UNIFY 已裁決單一前端雙引擎;引擎合一後「改誰的 spec」有單一答案(方言 profile+分歧簿 rivD/D 各自記),維護面從 2×(兩個引擎各修一次)降為 1×+profile |

### 2.2 使用者目標權重

「統一到 Rust 生態、簡單、跨平台」:
- 一套核心=一份跨平台面(macOS/Linux/Windows/wasm 一個引擎四處編譯),
  而非兩個引擎各自攜平台矩陣;
- 簡單=共享葉(vl-values/vl-vcd/vl-syntax)之外,排程/求值/傾印單份;
- Rust 生態統一=wasm/FFI 面(ADR-0005 憲法)只掛一個引擎。

### 2.3 上游定位差異(使用者提供的精確化)吸收

Verilator=可綜合 RTL 定位、非可綜合支援有限(INITIALDLY 歷史、specify
忽略、TB 構造靠 C++ TB 自補);iverilog=事件驅動完整標準實作。**CLI 合規
複刻的正是這個定位差**——rverilator 面按 Verilator 的接受域收(既有
OUT 硬錯清單),riverilog 面按 iverilog 的接受域收;引擎的完整能力
(4-state/延遲/TB 構造)不因 rverilator 面而膨脹,也不因 riverilog 面
而閹割——接受域在 CLI 層把關,不在引擎層。

## 3. 合流路徑(分弧,零漂移鐵則全程)

| 步 | 內容 | 驗收 |
|---|---|---|
| U-1 | **vl-sim 降為 rverilator 面的方言 profile 適配層**:riv-kernel 升為 `vl-engine`(或 riv-kernel 更名),rverilator --binary 改驅動統一引擎 + Verilator 形 profile(2-state 投影:驅動值 x/z→0、settle 收斂序=宣告序) | corpus/e2e 59/59 byte-exact 零漂移(現有金樣即驗收);counter8/fulladd sim_gate 綠 |
| U-2 | **求值面合流**:vl-sim eval(IEEE §5 表單測軌)與 riv-elab/interp 的 lowered-ops 單一語義源整合——const 求值與運行時共用一份。**✅ 2026-10-25**:新 `vl-ops` crate(adjusted/apply_* 單源),kernel 兩執行點+elab const interp+vl-sim eval_w 四份手寫臂全數收編;riv-ir Op::Bit/BitNot 補 signed 域;副產品=雙 oracle 七探針抓三語義分歧(~ 自決定修復翻案/riv signed-bitwise/riv logic-x abort)全修,REGISTRY incident 10-24 | vl-difftest 全綠+value.rs 單測軌不動(實績:e2e 59/59+regress 全閘 PASS+vl-ops 10 測) |
| U-3 | **CLI 面收編**:rverilator 現有旗標逐字保持(legacy 行為測試釘死);riverilog/rivvp 旗標面照 RIVERILOG-PLAN §5;兩面共用 -E/-I/-D 前端。**◐ 前半 ✅ 2026-10-25**(a83ccb1):共享 -E 前端=render_preprocessed+directive_name 上移 vl-syntax::pp(vl-cli golden byte-equal=零漂移機械證明);riverilog -t null(免寫)/-E(a.out/-o/stdout 路由)/-y fixpoint 庫搜尋(-Y 附加非取代)/-D attached+separate+pp 錯=1、parse=2 分類+rivD4 `-o -` 硬錯兌現;rivvp -N(kernel stop_exit additive)/-q/-l 鏡像+`-l -`=rivD14 硬錯+-M/-m=R7 指針。殘=U-3b(dialect 逐字受理表/icarus-misc/-E oracle 正規化閘/-l - 契約/-i 真實作/-M depfile 長尾/rivD13) | cli_gate+riv_cli goldens 不漂移(實績:cli_gate riv-cli 9→23 測+vl-cli 新 5 測,兩臂 SUMMARY 逐字零漂移 e2e=58/59、iv=21/22) |
| U-4 | **rVPI 收編**:vl-sim::vpi(ADR-0003)掛到統一引擎鉤子;riv 側 VPI(R7 面)同註冊面。**◐ slice-1 ✅ 2026-10-26**:Kernel 四觸發點落地(start=run 入口/vc=store_var_raw 單漏斗 post-settle/after_delay=時間輪推進先於 slot/end=finish_out 單次守衛;全 Option 零成本)+vpi_read/write_var 面+riv-kernel vpi_hooks_gate 2 測(逐字序斷言)。**slice-2 ✅ 2026-10-26**:CbFn embedder 解耦=VpiHost trait(ADR-0003 D5 修訂:registry 單一具體型、註冊面雙面同源;泛型 E 否決)+USim 註冊面(register_cb/remove_cb/vpi_handle_by_name + kernel 四鉤子橋 UsimHost)+FireSet 觸發協議(凍結表留在原地)+kernel after_delay 後 stop_as_finish 檢查(timer finish 短路)。殘=riv 側 VPI(R7 面)同註冊面 | vpi_gate 8 測零漂移+usim_vpi_gate 3 測綠+regress PASS |
| U-5 | **退役 vl-sim segments 解釋器**(interp/eval 不再被任何面引用),wasm/FFI/spec 改掛統一引擎。**✅ 2026-10-26**(slice-1=faces 3ef6191;slice-2=spec 層遷移):vl-spec/spec-i2c 改 USim;kernel 增 stepwise run_until(deadline=絕對調度點,外部驅動時鐘無事件也推進 now;started 標記門一次性行;每次入口播種 Ready——put 跨 run 喚醒的線程需要隊列)+run_until 後 vpi_write_var 按宣告寬截斷(legacy vpi_write_bits 對齊,量得:64-bit put 落 1-bit reg 破壞 if(we))+take_vcd 收尾未關 writer。interp/eval 降為 tier-B 研究對照(difftest RVERILATOR_LEGACY 回退路徑保留),實體刪除隨 tier-B 清理弧 | wasm 8/8+features 2/2+spec PASS+ffi 四閘綠;e2e=58/59+iv=21/22 零漂移 |
| U-6 | tier-B(--gen-rust)重定向:對統一引擎的 IR 做 lowering。**◐ slice-1 ✅ 2026-10-26**:ADR-0002 定案(生成碼=獨立 Rust,語義面全單源:值運算=vl-ops 同函數、$display=riv_kernel::render_pieces 唯一路徑;async 只作宿主語法糖禁真並發)+riv-gen crate+vl-cli --gen-rust 旗標;子集=單 initial 直線線程(4 corpus 案例 round-trip byte-exact vs oracle 金樣:02/10/11/45;EndThread 收編為 no-op),子集外=M9 硬錯逐字釘死;delay/NBA 對照表=ADR-0002 §4 | roundtrip 闸(4 案例 byte-exact+硬錯指針)+cli_gate gen_rust+regress PASS。**slice-2 同日**:多進程=嵌入式排程器(線程狀態機,掛起僅 Delay;run_all=ADR-0001 序),覆蓋 4→16 案例 byte-exact(含 32_proc_order/49_multi_initial) | roundtrip 闸 16 案例+cli_gate gen_rust+regress PASS。**slice-3 同日**:事件驅動(邊沿 always 靜態敏感+@(edge) 掛起;edge_fire_pub 單源;Rearm=pc=0 重置、WaitEdge 推進 st——兩刀實錄=線程死亡/永久重停)+roundtrip 每例 10s watchdog(鐵則 4),覆蓋 16→18(23_nba_vs_blocking/25_edge_or byte-exact) | roundtrip 闸 18 案例+regress PASS |

**順序紀律**:U-1 是唯一有漂移風險的步(金樣已在 git=可逐例對帳);每步
regress 全綠才進下一步;任何一步不可達成即停線重評(本 ADR 回退路徑=
兩引擎現狀,損失僅合流投入)。

## 4. 後果

- **正面**:單一引擎一份平台矩陣;SV-UNIFY 的語料/功能一次落雙 CLI 面
  (SV-2 即時斷言等不再做兩遍);XLS/cocotb 生態對接(§9.1/9.2)單點;
  雙引擎 lockstep(§9.2)從「兩進程」簡化為「同引擎雙方言 profile 對照」;
  維護 1×;wasm 面單掛。
- **負面/風險**:riverilog 面的效能受統一核心的通用性拖累(R10 目標 1M
  cycles/s 須在統一核上重驗;575k 現值是 riv-kernel 單獨的數字);Verilator
  形 profile 若與 vl-sim 既有行為有不可調和差 → 該例登記分歧(金樣仲裁);
  out.riv 格式(OD2 翻案面)降級為 riverilog CLI 的內部細節,不再是介面
  凍結點。
- **中性**:riv-ir 保留(riverilog 面序列化+兩段式流程契約不變);兩份
  e2e 語料與全部閘不動(它們本來就是 CLI 面的驗收)。

## 5. 與既有裁決的關係

- 推翻:RIVERILOG-PLAN §3 選項 C 裁決(§3.1 表 vl-sim 行「不復用」)、
  OD2(兩二進制→CLI 面合規但引擎單一;`riverilog`+`rivvp` 二進制保留,
  但只是 CLI 面,非兩套運行時)。
- 不變:ADR-0001(宣告序)、ADR-0003(rVPI,U-4 收編)、ADR-0004/0005
  (vl-vcd/SVG 憲法)、OD1(.vvp 不相容,自有 out.riv 保留)、雙 oracle
  方法論(AGENTS §1 全文)、SV-UNIFY(單一前端,引擎面隨本 ADR 更簡)。
- PLAN §1 路線:P3.5 SV-UNIFY 之後、M9 tier-B 之前插入 U-1..U-6(統一弧),
  M9 的 tier-B 對象隨 U-6 更新。
