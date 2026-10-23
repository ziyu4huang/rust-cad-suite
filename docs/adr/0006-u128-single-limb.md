# ADR-0006: 單一 u128 limb(64→128 位寬上限擴寫)

狀態:Accepted(2026-10-23)。前序:ADR-0001(值域單 limb,M7 時 64 位上限)。

## 背景

lint 差分軌(ADR-0004 同期立的 M7b 路線圖)預登記:餘 13 檔差異中一族
(case_large/dynslice/partsel/constpower/generate/const_func_shadow)被
「vectors wider than 64 bits」硬錯攔住——yosys tests/simple 語料存在
65–128 位常量。單 limb 擴到 u128 是預登記的最大單一槓桿。

## 決策

1. `Bits{data, mask}` 自 `u64` 擴為 **`u128` 單 limb;`MAX_WIDTH` 64→128**。
   >128 位仍硬錯(多 limb 布局=獨立 ADR,維持 ADR-0001 圍欄)。
2. API 面不破壞:`from_u64` 保留(u64 輸入恆可容),新增 `from_u128`/
   `to_u128_lossy`/`to_i128_lossy`;`to_u64_lossy`/`to_i64_lossy` 保留為
   「低 64 位有損」形式(文檔註明)。
3. **有符號語義升級**:div/mod/pow 的 signed 路徑自 i64 改 **i128**
   (`to_i128_lossy`)——擴寬後 65–128 位有符號運算若走 i64 會靜默繞回。
4. **位移量陷阱修**:`shl/shr/ashr` 的位移量改 `to_u128_lossy` 判界
   (>64 位位移量 pre-widening 會被 `as u64` 截斷成小值=靜默錯誤);
   `ashr` 的 w==128 時無填充空間,分支直移。
5. **字面量累積防溢**:`from_numlit` 十六進制/二進制累積加 128 位房間守衛
   (超位數字只攜高位,丟棄=既有截斷語義;無守衛=debug build 位移溢出 panic);
   值域 fit 檢查改 `declared.min(MAX_WIDTH)`;十進制字面量改 u128 parse。
6. 金律不變:x/z 編碼( mask1data0=x, mask1data1=z)、算術遇未知=全 x
   (IEEE §5.1.4)。

## 效果(實測)

- lint parity **74→77/87**(case_large/dynslice/partsel 三檔翻綠);
  路線圖 ×5~7 估計偏樂觀——constpower/const_func_shadow/generate 三檔實際
  需 >128 位(多 limb=未來 ADR),multiplier/mem_arst/paramods/i2c 卡點在
  lvalue 範圍/屬性形狀/defparam/intra-assign,與位寬無關。
- e2e **57/57 零漂移**(語料無 >64 位值;fmt_dec 全寬 u128 Display)。
- 29 套件 cargo test 全綠;`wide_u128_values` 測試釘死 65–128 位往返/
  全寬 fmt_dec/fit 邊界/>64 位移量/i128 有符號。

## 基線事故備忘(閘的價值)

首度 bump 基線時把 77 誤算成 78(拒絕清單計數≠淨增計算)——**regress 閘
當場紅燈拒收**(`lint parity 77/87 < committed baseline 78`),修正為 77
後過閘。精確數門檻第二次攔下人為錯誤(第一次=1023 函數註冊回歸)。
