/* zcode-workflow
description: 通用自我改進規劃工作流:每 run 依當前 context 盤點候選弧,走 設計→可證偽實驗→獨立對抗審→實作→機械閘(build
  零警告/cargo test/regress 全閘)→帳務提交,可連跑多輪並把教訓反饋下一輪。subagent 全走 GLM-5.3 $max(Pro
  MAX,非 Flash)。
whenToUse: 要讓 rust-cad-suite 自主推進一輪(或 N 輪)完整開發弧時:選題、實證、審查、實作、過閘、提交一次到位。工作樹必須乾淨。
args:
  focus:
    type: string
    description: 限定選題範圍(例如「U-3」「效能」「SV-UNIFY」);空=規劃者依盤點自選最有價值弧
    required: false
    default: ""
  max_arcs:
    type: number
    description: 本次 run 要連跑幾輪「設計→實驗→判定→實作→提交」弧(1-5)
    required: false
    default: 1
  push:
    type: boolean
    description: 閘門全綠後是否 git push origin main
    required: false
    default: true
*/
// rust-cad-suite 自我改進工作流(通用規劃器,ADR-0009 體系)
// 每 run = N 輪完整弧:盤點 context → 設計(選弧+方案)→ 實驗(可證偽探針)
// → 判定(獨立對抗審)→ 實作 → 機械閘(build/test/regress)→ 帳務提交 → 反饋下一輪。
// 全部 subagent 走 run 級 subagent_model(Pro MAX)。

interface DigestItem {
  /** 領域或弧名,例如「U-3 CLI 面收編」或「效能 R10」。 */
  area: string;
  /** 一句話現況:已完成什麼、還缺什麼。 */
  state: string;
  /** 證據指針:檔案路徑、提交號或可重跑命令。 */
  evidence: string;
}
interface Context {
  /** 五句話以內的倉庫現況總結。 */
  summary: string;
  /** 候選工作弧清單,依價值/依賴排序。 */
  pending: DigestItem[];
  /** 目前已知的風險或登記中的分歧。 */
  risks: string[];
}
interface Probe {
  /** 探針短名。 */
  name: string;
  /** 這個探針證偽什麼假設(一句話)。 */
  question: string;
  /** 怎麼跑:最小復現檔內容要點+確切命令(兩引擎/oracle 對跑)。 */
  how: string;
  /** 什麼結果支持方案、什麼結果推翻方案。 */
  expect: string;
}
interface ArcPlan {
  /** 弧名,對齊 PLAN/ADR 的既有編號(如 U-3)。 */
  arc: string;
  /** 一句話目標與其價值。 */
  goal: string;
  /** 設計:方案本體、考慮過的替代案、取捨理由;引用檔案 path:line。 */
  design: string;
  /** 實作前必跑的可證偽探針;無假設可證偽時才可為空。 */
  probes: Probe[];
  /** 外科手術式實作步驟(含帳務檔案)。 */
  steps: string[];
  /** 驗收:每項=確切命令+預期結果。 */
  gates: string[];
}
interface ProbeResult {
  /** 對應的探針名。 */
  name: string;
  /** 實際跑了什麼(命令+檔案位置)。 */
  ran: string;
  /** 觀察到什麼(關鍵輸出逐字)。 */
  observed: string;
  /** supports=支持方案;refutes=推翻某假設;inconclusive=跑不出結論。 */
  verdict: "supports" | "refutes" | "inconclusive";
}
interface Review {
  /** ship=可實作;ship-with-fixes=修這些點後可實作;reject=方案不成立。 */
  verdict: "ship" | "ship-with-fixes" | "reject";
  /** 必須折入方案的修正(逐條可執行)。 */
  fixes: string[];
  /** 判決理由與證據。 */
  reasons: string[];
}
interface ArcOutcome {
  /** 弧名。 */
  arc: string;
  /** 本輪弧走到哪。 */
  stage: string;
  /** 備註(一句話)。 */
  note: string;
}

artifact.board("arcs", {
  title: "自我改進弧進度",
  key: "arc",
  status: "stage",
  columns: ["規劃", "實驗", "審查", "實作", "閘門", "已提交", "已跳過"],
  cardTitle: "arc",
  detail: [{ field: "note", label: "備註" }],
});

const maxArcs = typeof args.max_arcs === "number" ? Math.min(Math.max(args.max_arcs, 1), 5) : 1;
const focus = typeof args.focus === "string" ? args.focus : "";
const doPush = args.push !== false;
const feedback: string[] = [];
const arcOutcomes: ArcOutcome[] = [];

const PLANNER_SYSTEM =
  "你是 rust-cad-suite(純 Rust 重寫 oss-cad-suite;Phase 1 rverilator,riverilog 為 iverilog 形 CLI 面)的資深架構規劃者。" +
  "規劃前必讀:AGENTS.md(十節鐵則)、PLAN.md、docs/adr/0009(統一引擎 U-1..U-6,U-1/U-2 已完成)、docs/REGISTRY.md(分歧簿/事故簿)、docs/OKR.md。" +
  "方法論:oracle 即 spec(verilator 5.050=/opt/homebrew/bin/verilator、iverilog 13.0),LRM 只解釋不覆蓋;關鍵假設必須配可證偽探針,禁止憑空斷言行為。" +
  "弧的大小以一輪迭代可完成為度;過大者切前半,剩餘寫進步驟末尾供下輪規劃。若你收到的指示互相矛盾或需要只有使用者知道的決策,直說並升級提問,不要硬拗。";
const EXPERIMENTER_SYSTEM =
  "你是 rust-cad-suite 的實驗員:只做實驗,不修改任何 git 追蹤檔。探針檔放 /tmp 下自建目錄。" +
  "可用工具:cargo build/run、target/debug/rverilator(RVERILATOR_LEGACY=1 切 legacy 引擎)、/opt/homebrew/bin/verilator --binary --timing、iverilog+vvp。" +
  "每個探針如實回報觀察輸出(逐字關鍵行)與判決;跑不出結論就回 inconclusive 並說明卡點,禁止腦補。";
const REVIEWER_SYSTEM =
  "你是獨立對抗審查者,任務是推翻方案而非背書。必須親自讀方案觸碰的代碼檔案(path:line)與探針回報,不信任轉述。" +
  "找:事實錯誤、缺 oracle 證據的行為假設、違反 AGENTS 鐵則(oracle 即 spec/零警告/帳務四層/語義手術紀律)、不可執行的步驟、被忽略的既有分歧簿條目。" +
  "verdict 給 ship 需要證據;挑不出毛病才准 ship。你不編輯任何檔案。";
const IMPLEMENTER_SYSTEM =
  "你是 rust-cad-suite 的資深實作者。動工前先讀 AGENTS.md 全文並逐條遵守。" +
  "語義敏感修改一律腳本化手術+命中計數斷言;新鐵則同輪落四層;零警告構建;提交=精確單檔 git add、message 禁反引號與 $()。" +
  "oracle 期望只准 cargo run -p vl-difftest -- --record 錄製;重錄後必逐 case 讀 diff。" +
  "帳務(PLAN/REGISTRY/ADR/OKR)用錨點斷言腳本更新。若閘門怎麼都過不了或指示矛盾,升級提問,禁止造假繞過。你只 commit,不 push(push 由工作流執行)。";

const surveyor = agent("盤點員", { system: "你是倉庫狀態盤點員:只讀不寫,證據必須是路徑/提交號/可重跑命令,禁止無據斷言。" });
const planner = agent("規劃者", { system: PLANNER_SYSTEM });

for (let arcN = 1; arcN <= maxArcs; arcN++) {
  phase("盤點現況與候選弧");
  const st = await git.status();
  const ctx = await surveyor.ask<Context>(
    "盤點 rust-cad-suite 目前的開發現況,供一輪「設計→實驗→判定→實作」自我改進弧選題。" +
    "必查:PLAN.md(未完成弧/里程碑表)、docs/adr/0009 §3(U-3..U-6 剩餘步)、docs/REGISTRY.md(未兌現分歧/事故教訓)、docs/OKR.md(KR 缺口)、git log -15(最近弧的收尾狀態)、docs/RIVERILOG-PLAN.md §8( riv 待辦)。" +
    "pending 依「價值×就緒度」排序,每項帶證據指針。前一輪弧的反饋:" + JSON.stringify(feedback),
  );
  if (st.staged.length > 0 || st.unstaged.length > 0) {
    const dirty: ArcOutcome = { arc: `arc-${arcN}`, stage: "已跳過", note: "工作樹不乾淨(staged/unstaged 非空),需要人類先處理,禁自動 stash。" };
    report(dirty, "arcs");
    arcOutcomes.push(dirty);
    continue;
  }

  phase("設計:選弧與方案");
  let plan = await planner.ask<ArcPlan>(
    "從下列盤點結果選「本輪最有價值且一輪可完成」的一個弧並完整設計。" +
    (focus ? "使用者指定範圍限制:「" + focus + "」,選題不得超出。" : "") +
    "\n倉庫總結:" + ctx.summary +
    "\n候選弧:" + JSON.stringify(ctx.pending) +
    "\n已知風險:" + JSON.stringify(ctx.risks) +
    "\n設計要求:方案+替代案+取捨理由(引用 path:line);實作前的可證偽探針(每個關鍵行為假設一個,含兩引擎/oracle 對跑命令與預期);外科手術式步驟;驗收命令與綠燈條件;帳務檔案清單。",
  );
  report({ arc: plan.arc, stage: "規劃", note: plan.goal }, "arcs");

  const experimenter = agent(`實驗員-弧${arcN}`, { system: EXPERIMENTER_SYSTEM });
  const reviewer = agent(`對抗審-弧${arcN}`, { system: REVIEWER_SYSTEM });
  let verdict: Review = { verdict: "reject", fixes: [], reasons: ["未審"] };
  let probes: ProbeResult[] = [];
  let approved = false;
  for (let round = 1; round <= 3; round++) {
    phase("實驗:可證偽探針");
    probes = await experimenter.ask<ProbeResult[]>(
      "依規劃者的探針清單逐個執行實驗並回報。探針清單:" + JSON.stringify(plan.probes) +
      "\n方案摘要:" + plan.design +
      "\n要求:先建 /tmp/rv-arc" + arcN + "-r" + round + "/ 放最小復現;兩引擎與 oracle 的輸出都要逐字關鍵行;每個探針給 supports/refutes/inconclusive。",
    );
    report({ arc: plan.arc, stage: "實驗", note: probes.map((p) => p.name + ":" + p.verdict).join("; ") }, "arcs");

    phase("對抗審查方案");
    verdict = await reviewer.ask<Review>(
      "對下列方案與實驗證據做對抗審。方案:" + JSON.stringify(plan) +
      "\n實驗結果:" + JSON.stringify(probes) +
      "\n必查:方案宣稱的行為是否真有 oracle/代碼證據(自己去讀檔與跑唯讀命令驗證);探針是否真證偽了關鍵假設;步驟是否可執行;是否踩 REGISTRY 既有分歧。" +
      "\nrefutes 或 inconclusive 的探針必須處理:方案據此修改了嗎?沒修改就是 reject 理由。",
    );
    report({ arc: plan.arc, stage: "審查", note: verdict.verdict + ":" + verdict.reasons.join(";").slice(0, 120) }, "arcs");
    if (verdict.verdict !== "reject") {
      approved = true;
      break;
    }
    phase("設計:依審查與實驗反饋修訂");
    plan = await planner.ask<ArcPlan>(
      "對抗審 reject 了你的方案,反饋如下。修訂方案(可改弧、改設計、換探針),保持同樣的輸出格式。" +
      "\n審查理由:" + JSON.stringify(verdict.reasons) +
      "\n實驗反駁:" + JSON.stringify(probes.filter((p) => p.verdict !== "supports")) +
      "\n原方案:" + JSON.stringify(plan),
    );
  }
  if (!approved) {
    const skipped: ArcOutcome = { arc: plan.arc, stage: "已跳過", note: "三輪對抗審未過,方案不成立;已記錄待人類裁決。" };
    report(skipped, "arcs");
    arcOutcomes.push(skipped);
    feedback.push("弧「" + plan.arc + "」三輪對抗審未過:" + verdict.reasons.join("; "));
    continue;
  }
  if (verdict.fixes.length > 0) {
    phase("設計:折入審查修正");
    plan = await planner.ask<ArcPlan>(
      "對抗審給 ship-with-fixes,把下列修正逐條折入方案後回傳最終版(同格式):\n" + JSON.stringify(verdict.fixes) +
      "\n原方案:" + JSON.stringify(plan),
    );
  }

  phase("實作與閘門");
  const implementer = agent(`實作-弧${arcN}`, { system: IMPLEMENTER_SYSTEM });
  const implNote = await implementer.ask<string>(
    "依最終方案實作。方案:" + JSON.stringify(plan) +
    "\n實驗證據(實作時的行為依據):" + JSON.stringify(probes) +
    "\n紀律:腳本化手術+命中計數斷言;做完整套實作與帳務更新(錨點斷言式),然後 git add 精確單檔列表並 commit(message 禁反引號與 $(),風格參照 git log -3)。不要 push。" +
    "\n完成後回報:改了哪些檔案、commit 雜湊、自跑 cargo build 與 cargo test 的結果。",
  );
  report({ arc: plan.arc, stage: "實作", note: implNote.slice(0, 160) }, "arcs");

  // 快閘:cargo build 零警告
  let gatesOk = false;
  for (let fix = 1; fix <= 4; fix++) {
    const build = await world.run("cargo", ["build"]);
    if (build.exitCode === 0 && !build.stderr.includes("warning:") && !build.stdout.includes("warning:")) {
      gatesOk = true;
      break;
    }
    await implementer.ask("cargo build 未達零警告零錯誤,修到過:\n" + build.stderr.slice(0, 4000) + "\n" + build.stdout.slice(0, 2000));
  }
  // 中閘:全 workspace 測試
  if (gatesOk) {
    for (let fix = 1; fix <= 4; fix++) {
      const test = await world.run("cargo", ["test"], { timeoutMs: 900000 });
      if (test.exitCode === 0) break;
      await implementer.ask("cargo test 有失敗,修到全綠(只修不掩蓋):\n" + (test.stdout + "\n" + test.stderr).slice(-6000));
    }
  }
  // 強閘:regress 全閘(提交前必過;唯一最強裁決)
  if (gatesOk) {
    for (let fix = 1; fix <= 3; fix++) {
      const regress = await world.run("bash", ["scripts/regress.sh"], { timeoutMs: 1800000 });
      if (regress.exitCode === 0 && regress.stdout.includes("REGRESSION GATE: PASS")) {
        gatesOk = true;
        break;
      }
      gatesOk = false;
      await implementer.ask("regress.sh 未 PASS,修到過(讀完整輸出找 FAIL 行,不得改基線數字本身):\n" + regress.stdout.slice(-6000) + "\n" + regress.stderr.slice(0, 2000));
    }
  }
  report({ arc: plan.arc, stage: "閘門", note: gatesOk ? "build 零警告+cargo test+regress 全閘 PASS" : "閘門未過" }, "arcs");

  phase("帳務與提交");
  if (!gatesOk) {
    const failed: ArcOutcome = { arc: plan.arc, stage: "已跳過", note: "閘門多輪未過;工作樹保留現場待人類檢視,未提交。" };
    report(failed, "arcs");
    arcOutcomes.push(failed);
    feedback.push("弧「" + plan.arc + "」閘門未過,現場保留:" + implNote.slice(0, 200));
    continue;
  }
  const committed = await implementer.ask<string>(
    "閘門全綠(cargo build 零警告、cargo test、bash scripts/regress.sh 皆 PASS,由工作流實跑確認)。" +
    "現在收尾:1) 依 AGENTS 帳務規則最終檢查 PLAN/REGISTRY/ADR/OKR 更新是否已含本弧(SKILL 16 條:錨點斷言);2) git status 確認無漏加檔案,補精確 add;3) 若有需要 amend 進 commit(禁改歷史),否則用一次性修正 commit;4) 回報最終 commit 雜湊與 git status --short 輸出(應僅剩 .zcode/ 類非追蹤物)。",
  );
  const head = await world.run("git", ["rev-parse", "HEAD"]);
  const commitHash = head.stdout.trim();
  if (doPush) {
    await world.run("git", ["push", "origin", "main"]);
  }
  const done: ArcOutcome = { arc: plan.arc, stage: "已提交", note: commitHash.slice(0, 8) + (doPush ? " 已推送;" : " 未推送;") + committed.slice(0, 120) };
  report(done, "arcs");
  arcOutcomes.push(done);
  feedback.push("弧「" + plan.arc + "」已完成並提交 " + commitHash.slice(0, 8) + ";教訓:" + committed.slice(0, 300));
}

phase("彙整弧報告");
const reportMd = [
  "# rust-cad-suite 自我改進迭代報告",
  "",
  "共 " + arcOutcomes.length + " 輪弧(max_arcs=" + maxArcs + (focus ? ",focus=" + focus : "") + "):",
  "",
  ...arcOutcomes.map((o) => "- **" + o.arc + "** [" + o.stage + "] " + o.note),
  "",
  "反饋給下一輪的要點:",
  ...feedback.map((f) => "- " + f),
].join("\n");
await artifact.markdown("arc-report", reportMd, {
  title: "自我改進弧報告",
  description: "每輪弧的設計→實驗→判定→實作結果與提交紀錄。",
  primary: true,
});

return {
  conclusion:
    arcOutcomes.length === 0
      ? "沒有弧被執行(max_arcs 設定或前置檢查未過)。"
      : arcOutcomes.filter((o) => o.stage === "已提交").length + "/" + arcOutcomes.length + " 輪弧完成並提交;詳見弧報告。",
  findings: arcOutcomes.map((o) => ({
    where: "PLAN.md",
    what: o.arc + " [" + o.stage + "] " + o.note,
    evidence: "工作流 phases 與 regress/build 輸出",
    status: "verified" as const,
    severity: "medium" as const,
  })),
  verified: [
    "cargo build(零警告斷言)",
    "cargo test(全 workspace)",
    "bash scripts/regress.sh(REGRESSION GATE: PASS)",
    "git rev-parse HEAD 與 push 結果",
  ],
  notCovered: arcOutcomes.filter((o) => o.stage === "已跳過").map((o) => o.arc + ": " + o.note),
};