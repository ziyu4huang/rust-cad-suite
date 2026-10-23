# GTKWave 的 VCD 能力面研讀(2026-10-23)

對象:gtkwave/gtkwave @ main。VCD 讀取器=`lib/libgtkwave/src/gw-vcd-loader.c`
(舊 `src/vcd.c` 已重構成薄殼,其檔頭 changelog 自 1999 起記錄了 20+ 次現實世界
VCD 相容性修復:evcd/finsim 參數/多重 $var 段/重複網/跳脫層級/real_parameter
vartype/MTI SV len=0 即 real…)。本研讀目的:為 vl-vcd reader 選定容忍子集
(採用/拒收表在末節,ADR-0004 為契約)。

## 解析器功能面(代碼證據)

**Header tokens**(Tokens enum):`$var/$end/$scope/$upscope/$comment/$date/$dumpall/
$dumpoff/$dumpon/$dumpvars/$enddefinitions/$dumpports(+off/on/all)/$timescale/$version/
$vcdclose/$timezero`——後三者是 IEEE 1364 之外的現實擴充(VCD close 時戳、Cadence
Extended-VCD dumpports 族、時間零點偏移)。

**Scope 型別表**:`module/task/function/fork/begin/generate/struct/union/class/
interface/program/package` + `vhdl_*` 前綴族(architecture/record/block/generate/
genif/genfor/function/process/procedure)——樹形只影響顯示分組,不影響值。

**標量值字元**:`0/1/x/X/z/Z` 之外還收 EVCD 強度字元 `h/u/w/l/-`(9 值 MVL9;
內部 RCV_X/Z/H/U/W/L/D 重編碼)。

**向量形式**:`b/B` 位向量、`r/R` 實數、`s/S` 字串,且**跨形式正規化**:
bits 變數收到 `R` → 當 `B`;real/string 變數收到 `B` → 當 `S`(recoder 防禦,
`process_binary` 內 if/else)。

**識別碼查找**:單字元 id 走快速路徑表;任意長 id 走排序陣列+bsearch
(`vcdid_hash`+`symbols_sorted`);同 id 多 `$var`(別名/重複網)自然共享值流。

**時間**:`#<u64>`;`$timescale` 單位字元 `m/u/n/p/f/a/z/s`(ms…zs)+帶符號乘數;
`$timezero` 記錄為全域時間偏移。

**容錯語義**(全部非致命):未知 id=stderr 訊息+byte offset 續跑;EOF 截斷=
`malform_eof_fix` 保留已解析部分;CRLF;`[msb:lsb]` 後綴;gzip 輸入
(`is_compressed`)。

**內部儲存**:逐符號 vlist(時間差+值重編碼壓縮)——GTKWave 自己的 trace 格式,
與 VCD 解析正交。

## vl-vcd 採用/拒收表(v1 契約,reader.rs 文檔註解同步)

| GTKWave 能力 | vl-vcd v1 | 理由 |
|---|---|---|
| 未知 id 非致命+byte offset | ✅ VL-W100 | 現實檔案必需 |
| EOF 截斷保留已解析 | ✅ VL-W101 | 同上 |
| EVCD 強度標量 h/u/w/l/- | ✅ 收為 Scalar(char)+VL-W102 | 留辨識,不做強度語義 |
| b/r/s 跨形式正規化 | ✅ VL-W103(值保 raw) | oracle VCD 健壯性 |
| 重複網/別名 | ✅ 同 id 多變數各收一份變更 | difftest alias 折疊同構 |
| `$timezero`/`$vcdclose` | ✅ $timezero 記錄不套用;其餘跳過 | 記錄≠解釋 |
| 任意 scope 型別 | ✅ 收為不透明字串 | 樹形夠用 |
| `$dumpports` 族 | ❌ body=硬錯 VL-E104 | EVCD 語義整族超範圍,明確錯勝過默錯 |
| gzip 輸入 | ❌ | facade 層已能解壓;核心保持零依賴 |
| $timezero 套用為偏移 | ❌ | 影響所有時間戳比對,需獨立 ADR |
| 強度語義/FST | ❌ | FST=M9 證偽線外的獨立工程 |
| 單字元快速路徑/bsearch | ❌(BTreeMap) | GTKWave 級效能非本 reader 目標 |

## 與本 repo 的對位

- 我們的 **writer**(vl-vcd/writer.rs,自 vl-sim 遷出)產 IEEE §18 正典形
  (sorted vars/時間槽摺疊);**reader** 的首要客戶=oracle(verilator)產 VCD 的
  交叉對拍(vcd_gate)+ 三面 facade 共用解析(CLI/wasm/FFI)。
- GTKWave 的渲染面(marker/translation filters/FST)屬檢視器工程,不在
  rverilator 範圍;本 repo 的 canvas 波形(demo 頁)只消費 vl-vcd 的 typed 輸出。
