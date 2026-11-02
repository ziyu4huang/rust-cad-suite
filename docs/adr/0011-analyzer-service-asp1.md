# ADR-0011: 統一分析器服務(analyzer-core + analyzerd + ASP/1)

日期:2026-11-02。狀態:**接受**(隨 S1..S5 落地轉正;正典企劃=docs/ANALYZER-SERVICE.md,
實作記錄=同文 §17)。
脈絡:使用者原話目標——模組化程式碼,把 Verilog/SystemVerilog 分析器拆出來獨立
可用且高效;原生 SV 與 yosys RTLIL 以一致 API 吃進來,完善可分析性;client/server
模式,client 以純 Bun TypeScript 與 server 通訊。

## 決策

1. **D1 crate 劃分**:兩個新 crate——`analyzer-core`(lib)+ `analyzer-server`
   (bin `analyzerd`);**零文件搬移**,vl-syntax/ry-ir 原地復用為庫依賴。
2. **D2 協議=ASP/1**:UDS(SOCK_STREAM)+ 4 字節大端 u32 長度前綴幀 + 負載=
   手寫 JSON 子集(零 serde;Int=i64/bigint,禁浮點);幀上限默認 64 MiB、
   鉗位 1 GiB;違約先檢查後分配。
3. **D3 併發模型**:thread-per-connection、連線間並發、單連線內請求串行;
   analyze 為純函數,線程安全零成本。
4. **D4 停機**:shutdown op 回 ack → unlink → drain(上限 2s;既有連線僅
   shutdown/ping 豁免,新 client 因 unlink 先行=connect 即失敗)→ exit 0;
   SIGTERM 硬殺由啟動期殘留清理兜底(先試連,連通=活實例硬錯 exit 1)。
5. **D5 socket 路徑**:默認 `/tmp/analyzerd.sock`;`--socket` > `ANALYZER_SOCK` >
   默認;路徑 ≥100 字節硬錯(macOS sun_path 104 含 NUL 留餘量);綁定後
   0o600。
6. **D6 統一視圖**:`Analysis`(模組/端口/參數/nets/cells/實例邊/診斷)由
   既有 IR 直讀投影,零新語義引擎;SV/RTLIL 原生逃逸口保住可分析性上限;
   RTLIL 側 span 恆 None(ry-ir AST 無 span,R1 記錄)。
7. **D7 編解碼一致性**:共享向量文件 `clients/analyzer-ts/fixtures/
   codec-vectors.json`(25 條,生成器首錄+雙端斷言維持),Rust 測試與 bun test
   同讀;TS 側禁 JSON.parse/JSON.stringify(number Int)/非 fatal UTF-8
   (bun 1.4.2 放行行為實測在案)。
8. **D8 bench=資訊性**:smoke 打印 per-op 延遲,不引入 criterion、不設硬閾值。
9. **D9 regress.sh S5 塊**:插在 ryosys 塊後、--with-record 前,帶 crate 在場+
   bun 在場雙守衛可降級;既有基線(469/479+456/456)零接觸。

## 後果

正:零搬移縱切小(兩 crate+一 client 目錄,既有 10 個消費端依賴邊不動);
core-first(10.7)使未來 wasm/FFI/LSP 面直呼 `analyzer_core::analyze` 零複製;
JSON 子集禁浮點使 TS↔Rust 數字序列化逐字節一致成為平凡命題;雙入口(Sv|Rtlil)
共用同一視圖與查詢形=「一致 API」的直接體現。
負:JSON 子集自養(轉義/數字/嚴禁項兩端各一份實現,靠向量文件單一事實源維持
一致);RTLIL 側查詢與診斷無位置(§4.3 不對稱如實呈現);std-only 停機無信號
處理,硬殺留殘留文件(啟動清理兜底);accept 輪詢 5ms(非事件驅動,本機服務
可接受);macOS accepted fd 繼承 listener O_NONBLOCK 的平台行為需顯式
`set_nonblocking(false)`(實錄在 §17 S3)。
