---
name: rtl-rulebook
description: 從 RTL 提取可檢驗的行為規則書(reset/fsm/timing/arithmetic/counter 等類別,每條帶源碼溯源與機械可判 check)——規則書、spec 提取、行為規則時使用;不是把 RTL 機械轉成文件
---

# 規則書提取技能(rtl_extract_rules)

## 觸發詞

規則書/spec 提取/行為規則/「這段 RTL 的行為契約是什麼」。

## 流程

1. 呼叫 MCP 工具 `rtl_extract_rules`(`path`|`source`/`lang`/`format`
   json|markdown|both 默認 both/`max_rules_per_module` 默認 128)。
2. **自查資料健康**:structuredContent.rulebook 的 parse_errors 與
   skipped(tb_file=測試平台檔,不是提取對象)、truncated 非空=該模組
   規則被截斷(注水防護有閘性)。
3. 按 check 五形讀規則:guard(觸發條件→賦值)/transition(FSM 遷移)/
   sequence(錨點後第 N 拍)/relation(組合恆等)/invariant(值域/恆等式)。
4. 產出按類別歸併的規則書敘事(reset/fsm/timing/arithmetic/counter/…)

## 「規則書不是文件轉寫」三鎖讀法

1. **生成端鎖**:規則只由行為構造 spawning(always/initial/assign/case 臂/
   generate 分支/RTLIL process);純宣告(埠/線/參數列表)永不單獨成規則。
   每條規則 evidence 必含 ≥1 行為類(process|assign|case|generate|expr)。
2. **資料端鎖**:check.form∈五形且專屬欄位非空;fingerprint=拼接鍵串
   (source|category|module|subject|when|effects),同指紋=生成器 bug。
3. **閘端鎖**:snippet 與源行逐位元組對拍(V4;SV 側 100% 帶 file:line)。

**qualifier 差異化信任度**:`derived`=語法結構直讀可證(直接引用);
`inferred`=模式推斷(引用前先人工核)。

## 要點

- 陳述=中文模板+源碼標識符原文名;表達式欄位=源碼渲染文本(不改寫
  不求值)——「可對回源碼」是可檢驗性的根。
- RTLIL 側規則=proc 後正典形子集(reset/fsm/timing),evidence=結構路徑
  (file,line=0,path);oracle 臨時名(`$0\wire`、`$add$2_Y`)在場屬正常。
- 反 side-channel:若規則書讀起來像宣告表/語法清單(doc-dump),那是
  生成端鎖被破壞——停止使用並回報,勿在其上做下游。

## 契約位置

正典=docs/RTL2SPEC-PLUGIN.md §2(schema/三鎖/五形詞表);機械判據
V1..V6=§8.3;ADR-0012。
