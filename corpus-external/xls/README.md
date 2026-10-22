# XLS codegen 語料快照(Apache-2.0)

來源:https://github.com/google/xls(release linux-x64 二進位,經
.github/workflows/xls-harvest.yml 一鍵收割:DSLX→IR→Verilog codegen)。

## 內容
- `dslx/` — 我們寫的 DSLX 樣本(add3 組合、acc_pipe 二級流水)
- `generated/` — XLS codegen 輸出(.v + .ir)
- 正規化(workflow 內,raw 語義不變):`always_ff`→`always`(V2005 目標)、
  注入 `` `timescale 1ns/1ps ``(XLS 無 timescale 概念;不注入會在
  檔序先於 TB 時觸發 verilator TIMESCALEMOD 致命警告 = REGISTRY D16)。

## 授權
Apache-2.0(帶專利條款):XLS-LICENSE.txt + XLS-NOTICE.txt 隨快照。
我們寫的 dslx/ 樣本與 TB 遵循本 repo 的 MIT。

## 差分語料
corpus/e2e/57_xls_add3、58_xls_acc_pipe(TB 在案,期望=oracle 錄製)。
