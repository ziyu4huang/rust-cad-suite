# ADR-0004: vl-vcd — 單一 VCD 核心,三面 facade 共用

狀態:Accepted(2026-10-23)。前序:ADR-0003(facade 慣例)。

## 背景

拆分前:VCD writer 住 vl-sim(`vcd.rs`,寫出器);JS demo 有 66 行自製 parser
(只認自家輸出子集);vl-difftest 有第二份 parser+norm_val;vl-ffi 完全沒有
VCD 面。「bun→wasm→rverilator 共用 VCD」實際上沒有發生——三份實作三份漂移面。

## 決策

1. **新 crate `crates/vl-vcd`,零依賴**,兩半:
   - `writer`:自 vl-sim 原樣遷出,**值介面改 4 態位串**(`"0101xz"`,標量=單字元)。
     零漂移論證:`Bits` 與 `fmt_bin()` 串逐寬度雙射(x=mask1data0、z=mask1data1,
     `value.rs:499-511`),故字串相等 ⇔ Bits 相等,writer 的去重/槽摺疊語義不變;
     由 e2e 四案例 `expect.vcd` byte-diff 機械證明(51/21/27/42)。
   - `reader`:容忍式解析器(採用/拒收表=docs/GTKWAVE-VCD-STUDY.md,即契約)。
     值=`VcdValue{Scalar(char)|Vector(String)|Real(String)|Str(String)}`——
     **刻意不用 Bits**:VCD 向量可超 64 位(我方 Bits 目前單 limb 上限),
     reader 必須位寬無關;正規化走 `normalize_width`(difftest `norm_val` 逐字遷入,
     行為=交叉對拍契約,禁「順手修正」)。
2. **錯誤模型自帶**(不依賴 rcs-diag):`VcdDiag{code,message,offset}`;
   E1xx 硬錯(E100 值變形/E101 #time/E102 $var/E103 $scope/E104 dumpports)、
   W1xx 容忍(W100 未知 id/W101 截斷/W102 EVCD 強度/W103 跨形式),warn cap 128。
   VCD 文字不值得拖 diag 的 Span/SourceMap 機器進 wasm。
3. **三面接線**:
   - vl-sim:writer 呼叫端(4 個 record_change 站點改傳 `fmt_bin()`);
     新增 `Sim::force_dump()`(無 TB `$dumpvars` 時強制出圖,wasm/FFI 用)。
   - vl-wasm:`vcd_parse(text) -> VcdJs`(times=Vec<u64>→BigInt64Array,values=原始
     字串);demo 頁改吃共享 reader,**JS 手搓 parser 退役刪除**(留 fallback=兩份
     parser 必漂移;wasm 模組本來就在頁面載入,fallback 無觸發條件)。
   - vl-ffi:manifest 新關鍵字 `vcd <path>`(隱含 force_dump+落檔)+
     `rcs_vcd_parse(text)` 行式協定(ok/var/c/warn;錯誤走 `ERROR: ` 前綴契約)。
     **rcs_run 回傳值保持 stdout-only**——混入 VCD 會破壞 embedder 的逐字比對
     (bun-example/run.mjs 契約)。Bun 範例 `vcd_read.mjs`=「agent 讀波形」驗收:
     跑語料→強制 dump→共享 reader 解析→斷言 q 終值。
4. **vl-difftest 重指向**:刪 parse_vcd/VcdData/norm_val,vcd_crosscheck 改吃
   `vl_vcd::parse`;對帳=vcd_gate 樣本點數前後一致(2/2@10、3/3@23)。
5. **e2e 閘同輪收緊 80%→100%**:0173318 的函數註冊回歸(39/40_func)曾以
   96.5% 藏身綠燈——精確數門檻是本教訓的機械化。

## 後果

- 新依賴邊:vl-sim/vl-wasm/vl-ffi/vl-difftest → vl-vcd(單向;反向依賴=cargo 硬錯)。
- writer 每次 record 多一次小 String 配置(dump 開啟時);直譯器為主的工作負載
  不構成量測點;tier-B(--gen-rust)是效能路徑且不受影響。
- `force_dump()` 契約=必須 run() 前呼叫(dump_t0 標 `#0`);兩個 facade 呼叫點皆前置。
- 非目標(v1,變更需新 ADR 修訂):gzip、$dumpports body、$timezero 套用、
  EVCD 強度語義、FST(PLAN M9)。

## 證據

- 零漂移:`bash scripts/regress.sh` → 57/57(含 4 例 expect.vcd byte-diff)。
- reader:`cargo test -p vl-vcd`(writer 3+reader 7,fixture 含磁碟真樣本
  51_vcd_counter/expect.vcd)。
- wasm:`wasm-test/vcd_parse.mjs`(BigInt 時序/單調/終值/錯誤路徑)。
- FFI:`bun crates/vl-ffi/bun-example/vcd_read.mjs` PASS ffi-vcd-read。
- 瀏覽器目檢:docs/e2e-report/wasm-demo-v2-verified.png(+accpipe)。
