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

## 收割 → 收編(2026-10-26 更新)

workflow 現在輸出可自我驗證的 artifact(`manifest.json` + `generated/` +
`dslx/` + raw + LICENSE/NOTICE),並在 CI 內用 `rv-xls-sim --syntax-check`
先過一次語法閘。本機收編:

```sh
python3 scripts/xls_harvest_manifest.py --dir <artifact> --tag <xls-tag> \
    --adapter target/debug/rv-xls-sim     # CI 內自動跑
python3 scripts/xls_adopt.py <artifact>  # 驗 sha256 + 正規化 + 寫入本目錄
```

`xls_adopt.py` 先驗證全部檔案再寫入(竄改=整批拒絕),且拒收 syntax_check
失敗的 artifact;它刻意不生成 e2e 案例(TB 要人寫、期望要 oracle 錄),
最後印出「寫 TB → 單案例錄期望 → 面板自動收錄」三步。收編後的
`harvest-manifest.json` 記下 tag 與 sha256 供追溯。

