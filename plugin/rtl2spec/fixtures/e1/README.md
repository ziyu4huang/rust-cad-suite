# fixtures/e1 — 復原對拍評測裝備(S5 遷入;自著 MIT;§5.2 S5/RTL2SPEC-MVP-NEXT)

本目錄=E1 實驗裝備從 `build/mvp-exp/e1/`(gitignored,禁清理但弧後易丟)
的 in-git 遷入,**byte-identical 複製原樣未動**(re-record 對照:2026-11-05)。

## 內容與 provenance

- `tb_counter8.v` / `tb_acc_pipe.v` / `tb_i2c.v` = 三份公共 TB(6+6+11=23 場景;
  實驗者=復原力弧實施 agent 自著;場景設計記錄見 build/mvp-exp/e1/README-e1.md §2.4)。
- `dut_orig_i2c.v` / `dut_reimpl_i2c.v` / `dut_orig_accpipe.v` / `dut_reimpl_accpipe.v`
  = 四份 wrapper(對拍方補齊接口缺口信息的實驗裝備;接口差異=規格書缺口證據,
  README-e1.md §5)。
- `../e1-reimpl/<leg>/top.v` = 盲寫產物(E1 原始輪,GLM-5.3-Flash,2026-11-05,
  原樣未動;盲隔離靠提示詞約束,README-e1.md §4/§5):
  - `counter8-a1/` = counter8 盲寫(A 組 n=1;E1 實測 6/6 matched)。
  - `i2c-a0/` = i2c 盲寫原始(A 組 n=1,**pre-S1 規則書**;compiled=false——
    `state` 未聲明,規則書無宣告類規則的實證)+`top_fixed.v`(最小修復版,
    僅補 1 行聲明,E1 實測 10/11 matched,唯一失配 SCENE10=異步復位語義丟失)。
  - `acc-a0/` = acc_pipe 盲寫(A 組 n=1,**pre-S2 規則書**;E1 實測 0/6 matched,
    組合定義規則缺失的實證)。
- `tb_i2c_dbg.v`(TB 調試版)未遷入——README-e1.md §1「終版不用」;原檔留 build/。
- 原版 ground truth=corpus/ 與 crates/spec-i2c/ 既有檔,零重複遷入。

## 執行

`bash plugin/rtl2spec/run-reimpl-eval.sh`(iverilog 守衛,缺場 exit 2;
寫只落 plugin/rtl2spec/build/reimpl-eval/;直調不進 regress——盲寫腿=輔助證據,
永不進確定性閘;機械面=編譯成敗+TB 逐場景 diff)。
