# ADR-0008: Multi-limb values (>128 bits)

- 狀態:Accepted(2026-10-24)
- 背景:ADR-0006 把值表示拓到單一 u128 limb(width ≤ 128);更寬的向量
  (語料需求最大 3594 位元,const_func_shadow 的函數返回寬度)在此封頂報錯。
- 裁決:
  1. **表示**:`Bits` 保持 struct(`pub width: u32` + 私有 `repr`),`repr` 為
     二態 enum:`N { data: u128, mask: u128 }`(width ≤ 128,數學與 pre-M8
     逐位一致)與 `W { data/mask: Box<[u64]> }`(little-endian limbs,高位
     limb 預遮罩)。結果寬度 ≤ 128 一律 canonicalize 回 N 臂——熱迴圈永不
     見 heap 形。struct 形保留 `bits.width` 欄位讀取面(30+ 呼叫點零改)。
  2. **上限**:`MAX_WIDTH = 1 << 20`(1M bits;語料最大需求 3594,餘量三個
     數量級)。超限=乾淨錯誤。記憶體總量閘:記憶體陣列 words×width > 2^30
     bits 拒絕(單源碼行 OOM 防護)。
  3. **運算契約**:
     - N×N 臂=pre-M8 代碼逐字保留(熱路徑零行為變化);
     - 混合/W 臂走 `Limbs` 引擎,結果經 canonical pack;
     - 帶號除/模/冪/比較新增 limb-aware 方法(`sdiv`/`srem`/`spow`/
       `cmp_signed`/`cmp_unsigned`),eval 的 i64/i128 通路退役;
     - **冪=精確模 2^w**(square-and-multiply,指數上限 2^20;超限保守 all-x
       對齊小臂 checked_pow 溢位政策)——新地面語義,與 Verilator 2-state
       寬數學一致(case 60 對拍通過);
     - `clog2`/`$clog2`、`%d`/`%o`/`fmt_dec(_signed)`、from_numlit(十進制
       limb 摺疊,61+ 位十進制字面量合法)、readmem(>128 位字=顯式錯誤,
       M8 readmem)全部 limb-aware。
  4. **符號 limb 紀律(三實錄硬化)**:limb 級運算在整 limb(64 的倍數)上
     跑,**符號中間值必須按 width 遮罩**——neg/pow 的 padding 位元垃圾會
     複合(spow 雙重負+limb 3 垃圾實錄);reduce_and/or 的補數表達式必須以
     care 遮罩閘住(mask 輸入會被 `!d` 展開回去);同號比較不需反轉
     (二補數 pattern 序=值序,`12_signed_ops` e2e 抓的翻面)。
  5. **效能**:窄路徑表示大小不變(40B),僅多一次 discriminant 檢查。
     A/B(interleaved release,2M-tick 32-bit counter):基線中位 2.35s vs
     M8 2.40s(+2%,same-tool spread ±15% 內;≤5% 驗收線)。寬值絕對數:
     256-bit 加法每週期 ~71 萬週期/s(0.28s/20 萬週期)。
- 證據:
  - lint parity **83/87**(基線入 regress.sh;constpower/const_func_shadow/
    generate 三案例翻綠;餘 3 OURS-ONLY=i2c intra-assign/reals/字串,
    不同根因;1 ORACLE-ONLY=values.v 既有登記分歧)。
  - e2e **59/59 @ 100%**(新 case `60_wide_values`:跨 limb 的 sum/prod/div/
    mod/移位/切片/帶號除餘/pow/%h/%0d/clog2 對 Verilator 5.052 逐字)。
  - wasm 8/8(三面 parity 延伸到寬值)。
  - 附帶修復(被寬埠錯誤遮蔽多年的潛伏 bug):①負 genvar `i as u64`=
    2^64-1 爆(generate.v gen_test6 下行迴圈,改 32 位元二補數截斷);
    ②65..=128 位元帶號/無號比較經 i64 截斷=與 oracle 活分歧(96-bit 重現
    釘入 value.rs 測試);③`$clog2` 65..128 位元截斷;④`%o` 64 位元以上
    截斷;⑤Repeat nat_width u32 靜默回繞({2^32{1'b1}} 寬度 0);⑥宣告
    範圍寬 i64→u32 靜默截斷(range_width 統一帶上限錯誤)。
- 非目標:>2^20 位元(乾淨錯誤);Knuth-D 除法(O(bits²) 位移恢復除法夠用,
  語料無寬除熱點);4-state limb 傳播(2-state 契約不變,IEEE §5 表仍在
  單測分軌);vl-vcd 字串介面不變(ADR-0004);rVPI 觸發點不變(ADR-0003)。
- 方法論註:planner 出案 → 對抗審(實跑取證:三案例=寬埠宣告非寬運算、
  比較=活蟲非潛伏、breakage 面雙倍於計畫、OOM 總量閘缺口)→ 實作吸收全部
  P0/P1;e2e 閘在首輪抓到 both-negative 比較翻面=差分語料價值的直接實證。
