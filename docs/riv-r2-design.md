# riverilog R2-R5 設計書 — 讓 riverilog+rivvp 真的編譯並模擬真 TB

> 2026-10-24,Engine architect 出案。上遊正典:`docs/RIVERILOG-PLAN.md`(§3.1
> 共享葉、§8 R1-R5、§9.8 十年信號)、`AGENTS.md`、
> `.agents/skills/eda-dev/SKILL.md`。本文是 R2 弧的實作設計;裁決項與
> PLAN 的偏差在 §9 逐條登帳。驗收=兩個內建語料:
> `corpus/e2e`(59 例,verilator 錄製)與 `corpus/iv-e2e`(10 例 case.json,
> enforced/pending)。

## 0. 本 session 實證塊(全部本機實跑;引用他人結論一律另註)

| 檢查 | 命令 | 結果 |
|---|---|---|
| chase-ledger 現狀 | `cargo run -q -p riv-difftest` | `== riverilog e2e: enforced 0/0, pending chase targets 10 (expectations recorded) ==`,**exit 0**(單獨跑;複合命令裡的 exit 1 來自最後的 `command -v`) |
| riv-cli goldens | `cargo test -q -p riv-cli` | `test result: ok. 8 passed; 0 failed` |
| 語料規模 | `ls -d corpus/e2e/*/ | wc -l` / `.../iv-e2e/*/` | 59 / 10 |
| **oracle 缺席** | `command -v iverilog; command -v vvp; ls /opt/homebrew/bin/iverilog` | **rc=1 / 不存在** — PLAN §0「iverilog 13.0 已安裝」在本機**現在不成立**(R0 當時成立,afc9ade 提交記錄在案)。錄製腿全部順延;既有 committed 期望不受影響 |
| vl-values 可提取性 | 讀 `crates/vl-sim/src/value.rs`(1-36, 178-186, 2016-2030, 2520-2560)+ INVENTORY 普查 | `Bits{width,repr}` 兩臂(value.rs:10-23);`from_numlit` 唯一 vl-syntax 繫點(:2020);limb 引擎 pub(crate) 註解 :182 已證實 stale;`n_w_padding_property` :2526 在案 |
| parser 現狀 | 讀 `crates/vl-syntax/src/parser/stmt.rs:50-67` | `wait`/`fork` 是**parse 級**里程碑錯 **`(milestone M8)`**(:50-57);`force`/`release`/`deassign` 是 **`(milestone M7)`**(:58-62);procedural `assign` 另一臂 **`(milestone M7)`**(:63-67);Kw 表已有全部關鍵字。**注意 M8/M7 分屬兩族、M7 族內三個錯誤臂**——C6 遷移時文字逐臂保留 |
| AST 面 | 讀 `crates/vl-syntax/src/ast.rs:132-163, 268-390, 392-452` | `ModuleItem` 含 GenerateRegion/GenIf/GenFor/GenCase;`Stmt` 含 DelayCtrl/EventCtrl/Case(3 種)/For/While/Repeat/Forever/SubCall/Disable;**無** Fork/Force/Wait/Deassign 變體;Expr 含 Concat/Repeat/HierDot/Index/PartSelect/Ternary |
| 兩模組案例 | 讀 `corpus/e2e/22_counter_nba/tb.v`、`51_vcd_counter/tb.v` | 兩例都是 `module counter #(parameter W = 8)(… output reg [W-1:0] q)` + tb 經 `counter #(.W(8)) dut (.clk…)` 例化 → **非平層**,Phase A 會在 elab 硬錯 → 歸 C7(審稿發現,已核實) |
| 多檔案例 | `ls corpus/e2e/57_xls_add3/ 58_xls_acc_pipe/ 59_i2c_master/` | 57=add3.v+tb.v、58=acc_pipe.v+tb.v、59=i2c_master.v+tb.v(雙源檔);53_readmem 帶 data.hex/data.bin/data2.hex |
| display 缺口 | 讀 `corpus/e2e/11_display_formats/{tb.v,expected.stdout}`、`45_display_esc/{tb.v,expected.stdout}` | 11:`%c%c %s 100%%`(`"ok"` 字串字面量)+`$write("no")$write("newline")$display("")`→一行 `nonewline`;45:`"100%% \\done/"`→`100% \done/`(`\\` 轉義)。→ §5-4 補 $write/%s/`\\` |
| 44/27 歸位 | 讀 `corpus/e2e/44_lfsr/tb.v`、`27_shift_reg/tb.v` | 44=單模組平層(repeat @(posedge)+NBA 移位),27=平層+dump → 兩例歸 C3 排程行 |
| no-timescale 預設衝突 | 讀 `crates/vl-sim/src/interp.rs:3155-3166`;`PLAN §0.2`;本 session 複跑探針 `rverilator --binary`(無 timescale TB→`10 10`;`1ns/1ps` TB→`10000 10`) | 我方 rverilator 寫死 `1ps/1ps`(interp.rs:3165;探針實證預設 unit==precision,但 `#10`→`%0t`=10 對 1ps/1ps 與 1s/1s 同值——**探針不解歧絕對值**);PLAN §0.2 記 iverilog VCD 對無 timescale 源印 `1s`——**衝突且 iverilog 側今日不可證**(oracle 缺席)。全 72 個語料源檔(72/72,迴圈 grep `timescale` 實數)都帶 timescale → 僅潛在;riverilog 預設=oracle 重裝後實測釘死,先按「未證=不釘」處理(§4) |
| e2e 金樣的分歧數學 | 讀 `corpus/e2e/48_frac_delay/{tb.v,expected.stdout}` | `#1.5`→golden `t=2000`、`#0.25`→`t=2000`(+0):**verilator 把延遲捨入到 timescale 單位**(1.5ns→2ns;0.25ns→0ns)。iverilog 按 precision 捨入會印 1500/1750 → **真分歧**,預登記 ledger(§7) |
| e2e 金樣 4-state 顯示 | 讀 `corpus/e2e/10_literals/expected.stdout` | `'hx=xxxxxxxx`、`zlit=1z` — verilator 金樣本身含 4-state 顯示,iverilog 按 IEEE 應一致 → 不預登記,build 時驗證 |
| VCD writer 身份 | 讀 `crates/vl-vcd/src/writer.rs:85` | `$version\n  rverilator (rust-cad-suite)` **硬編碼** — riverilog dump 需要可參數化(預設不變保零漂移) |
| iv 金樣形狀 | 讀 `corpus/iv-e2e/{01_hello,03_nba,02_monitor,07_dump,10_gates}/expected.stdout` | `$finish` 行=`tb.v:6: $finish called at 0 (1ps)`(裸 stdout);07_dump 有 `VCD info: dumpfile dump.vcd opened for output.` |
| 新檔檢查 | `find corpus -name 'expected.iverilog-stdout.txt' | wc -l` | 0(錄製腿檔案尚不存在,設計只允許新建) |
| e2e 無 case.json | `find corpus/e2e -name case.json | wc -l` | 0(runner 不得要求) |

**runner 進場前的兩個 harness 事實**(讀 `crates/riv-difftest/src/main.rs`):
enforced 臂今日只跑 compile 不跑 rivvp(main.rs:162-190,因為無東西可載);
DRIFT 是 print-only(main.rs:201-215,exit 恆 0;regress.sh:51-53 只 grep 尾行
形狀)。本設計的 runner 重寫(§6)在保留 exit-0 契約的同時把 DRIFT/UNLEDGERED
變成可 grep 的文字訊號。

## 1. 總架圖(裁決後)

```
                vl-syntax (共享, 不 fork;R1 dialect profile 補 -g 階梯)
                rcs-diag
                   │
  riverilog bin ───┴─── riv-elab ──(AST+常量求值→)── riv-ir (RIV-1 程式)
  (pp→parse→elab→write out.riv)                          │
                                                         ▼
  rivvp bin ──────────────────────────────────── riv-kernel (直譯+分層排程)
  (load RIV-1 → run)                                 │
                                              vl-values (提取自 vl-sim::value)
                                              vl-vcd (dump 寫出經 DumpSink)
```

依賴箭頭(零 **外部** dep 的定義=PLAN §2-4「crates.io 零依賴」;workspace 內
共享葉依 PLAN §3.1 白名單):

| crate | deps(全 workspace 內) | 鐵則來源 |
|---|---|---|
| `vl-values`(新,提取) | **無**(純 std) | PLAN §3.1;AGENTS 10.8 |
| `riv-ir`(新) | **無**(純 std;字面量以 (width,signed,base,digits) 攜帶,載入期才變 Bits) | §9.8-1/5 |
| `riv-kernel`(新) | `vl-values` + `riv-ir` | §9.8-2「對 vl-sim 依賴=零」;不碰 vl-syntax/rcs-diag(編譯期已把 file:line 烘進常量欄) |
| `riv-elab`(新) | `vl-syntax` + `rcs-diag` + `riv-ir` + `vl-values` | 共享葉白名單 |
| `riv-cli`(既有) | `riv-elab` + `riv-kernel` + `riv-ir` + `vl-syntax` + `rcs-diag` + `vl-vcd` | 兩 bin 同 crate 既有形 |
| `riv-difftest`(既有,重寫) | 無 lib;子程序驅動兩 bin | runner 契約 |
| `vl-sim` | +`vl-values`(re-export shim) | 提取零漂移 |

**禁**:riv-kernel/riv-elab `use vl_sim`(§9.8-2 語義隔離;RK1 帳務);任何
riv-* crate 出現 crates.io 依賴;第二個 VCD parser/writer(ADR-0004)。

## 2. 裁決 D1 — vl-values 提取契約

**方案:新 crate `crates/vl-values`,std-only,內容 verbatim 搬遷 + 三處手術。**

1. **搬遷面**:`Bits`/`Repr` 兩臂、`MAX_WIDTH`、`ValueError`/`VResult`、全部
   構造/運算/Limbs 引擎/fmt/clog2 與測試模組,從
   `crates/vl-sim/src/value.rs`(2607 行)原樣移入
   `crates/vl-values/src/lib.rs`。visibility 手術:`pub(crate)` → `pub`
   (Limbs/limbs_for/limbs_divmod_small/limbs_mul_add_small — INVENTORY 已證
   workspace 零外部使用者,`:182` 註解同輪改寫為實情)。
2. **vl-syntax 斷臍**:`from_numlit(&NumLit)`(value.rs:2020)是唯一非測試
   vl-syntax 繫點,而 `NumLit` 內嵌 `rcs_diag::Span`(ast.rs:470)。拆成:
   - vl-values 新面:`pub enum Base { Dec, Bin, Oct, Hex }` +
     `pub fn from_numlit_parts(width: Option<u32>, signed: bool, base: Base,
     digits: &str) -> VResult<Bits>`(邏輯逐行同今日,含 §2.5.1 十進 x/z
     整值規則)。
   - vl-sim 保留 `src/value.rs` 為 **shim 模組**:`pub use vl_values::*;` +
     `pub fn from_numlit(n: &NumLit) -> VResult<Bits>`(ast::Base→vl_values::
     Base 映射後轉發)。**全部既有呼叫端零改動**(eval.rs:7
     `use crate::value::{self, Bits, ValueError}`、vpi.rs:17、vl-spec 的
     `vl_sim::value::Bits` 照舊)。
3. **測試分流**:測試模組實數 **18 個 `#[test]`**(`grep -c '#\[test\]'
   crates/vl-sim/src/value.rs` = 18)。`lit()` helper(:2122,包
   `parse_based`)有 **22 個呼叫點、6 個測試**使用(逐名實證:
   wide_u128_values / literals_round_trip / compare_and_case /
   shifts_and_ashr / signed_adjust / wide_literals_and_roundtrip),另有
   直接呼叫 `parse_based`/`from_numlit(&NumLit{..})` 的測試——**凡觸
   NumLit/parse_based 的測試留在 vl-sim**(作 shim 面的 integration
   test);純值測試(含機檢 `n_w_padding_property` :2526、
   `mixed_n_w_crosscheck` :2498)隨遷 vl-values。vl-values 測試模組零
   vl-syntax import。

**零漂移證明(同輪,缺一不算提取完成)**:`cargo build` 零 warning;
`cargo test` 全綠(含 vl-values 新家裡的 property 閘);
`./target/debug/vl-difftest --min-pass 100` 59/59 byte-exact;
`git diff --stat corpus/e2e` 為空。這是 ADR-0004 vl-vcd 提取同款配方。

## 3. 裁決 D2 — 兩進制流程:out.riv 前拉(RIV-1),in-process 否決

**裁決:`riverilog -o out.riv …` → `rivvp out.riv …` 為 R2 起的真流程。**
PLAN §8 把 out.riv 放 R6;runner 契約(「drive the riverilog compile ->
rivvp run flow」)與 §9.8-1(執行緒=顯式 PC+locals、可序列化)共同強制:
threaded code 本來就要按「無 Rust stack 生存」設計,序列化只是把同一 IR
寫成文字——前拉成本≈0,不前拉則 rivvp 對真輸入恆 milestone 錯、R6 變翻新
手術。記為 PLAN 偏差 §9-1;R6 縮小為「格式成熟+rivvp 完整旗標面+兩次編譯
byte-冪等閘」。

**RIV-1 格式(最小、own-format、versioned)**:
- 文字、行導向;首行 `riv 1`;尾行 `riv end`(reader 斷言包夾)。段落序固定:
  `meta`(time_precision、timescale 字串、top 名、源檔清單)、
  `vars`(`var <id> <kind reg|net|array> <width> <signed> <name> <init-digits>?`)、
  `arrays`(維度行)、`assigns`(cont-assign 線程索引)、
  `threads`(`thread <kind initial|always|...>` + op 行)、
  `funcs`(task/function 子程序)、`strs`(display 格式字串池)。
- Op 行式:`<pc> <opcode> <operand…>`,運算元=槽索引(見下)/var id/字面量
  (width,signed,base,digits)/字串池索引。手寫 writer/reader(零 serde;
  riv-difftest 的 manifest 手寫解析先例)。
- **往返閘**:writer→reader→writer 逐 byte 相等單測;RIV-1 從第一天帶版本,
  R3/R4/R5 的 IR 擴張=版本遞增(RIV-2…),R6 才凍結 byte 穩定性閘。

**riv-ir 內容模型**(riv-elab 寫、riv-kernel 讀):
- `Var { id, name, width, signed, kind: Reg|Net|Array{dims}, init }`;
- `Thread { kind, pc0, frame_len, sens: Vec<SensItem>, body: Vec<Op> }`
  (cont-assign 也是 thread:kind=ContAssign,sens=RHS 讀集);
- `Op` 三地址、**寬度烘焙**(見 D4),家族清單附錄 §A;
- 函式/任務=`Func { frame_len, body, automatic: bool }`,呼叫=
  `Call{f, args→frame slots, ret slot}`——**無運算元堆疊**,全部槽定址
  (暫存在 lowering 期配置進 frame),暫停/恢復天然可序列化(§9.8-1)。
  **幀模型=每啟用一幀**:執行緒攜帶 `frames: Vec<Vec<Bits>>`(啟用棧)與
  `ret_pc: Vec<u32>`(返回棧);區域變數定址=(幀深,槽)。
  **static/automatic 語義按 IEEE 1364-2005(預設 static)**:非 automatic
  的 task/function 其局部綁到**每例項一份的共用幀**(載入期分配)——並發
  共用儲存是 IEEE 定義的行為,不是 bug;`automatic` 用啟用棧。靜態幀+
  啟用幀雙軌都在 IR 內表達,**C6 前凍結此模型**(審稿發現:靜態 callee 幀
  無法表達兩活躍執行緒共用呼叫同一 task 的 IEEE static 語義;語料無此形
  ——59 的兩 task 單呼叫序、20 單呼叫者——所以 C6 必附一個
  「兩執行緒並發呼叫 static task=共用儲存」單測,語料抓不到的由單測釘)。

## 4. 裁決 D3 — riv-kernel:分層隊列+時間輪+顯式 PC 執行緒

**值域**:`Bits`(vl-values,4-state 從第一天;60_wide_values 的多 limb 面
由提取白得)。**存儲**:`Vec<Value>` per var(id 定址);array=每字一 slot。

**執行緒物件**:`ThreadState { pc, frame: Vec<Bits>, status }`,status ∈
`Ready(region)` / `Suspended(Resume)` / `Done`;`Resume::Delay(u64 時點) |
Edge(sens) | Level(expr 槽集) | NamedEvent(id) | Join(fork id) |
Forever 等`。初始 always/initial/cont-assign/gate(R4)/monitor 各成 thread;
**thread 序=宣告序**(ADR-0001 慣例;iverilog 不保證——32_proc_order 是
登記中的分歧候選,§7)。

**分層排程(iverilog 契約,PLAN §4-2;nba.v 探針=03_nba 釘死)**:

```
slot(t):
  loop {
    1. Active:FIFO 排空(阻塞賦值即時提交;值變化→喚醒者入 Active 尾)
    2. Inactive:排空(#0 / 顯式 #0 區)
    3. NBA:整批提交 nbassign 排程值 → 記錄變化集 → @-等待者入 Active
    4. Monitor:$strobe 依源序、$monitor(on-change,slot 至多一發,t0 arm)。
       **所有權**:Monitor 區域與其在迴圈中的位置屬 C3(kernel 骨架);
       `$monitor`/`$strobe` 的系統任務語義(t0 arm、on-change 觸發、源序)
       屬 C5 批次——區域先在,syscall 後到,不重複認領(審稿所有權發現)
    5. Active∪Inactive∪NBA 非空 → 續 loop;否則跳出
  }
  時間輪取下一時點;$finish 即刻終止(exit 0)。
```

- **變化通知**:slot 內每個提交步驟產生 `var → (old,new)` 變化集;邊沿等待
  者按 old→new 判 posedge/negedge;電平等待者(@*/wait)重估;cont-assign
  thread 撞醒;monitor arm;dumper(R2 起最小實現)通知。混雜順序的收斂
  語義以 03_nba(層次)+42_nba_handshake(e2e,NBA 取樣)+59_i2c_master
  (wire 鏈)三 fixture 對拍釘死;RK6 條款生效(對拍反覆不收斂 ≥3 →
  凍結該族列 known-divergence,不硬 byte-diff)。
- **時間輪**:`BTreeMap<u64, Vec<SlotEvent>>`,u64=全域 precision 單位;
  timescale 收斂在 elab(多檔 precision 取 min)。**無 timescale 源的預設
  今日不釘**:PLAN §0.2 記 iverilog 印 `1s`,我方 rverilator 寫死
  `1ps/1ps`(interp.rs:3165)——兩證據衝突且 iverilog 側不可證(oracle
  缺席);全部 72 個語料源檔都帶 `timescale`(§0 實數)故僅潛在。riverilog
  預設在 oracle 重裝後以實跑探針釘死,屆時入 rivD(未證前 elab 對無
  timescale 源報「default unmeasured (milestone R2)」硬錯,禁猜)。
  $finish 行渲染=`<file>:<line>: $finish called at <t>
  (<precision-timescale>)`(01_hello 金樣逐字;file:line 在 lowering 期
  烘進 op 常量欄,kernel 不見 span)。
- **閘延遲/net 延遲**(R4):值更新事件入時間輪(延遲濾波);10_gates
  (t3 y=x mid=0,t13 settle)為釘死 fixture。
- **force/release/deassign**(R3):reg 帶 force-overlay 層;release 後
  保持最後強制值(04_force 金樣:released r=1);連續賦值穿透(force r
  時 w 跟隨)。procedural assign/deassign 同層。
- **輸出**:kernel 內建 stdout buffer(顯示家族寫入),rivvp drain 後寫
  stdout;VCD 經 `trait DumpSink` 鉤子,rivvp 接 vl-vcd writer(ADR-0004:
  kernel 自身不產 VCD 文字;writer.rs:85 的 `$version` 硬編碼加
  `program_name` 參數,**預設值不變**保 e2e expect.vcd 金樣零漂移,riverilog
  傳自身名=ivD1 自標識)。
- **kernel 無 watchdog**:runner 以子程序粒度執行(兩 bin 流程),per-case
  看門狗=殺程序(§6);AGENTS §4 的掛死 fixture 責任移到 elab/kernel 的
  恢復迴圈(pos 未動→bump)+ runner 看門狗雙層。

## 5. 裁決 D4 — riv-elab:平層先行,層次/生成 R4

**Phase A(R2,chunks 2-3;單 top 平層)**:
1. top 自動偵測=「未被例化的模組」(與 vl-difftest lib.rs:74-90 同演算法,
   本 crate 重實現;57_xls_add3 的 DUT+tb 形覆蓋)。
2. decl 處理:net/reg/param/localparam;常量表達式用 vl-values 求值
   ($clog2、param 鏈、`localparam [W-1:0]`);初值(Illegal 於 net 上=錯)。
3. **寬度/符號在 lowering 期烘焙**(IEEE §5.4/§5.5):上下文決定運算把
   操作數強制到表達式寬(符號僅整體 signed);自決定子上下文(比較兩側/
   移位量/拼接元素/複製計數/三元分支各自新上下文)逐子樹算寬;op 行帶
   out-width+輸入寬。語義基準=既有 e2e 金樣(01_arith_widths 的 9-bit
   進位保留、33_arith_context、60_wide_values)+ value.rs 契約單測;**按
   spec 重實現,禁抄 vl-sim 代碼**(crate 法)。
4. display 格式字串編譯成 fmt 程式(`%0d/%d/%05d/%h/%b/%o/%c/%s/%%/\t/
   \\/%t`;11/45 金樣實證面)。**字串字面量路徑**:`StrLit` 在 lowering 期
   變 `Bits`(寬=8×字元數,字元 8-bit 一槽,IEEE 字串即無基數寬度向量)
   ——vl-values 無字串型是對的,字串走值通道;`%s` 從 Bits 逐字節渲染
   (11 金樣 `HI ok 100%`)。**$write=無行尾 newline**、`$display("")`
   補行尾——11 金樣 `nonewline` 單行釘死;`\\`→`\`(45 金樣
   `100% \done/`)。%t 契約=precision 單位、預設寬 20(24_clock_time 金樣:
   `$time` 在 %0d=23(單位刻度)、%0t=23000(precision 刻度))。
5. 未支援構造=**elab 期里程碑硬錯**(exit 2,訊息帶 `(milestone R<x>)`),
   清單:real 變數/實數字面量、UDP(R9)、specify、net delay(sorry 對位,
   exit 1)、SV 階梯外構造。每個恢復/枚舉迴圈帶 pos-未動 bump 守衛
   (AGENTS §4)。

**Phase B(R4,chunk 7)**:例化(參數覆蓋 #(.W(n))/defparam/位置與具名
埠)、層次名解析(`dut.l1.y`;每宣告形態註冊,AGENTS §6 禁盲前綴)、
generate 展開(GenFor/GenIf/GenCase→展開期逐一例項化,具名 block 作用域
:g/:m1/:c0)、gate 原語(12 種+bufif1/notif 三態+z 內部值)、array(R3 後半
提前:30/52/55 只需平層 array+變指標讀寫)、多驅動仲裁(單驅動=R2 直通;
多驅動+強度=R4 機械定義)、多檔編譯(57/58 的 DUT 檔)。

**方言注意(共享 parser 的代價)**:iv-e2e 04/06 需要
force/release/fork/join/wait——parser 今日 parse 級拒收,且**文字分兩族**:
`wait`/`fork`=`(milestone M8)`(stmt.rs:50-57),`force`/`release`/
`deassign`=`(milestone M7)`(stmt.rs:58-62),procedural
`assign`=`(milestone M7)`(stmt.rs:63-67)。
R3 同輪:vl-syntax 加 AST 變體+parser 臂(關鍵字已在 Kw 表);**vl-sim
把同名硬錯從 parse 級遷到 Sim::build/run 級,訊息文字逐族原樣保留
(M8 族照 M8、M7 族照 M7——審稿發現:引錯族號會在遷移中靜默改寫)**
(corpus/e2e 無釘死此訊息的案例,triage 全表無 force/fork 構造,
遷移不碰 rverilator 金樣;遷移後跑 vl-difftest 全量自證)。

## 6. runner 契約(riv-difftest 重寫;門閘調用的逐字契約見 §10)

**新旗標面**:`--e2e --corpus DIR`(跑)、`--record-iv-leg --corpus
corpus/e2e`(僅當真 iverilog 存在)、保留 `--record`(iv-e2e 錄製)。
`--e2e` 的 corpus 標籤取自路徑尾段(`e2e`/`iv-e2e`)。

**e2e 臂(每例)**:
1. 建 `/tmp` work dir,**拷貝全部非 `.v/.sv` 資料檔 + 原始檔**(鏡像
   vl-difftest 錄製的 cwd 契約;53_readmem 的 data.hex 解析靠它);
2. `riverilog -o out.riv <files>`(cwd=work dir)→ 失敗分類:exit 2+里程碑
   指針=§7 的 pending/error 判定;其他 exit=error;
3. `rivvp out.riv`(plusargs 契約僅 iv-e2e 需要;e2e 無 plusargs);
4. **看門狗**:每例 60s 牆鐘(環境變數 `RIV_CASE_TIMEOUT_MS` 覆寫),逾期
   kill → status `error` detail `watchdog`(AGENTS §4 的看門狗條款在 riv
   線從第一天落實;兩 bin 子程序使 kill 可行——這也是流程選 D2 的附帶紅利);
5. **正規化=vl-difftest 四前綴 + riv 工具行形**(§7 偏差 3):丟
   `- `/`-Info`/`-Warning`/`-Error` 前綴行(vl-difftest 契約逐字)∪ 丟
   `^[^: ]+:<line>: $finish called at ` 與 `^VCD info: ` 形行(riv 自家
   工具報告語言;敵意 fixture 單測釘死——設計行 `-7div2=-3` 與
   `tb.v:6: ...` 形的設計輸出存活);
6. byte-diff 對 `expected.stdout`;**expect.vcd 不在 runner 契約內**(VCD
   正確性走 R5 的 iv-e2e x-check;e2e dump 案例只需 stdout 面+不炸)。

**iv-e2e 臂(每例,case.json: files/-g/plusargs/enforced)**:
- `enforced`:`riverilog -g<level> -o out.riv <files>` + `rivvp out.riv
  <plusargs>`;**stdout 原始 byte-diff + exit 碼**對 expected.stdout/
  expected.exit(無正規化——iverilog 工具行本身就是金樣,01_hello 的
  finish 行在案)。
- `pending`:通過條件=riv 端**硬錯帶里程碑指針**(exit≠0 且 stderr 含
  `milestone R`);crash/訊息不對=status `error`;**正確硬錯=status
  `pending`**;若 pending 案例反而跑通=status `pass` + detail
  `DRIFT: ledger stale`(chase-ledger 紅燈,翻 case.json enforced=true
  同輪完成;**我不 commit**——工作樹留給 user 審,翻轉以 edit 呈現)。

**輸出契約(逐字)**:每例一行 `<corpus>\t<case>\t<status>\t<detail>`,
status ∈ {pass, diff, error, pending, skip};尾行
`SUMMARY e2e=<p>/<n> iv=<p>/<n>`(p=status pass 計數;未跑的臂=0/0);
**exit code 恆 0**。分類鏡像 vl-difftest:detail 含 `milestone R`(子串,
**不依賴括號**——今日訊息是無括號形 `lands at milestone R1`
riverilog.rs:161 / `lands at milestone R2` rivvp.rs:59;§5-5 新訊息用
`(milestone R<x>)` 括號形,分類器兩形通吃)= DECL 類(expected-fail
marker 對位); regress 閘以 grep 執法:`UNLEDGERED` 計數=0、`DRIFT`
計數=0、p≥基線(§8)。runner 契約逐字文中「(dir per case, tb.v +
expected.stdout)」括注是常見形描述,**實作體=本節 1-6**(雙源檔 57/58/59
、資料檔 53、51 的 VCD 檔都在案;與 §10 的調和注同讀)。

## 7. 語料 → 里程碑切片映射(triage 桶驅動;禁金鍍)

**桶 → 落點**(59+10 例全數歸位;48_frac_delay 從第一天 ledger、09_udp
錨 R9;每例恰一個落地 chunk——審稿所有權發現修正):

| 桶(triage) | 案例 | 落點 | 需要的切片 |
|---|---|---|---|
| arith/display | 01, 02, 03, 04, 05, 06, 07, 08, 09, 11, 45, 47 | C3 | 平層 elab+表達式+display(含 %s/$write/`\\`)+cont-assign+concat/part-select lvalue |
| signed/widths | 10, 12, 31, 33, 34, 46, 60 | C3 | vl-values 全面(60=多 limb 白得)、$signed/$unsigned 視圖、ashr 符號填 |
| scheduling/nba(平層) | 21, 23, 25, 27, 32, 42, 43, 44, 49 | C3 | 分層排程+@or-list/@*+repeat @posedge+NBA(21/27/42 的 dump 面=C3 最小 dump) |
| case family | 13, 26, 35 | C5 | casez(+? 遮罩雙位)/default-first(06_case_identity 屬 arith/display,上文已含) |
| loops | 14, 50 | C5 | for/while/forever(Stmt 已有 AST;`repeat` 已隨 C3 的 @posedge 慣用形落地) |
| delays | 24 | C5 | $time+%t(precision 刻度渲染是本例主題,隨 $monitor 批次);48=ledger(§7 末) |
| functions/tasks | 17, 18, 19, 20, 39, 40, 41 | C6 | 任務/函式子程序(每啟用幀+static 共用幀,§3)+task 內 @+inout copy-in/out |
| scheduling/nba(兩模組) | 22, 51, 57, 58, 59 | C7 | 例化+參數覆蓋(22/51 的 `counter #(.W(8)) dut(...)` 實證)+多檔+wire 鏈收斂(59 為 RK6 指標案例) |
| params | 15, 38 | C7 | param 覆蓋+$clog2(15 的 named override `#(.W(4),.V(3))` 即例化=非平層) |
| hierarchy | 16 | C7 | 例化+層次引用 |
| generate | 28, 29, 36, 37 | C7 | GenFor/If/Case 展開 |
| memories | 30, 52, 55 | C7 | array 變指標讀寫+NBA 字寫 |
| gates | 54 | C7 | 閘原語+bufif1(z 內部不顯示=金樣安全) |
| VCD/dump | 21/27/42/51 的 dump 面 | C3(最小)/C8(正確;51 全例 C7) | 最小 dump 經 DumpSink→vl-vcd |
| readmem | 53 | C8 | $readmemh/b(@addr/註解/OOB 警告面按金樣) |
| iv: 01/03/05 | — | C3(內容)/C4(翻 enforced) | 平層+4-state 顯示(05=vl-values 真值表白得) |
| iv: 02/08 | — | C5 | $monitor(t0 arm/on-change)+plusargs($value$plusargs 從 R5 前拉,§9-6) |
| iv: 04/06 | — | C6 | force/release+fork/join(parser 擴展,§5 方言注) |
| iv: 07 | — | C3(info 行)/C4(翻)/C8(VCD x-check) | 兩段驗收 |
| iv: 09 UDP | — | **R9,不前拉** | pending 錨 `milestone R9`(PLAN §8 R9 原序;09_udp 無金鍍替代) |
| iv: 10 | — | C7 | 閘延遲+x 未 settle 顯示 |

覆蓋覆核:e2e 12+7+9+3+2+1+7+5+2+1+4+3+1+1 = 58 = 59−48(ledger)✓;
iv 10/10 有主。

**e2e 預登記 ledger(新建檔,證據在 §0)**:
- `corpus/e2e/48_frac_delay/riv-pending.txt` = `milestone R2-semantics |
  rivD7: verilator golden rounds delays to the timescale UNIT (#1.5→2000,
  #0.25→+0); iverilog (our spec) rounds to PRECISION (1500/1750) —
  registered divergence, byte-diff impossible by design`。
- **不預登記**(build 時按結果定):32_proc_order(iverilog t0 宣告序未證;
  oracle 缺席無法今日對拍;若 diff→登記 rivD8+ledger)、10_literals(金樣
  4-state 顯示兩工具應一致)、24_clock_time(IEEE %t 共識面)、54_gates
  (z 不現身)。原則:**pending 只給已證分歧/能力未到;疑似分歧先跑後裁**
  (chase-ledger 反向:無 ledger 的提前失敗=UNLEDGERED 紅燈,§6)。

**語料不需要、明確不建**(金鍍禁令):$fopen/$fwrite MCD、$random/
$realtime、specify/SDF/-T min:max、lxt/fst、join_any/join_none(SV=R8)、
UDP 表(R9)、cocotb/wasm 面(R10)。

## 8. 閘與 3-tiers 落層(每條新鐵則四層同輪)

1. **機械閘**(`scripts/regress.sh` `[riv]` 段擴充,chunk 4 落地):
   - `cargo run -q -p riv-difftest -- --e2e --corpus corpus/e2e` → 尾行
     SUMMARY grep:`e2e=<p>/59` 且 p ≥ 基線。基線鏈(下限/目標,§7 歸位
     推得;下限=該 chunk 保證、目標=全數落地):C4 起 **19/28**
     (arith/display 12+signed/widths 7;目標含平層排程 9 例=27,32 若
     分歧再減 1)→ C5 **25/34** → C6 **32/41** → C7 **48/57** → C8
     **49/58**;弧末常態 **57-58/59**(48=永久 ledger;32=候選,oracle
     回歸後定案);
   - 同 iv:`iv=<p>/10`,p ≥ 基線:C4 起 **2/10**(01+03 翻 enforced)
     → C5 **5** → C6 **7** → C7 **8** → C8 **9/10**(09=UDP R9);
   - `UNLEDGERED`/`DRIFT` 計數 grep=0(`|| true` 護鏈,AGENTS 10.5);
   - 既有 `riv-cli` goldens + 舊 `== riverilog e2e:` 尾行 grep 同輪遷移到
     新 SUMMARY 形(chunk 4 內同步改 regress.sh,防雙閘漂移);
   - `bash scripts/regress.sh` 全鏈+`vl-difftest --min-pass 100` 59/59
     (共享 parser 改動的 rverilator 零漂移證明,每個碰 vl-syntax 的
     chunk 必跑)。
2. **正典**:SKILL 增補「riv runner 正規化=四前綴+自家工具行形;pending
   ledger 檔案=chase-ledger 對 e2e 的延伸」(chunk 4 同輪)。
3. **AGENTS**:§10.5 增一行(riv 語料樹的 ledger 檔名與 UNLEDGERED 執法)。
4. **證據頁**:riv-difftest 輸出全文本即證據;html 面不在本弧(避免與
   vl-difftest 報告頁混淆)。

**單測釘死面**:正規化契約(riv 版,含敵意 fixture)、RIV-1 往返、
kernel 區域序(03_nba 形單測:active 讀舊/#0 讀舊/#1 讀新)、$finish 行
格式、%t 縮放、force-release 殘值。

## 9. 對 PLAN 的偏差登帳(planDeviations 逐條)

1. **out.riv 前拉 R6→R2**(§3):runner 契約的兩 bin 流程+§9.8-1 共同
   強制;RIV-1 versioned 從第一天,R6 縮為成熟度+冪等閘。
2. **最小 dump 前拉 R5→C3**(§7):e2e 21/27/42/51 與 iv 07 的 stdout 面
   需要不炸的 dump;VCD 正確性+x-check 仍在 R5/C8。
3. **runner e2e 正規化擴充**(§6-5,與 §10 逐字契約的調和注同讀):riv
   自家工具行($finish called at / VCD info:)加入丟行集——iverilog 形
   引擎對拍 verilator 錄製金樣的必然(e2e 金樣在錄製期已被 vl-difftest
   lib.rs:100-109 丟掉 `- ` 工具行,金樣裡無 finish 行;rivvp 又必印
   iverilog 形 finish 行——§10 的字面讀法不可實現);單測釘死,記 rivD7
   鄰接條目。
4. **e2e 語料引入 pending ledger 檔**(riv-pending.txt):chase-ledger 律
   從 iv-e2e 延伸到 e2e(PLAN §6 只定義了 iv 側);首例 48_frac_delay
   (verilator 單位捨入 vs iverilog precision 捨入,金樣數學在案)。
5. **vl-syntax parser 接受 fork/force/wait/release/deassign**(§5):共享
   parser 的方言代價;vl-sim 同輪把這些錯誤從 parse 級遷 build 級——
   **逐臂保文字**:`wait`/`fork`=(milestone M8)(stmt.rs:50-57),
   `force`/`release`/`deassign`=(milestone M7)(stmt.rs:58-62),
   proc-assign=(milestone M7)(stmt.rs:63-67),vl-difftest 全量自證。
6. **$value$plusargs/$test$plusargs 前拉 R5→C5**:iv 08 需要;~小面。
7. **事實更正(非設計選擇)**:PLAN §0「iverilog 13.0 已安裝」今日不成立
   (§0 實證)——`--record-iv-leg`/tri_gate/32_proc_order 的 oracle 腿
   全部順延至 oracle 重裝;iv-e2e 既有金樣不受影響(錄製已入庫)。OKR/
   REGISTRY 相應行 chunk 9 更新時註記。

## 10. runner 契約(逐字,門閘照此調用)

```
  cargo run -p riv-difftest -- --e2e --corpus corpus/e2e
  cargo run -p riv-difftest -- --e2e --corpus corpus/iv-e2e
- e2e cases (dir per case, tb.v + expected.stdout): drive the riverilog compile -> rivvp run flow decided in the design, with a per-case watchdog; normalize stdout exactly like vl-difftest (whitelist prefixes only) and byte-diff against expected.stdout. Mirror vl-difftest's case classification including any expected-fail markers.
- iv-e2e cases: honor case.json (files/-g/plusargs/enforced). enforced => byte-diff stdout+exit vs the recording. pending => passes only if the binaries hard-error with the milestone pointer (nonzero exit, message names the milestone); a crash or a wrong message is status error.
- stdout: one line per case "<corpus>\t<case>\t<status>\t<detail>" with corpus in {e2e, iv-e2e} and status in {pass, diff, error, pending, skip}; final line "SUMMARY e2e=<p>/<n> iv=<p>/<n>"; exit code 0 regardless.
- --record-iv-leg --corpus corpus/e2e (only used when real iverilog exists): run real iverilog+vvp per e2e case and write expected.iverilog-stdout.txt next to expected.stdout (new files only).
```

**§10 逐字契約與 §6 的調和注(審稿裁定:兩節必須一致才可動工)**:
1. 「normalize stdout exactly like vl-difftest (whitelist prefixes only)」
   的字面讀法**不可實現**:e2e 金樣在錄製期已把 verilator 的 `- ` 前綴
   工具行丟掉(vl-difftest/src/lib.rs:100-109),金樣裡沒有 finish 行;而
   本設計(§4)要求 rivvp 印 iverilog 形 finish 行(iv 金樣 01_hello 逐字
   釘死,不可加前綴)。故「whitelist prefixes only」的執行解釋=**丟行
   白名單(前綴/形兩類),無內容改寫**:vl-difftest 四前綴 ∪ riv 自家
   工具行形(`^[^: ]+:[0-9]+: \$finish called at ` 與 `^VCD info: `),
   單測含敵意 fixture(設計行 `-7div2=-3`、以 `tb.v:6: ` 開頭的設計輸出
   都存活)。此為 §9-3 記帳的偏差,是本設計對 §10 該句的唯一偏離。
2. 「(dir per case, tb.v + expected.stdout)」括注描述常見形;實作體=
   §6-1..6(雙源檔 57/58/59、資料檔 53 三件、51 的 expect.vcd/oracle-ref.vcd
   均在案;glob 源檔+拷非源檔+top 自動偵測)。
3. 其餘逐字遵守:SUMMARY 形、status 枚舉、exit 0、--record-iv-leg 新檔
   政策(今日 0 個 expected.iverilog-stdout.txt 在案,§0 實數)。

## 11. build-order chunks(每 chunk=一個 builder session;1-4 交付
「能在簡單 TB 上編譯+跑」的引擎)

- **C1 vl-values 提取**:§2 全文(shim+測試分流+零漂移四證)。驗收:
  regress 全鏈綠、e2e 59/59、vl-values 內 property 閘在案。
- **C2 riv-ir + 平層 elab**:RIV-1 往返閘;riv-elab Phase A(§5-1..5:
  top 偵測/decl/常量/寬度烘焙/display fmt 編譯含 %s 的 StrLit→Bits 路徑/
  milestone 硬錯面);riverilog 對平層設計寫出 RIV-1(-o,預設 a.out
  對位)。驗收:cargo build 零 warning;`riverilog -o /tmp/x.riv
  corpus/e2e/01_arith_widths/tb.v` exit 0 且 RIV-1 可讀回。
- **C3 riv-kernel + rivvp 貫通**:§4 全文(分層迴圈/時間輪/執行緒/delay/
  edge/level/NBA/finish 行;Monitor 區域骨架在,$monitor/$strobe 語義
  C5)+$write/%s/`\\` 顯示面(§5-4)+`repeat(n) @(posedge clk)` 慣用形
  (44/27/42 需要)+最小 dump(DumpSink→vl-vcd);rivvp 載入 RIV-1 跑通。
  驗收:01_hello/03_nba 形 TB 手跑逐字對金樣(翻轉在 C4)。
- **C4 runner 重寫 + 閘落地**:§6 全文(--e2e/看門狗/正規化+單測/SUMMARY/
  ledger 判定)+regress.sh `[riv]` 擴充+SKILL/AGENTS 條文(chunk 4 四層
  同輪)。驗收:兩條 runner 契約命令 exit 0;**e2e 基線下限 19/59、目標
  28**(arith/display 12+signed/widths 7 為下限;目標含平層排程 21/23/25/
  27/32/42/43/44/49);**iv 翻 01_hello+03_nba → 基線 2/10**(case.json
  edit 不 commit)——03 的翻轉在此認領(審稿發現:原案無主,弧末 9/10
  不可達);regress 全鏈綠。
- **C5 R3-A 行為批次**:for/while/forever(14/50)/casez(13/26/35)/
  $time+%t(24)/$monitor/$strobe/plusargs。e2e 下限 25、目標 34;iv 翻
  02/05/08 → 5/10。
- **C6 R3-B 程序批次**:前置=§3 幀模型已在 C2 的 RIV-1 落地(每啟用幀
  棧+static 共用幀),本 chunk 加**兩執行緒並發呼叫 static task=共用儲存**
  單測(語料無此形,§0 實證);tasks/functions(17-20/39-41)+fork/force/
  wait parser 擴展+vl-sim 錯誤遷移(§5 方言注,M8/M7 逐族)+iv 翻 04/06。
  vl-difftest 59/59 自證。e2e 下限 32、目標 41;iv 7/10。
- **C7 R4 結構批次**:例化/params(15/38)/層次(16)/generate(28/29/36/
  37)/arrays(30/52/55)/gates+延遲(54)/兩模組排程例(22/51/57/58/59);
  iv 翻 10 → 8/10。RK6 檢查點:59_i2c 若區域序對拍反覆不收斂→凍結族+
  known-divergence 條目。e2e 下限 48、目標 57。
- **C8 R5 IO 批次**:$readmemh/b(53)+VCD 正確性(vl-vcd reader 交叉)+
  iv 07 VCD x-check 全翻。e2e 下限 49、目標 58;iv 9/10。
- **C9 收口**:32_proc_order 裁決(oracle 在場時;分歧→rivD8+ledger,
  一致→目標 58 可達)、ledger 終掃、rivD 分歧簿/REGISTRY/OKR/AGENTS/
  PLAN §13 pointer 全落、README status 行、基線定稿(**e2e 57-58/59 +
  iv 9/10 為弧末常態**;48/09 永久 pending 錨)。

## 附錄 A:Op 家族(kernel 執行面;C2 定 RIV-1,C3 補排程類)

| 家族 | op(非窮舉,簽名面) | 備註 |
|---|---|---|
| 資料移動 | Const, Mov, Cast{trunc,zext,sext,view-signed} | 寬度烘焙 |
| 算術 | Add, Sub, Mul, Div, Rem, Pow, Neg | div/rem 除 0=x(vl-values 契約) |
| 位/縮減 | And..Xnor, ReduceAnd/Or/Xor/Nand.., Not | 4-state 表=vl-values |
| 移位 | Shl, Shr, Ashr(左值符號填按寬) | AGENTS 10.8 律在 vl-values 側 |
| 比較/邏輯 | Lt..Gt(±), Eq/Ne/CaseEq/CaseNe/CaseZ/CaseX, LogicAnd/Or/Not | 自決定上下文 |
| 拼接 | Concat{parts}, Replicate{n,v}, PartSel/BitSel(常量+變指標, r/w) | lvalue 形同 |
| 控制流 | Branch, Jump, Case dispatch | |
| 賦值 | AssignBlk, AssignNba(入 NBA 批), AssignForce/Release/Deassign | |
| 排程 | Delay{ps}, WaitEdge{sens}, WaitLevel{slots}, WaitEvent, Disable | |
| 程序 | Call/Ret(func/task;每啟用一幀,`frames: Vec<Vec<Bits>>`+`ret_pc` 棧;static 綁每例項共用幀、automatic 走啟用棧,§3), Fork{branches}, JoinPoint, EndThread | 槽幀續存;C6 單測釘並發 static 語義 |
| 系統 | SysCall{id, fmt-pc, arg-slots, file, line} | $display 族/$finish/$stop/$monitor/$strobe/$time/$dumpfile/$dumpvars/$readmem*/plusargs — VPI-first:經註冊表(ADR-0003 形狀,R3 起 shim,R7 外部模組面) |
