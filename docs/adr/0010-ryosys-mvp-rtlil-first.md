# ADR-0010: ryosys MVP 範圍與 IR 主權(RTLIL-first)

日期:2026-10-27。狀態:**接受**(隨 M0 slice-2 落地轉正;企劃=docs/RYOSYS-MVP.md,
證據基礎=§0 本機 oracle 實跑復核)。
脈絡:PLAN §1 P2;五路調研(driver/kernel、RTLIL 文法、前端/後端、passes、
本倉慣例)+ oracle 實跑;yosys 源碼 clone(v0.70,gitignored 唯讀)落後二進制
179 commits——讀碼用 clone、golden 一律二進制實跑,細節分歧以二進制為準。

## 決策

1. **RTLIL-first**:MVP 輸入=RTLIL 文本(-readable 傳統全名形),不做 Verilog
   前端;外部 .v 語料經 oracle(read_verilog+proc)在錄製輪預處理成 .il 入 git。
   IR 主權=ry-ir 擁有 RTLIL 對象模型,oracle .il 是唯一合法輸入形態;測試運行
   零 oracle 依賴。
2. **crate=ry-\* 家族**(ry-ir/ry-cli/ry-difftest),bin 名 `ryosys`,不冒名
   `yosys`(PATH 安全,承 PLAN §8 #2 暫定裁決同款理由);核心 crate 零外部依賴。
3. **命令面=兩層 registry**:實現層極小命令集(M0=5 命令:read_rtlil/
   write_rtlil -readable/write_json/check/stat 無參)+ 登記層 `yosys -H` 普查
   (306 條,逐字入 crates/ry-cli/src/registry.rs + corpus/ys-e2e/PROVENANCE);
   普查外名字逐字重現 oracle `No such command: %s (type 'help' for a command
   overview)` 錯誤;登記層命中=`ERROR: <cmd>: not implemented in ryosys
   (milestone Yx)` 硬錯(Y 碼映射:hierarchy/simplemap/read_json→Y0[M1 兌現]/
   proc\*+opt\*→Y2[M2]/read_verilog 族→Y3/其餘→Y0)。**`-p` 切分契約=token
   尾分號**(register.cc Pass::call 詞法循環逐字移植;無空格暗角形入 fixture)。
4. **golden=write_rtlil -readable 傳統全名形**;twines/$priv@/$pub@ 硬錯(Y1)
   ——$priv@ 在 oracle 是靜默錯名,猜=假綠。
5. **歸一化契約=結構性切+前綴精確丟行**(stdout prologue 至 `-- Running
   command ` 前、epilogue 自 `End of script. Logfile hash:` 起,末尾換行分裂
   空元素非內容行;.il 首行 Generated-by;JSON creator),敵意 fixture 單測釘死
   (crates/ry-difftest/tests/normalize.rs);機器相關字段分歧(D-ys1..4)先登記
   docs/REGISTRY.md 再繼續;stderr 零歸一化(成功路徑兩端必空=診斷漏網檢測)。
6. **迭代序契約**(hashlib 實測):dict/pool 正向迭代=逆插入序(check/stat 的
   模組/cell/wire 迭代與警告序按此);write_rtlil dump=插入序;write_json 先
   design->sort()(名序,lt_by_name 逐 byte)——兩出口順序**禁互相推導**。
7. **顯式排除**:selection 機制、-c/-s 腳本文件、TCL/Python、插件、equiv/sat/
   formal、stat 選項面(-liberty/-tech/-width)、write_rtlil 默認壓縮形。

## 後果

正:M0 縱切小而全(≈960 行 parser 的等價移植面),roundtrip 冪等是免費差分閘;
命令面按弧生長,硬錯防靜默跑偏;M0 收口即 ys-e2e **141/141**(87 P-free+54
P-bearing,P-bearing 的 exit=1 process 硬錯本身入庫為 M2 翻轉閘)。

負:golden 釘在 0.70+179 移動目標(升級=全量重錄+churn 檢視);write_rtlil 默認
形分歧直到 Y1 兌現;Verilog 直入能力後置(Y3)——CLI 用戶面早期窄;二進制與
v0.70 源碼的行為分歧(stat 表頭形、boxed 模組靜默跳過、`Warnings:` 彙總行)已
按二進制釘死,源碼行號引用僅作文脈。
