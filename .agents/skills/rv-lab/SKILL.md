---
name: rv-lab
description: 操作 rust-cad-suite 模擬器的 agent 工具箱——跑模擬、對 oracle 差分、渲染波形、iverilog 形流程、瀏覽器 GUI;四支 scripts 包 CLI,一律經此層調用
---

# rv-lab:agent 操作模擬器的工具箱

適用:要在 rust-cad-suite repo 裡**執行** Verilog/模擬/看波形(而非開發模擬器
本身——那是 [eda-dev](../eda-dev/SKILL.md) 的職掌)。

## 鐵則(違反 = 假綠)

- **oracle 即 spec**:行為分歧時 `/opt/homebrew/bin/verilator`(rverilator 面)
  或 `iverilog 13.0`(riverilog 面)贏,LRM 只用來解釋。分歧要修碼或記
  `docs/REGISTRY.md` 分歧簿,禁止無據斷言。
- **期望值禁止手寫**:入庫期望只准 `cargo run -p vl-difftest -- --record`
  (或 iv 臂等價)從 oracle 錄製;`rv-diff.sh` 的輸出只做開發期判讀。
- **最強閘**:任何語義改動後 `bash scripts/regress.sh` 必綠才算改完。

## 四支 scripts(scripts/,全部從任意 cwd 可用)

| 命令 | 做什麼 | 何時用 |
|---|---|---|
| `bash scripts/rv-sim.sh [-t] [-L] <file.v>...` | rverilator 編譯+模擬;`-t` 錄 VCD、`-L` 切 legacy 引擎對照 | 跑一個設計看 stdout |
| `bash scripts/rv-diff.sh <file.v>` | 我方 vs verilator 並排+正規化 MATCH/DIFF 判決(exit 0/1) | 語義分歧的第一探針 |
| `bash scripts/rv-wave.sh [-o out.svg] <file.v>...` | 模擬+錄 VCD+`vcd2svg` 出 SVG 波形(ADR-0005 單渲染路徑) | 看波形;`KEEP_VCD=1` 留 .vcd |
| `bash scripts/riv-run.sh [rivvp flags] <file.v> [plusargs...]` | riverilog→RIV-1→rivvp 完整 iverilog 形流程 | iverilog 面;`KEEP_RIV=1` 留 out.riv |

範例:

```bash
bash scripts/rv-sim.sh corpus/e2e/01_arith_widths/tb.v        # 與金樣 byte-exact
bash scripts/rv-diff.sh /tmp/probe.v                          # 分歧?oracle 說了算
bash scripts/rv-wave.sh -o /tmp/wave.svg corpus/e2e/51_vcd_counter/tb.v
(cd corpus/iv-e2e/01_hello && bash ../../scripts/riv-run.sh -n tb.v)
```

注意:`$finish` 橫幅帶**命令列上的路徑原樣**,byte-exact 比對時要像金樣錄製
時一樣從 case 目錄跑相對路徑。

## 瀏覽器 GUI(web → wasm → rust,ADR-0005 憲法)

```bash
bun crates/vl-wasm/wasm-demo/serve.ts   # → http://127.0.0.1:8031
```

編輯器寫 Verilog → ▶ Run → stdout + VCD 由 **vl-wasm**(同一 riv 核心)執行,
波形由 **vl-vcd 的 Rust SVG 渲染器**在 wasm 內畫出(信號名可點選開關);另有
XLS 獨立驗證卡與 spec lockstep 卡(ADR-0007)。與 CLI 共用同一核心與同一渲染
路徑——瀏覽器只是注入 SVG,不算圖。

## 閘

- regress 內建 agent-toolbox smoke:四支 script 各對一個 corpus case 的
  golden 實跑比對;改 scripts 或 CLI 輸出契約後 regress 會抓。
- wasm 面閘(regress):pkg 差分 8/8、vcd-parse/vcd-svg、spec gate、demo serve
  smoke(200+no-store)。
