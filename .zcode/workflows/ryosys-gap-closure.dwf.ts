/* zcode-workflow
description: ryosys known-gap 续弧收平弧:基线三闸→源码对齐尝试(拿不到则以 v0.70 源码+二进制实跑仲裁)→歸類員把
  known-gap 按根因形态分组→每组一个独立源码对读员(logic-diff/internal-order/mixed
  可复刻性分类)→實作員收平循环(golden 禁改、regress.sh 精确数与名单同轮同步)→對抗審≤3轮(oracle
  抽验)→換人複審→總閘≤3轮。M2 五例(2026-10-30 一轮全收平)与 M3 31 例(-pbo)两次实录过的结构。
args:
  cases:
    type: string
    description: 逗号分隔的 known-gap 案例名(须与 scripts/regress.sh 执法名单逐字一致);留空=由歸類員从
      regress.sh 执法块自动解析
    required: false
    default: ""
  envDeveloperDir:
    type: string
    description: 总闸 regress 红灯时的 env DEVELOPER_DIR 诊断对照前缀;闸本身=裸跑 regress.sh
    required: false
    default: /Library/Developer/CommandLineTools
  planDoc:
    type: string
    description: 正典文档路径:续弧指南/收口节所在,对读员与實作員全程引用
    required: false
    default: docs/RYOSYS-MVP.md
*/
interface GapGroup {
  /** 形态名(与文档分类一致,如 D-ys8 纯序差) */
  form: string;
  /** 该形态覆盖的案例名(与 regress.sh 执法名单逐字一致) */
  cases: string[];
  /** 文档已载根因现状一句话(带出处) */
  priorRootCause: string;
}

interface FormPlan {
  /** 形态名 */
  form: string;
  /** logic-diff=可复刻修我方 pass;internal-order=登记分歧;mixed=部分可复刻 */
  class: "logic-diff" | "internal-order" | "mixed";
  /** oracle 机制(源码级,带 文件:行) */
  mechanism: string;
  /** 组级修法(函数/行为级);internal-order 则为登记要点 */
  fixPlan: string;
  /** 本形态预期覆盖的案例 */
  covers: string[];
  /** 不符本形态预期的例外案例及说明 */
  exceptions: { case: string; note: string }[];
  /** 证据:实跑输出摘录+源码位置 */
  evidence: string;
}

interface RoundStatus {
  /** 全部 logic-diff 收平且 internal-order 登记完成、闸名单同步一致 */
  done: boolean;
  /** 一段话:本轮做了什么 */
  did: string;
  /** 本轮新收平(byte-exact 对上既有 golden)的案例 */
  flipped: string[];
  /** 本轮登记为注册分歧的案例 */
  registered: string[];
  /** 尚未收口案例 */
  remaining: string[];
}

interface Review {
  approved: boolean;
  issues: string[];
}

interface Finding {
  /** 位置:文件路径或说明 */
  where: string;
  /** 一句话:发现了什么 */
  what: string;
  /** 证据:读了什么、跑了什么 */
  evidence: string;
  /** verified=已被机械闸或独立复核证实;unconfirmed=未经证实 */
  status: "verified" | "unconfirmed";
  /** high 留给崩溃/错误结果/数据损失 */
  severity: "low" | "medium" | "high";
}

interface WorkflowReport {
  /** 直接回答:收平几例、登记几例、闸是否全绿 */
  conclusion: string;
  findings: Finding[];
  /** 本次 run 实际检查了什么、怎么检查的 */
  verified: string[];
  /** 没看/没法看的部分及原因 */
  notCovered: string[];
}

// world.run 命令名须编译期字面量,oracle 绝对路径硬编码;换机器须改常量。
const YOSYS = "/Users/huangziyu/proj/rust-cad-suite/vendor-reference/suite-dl/oss-cad-suite/bin/yosys";
const ORACLE_SHA = "9534a7f4d";
const REGRESS_ENV_DIR = String(args.envDeveloperDir ?? "/Library/Developer/CommandLineTools");
const PLAN_DOC = String(args.planDoc ?? "docs/RYOSYS-MVP.md");
const CASES_ARG = String(args.cases ?? "").split(",").map((s) => s.trim()).filter((s) => s.length > 0);

artifact.board("progress", {
  title: "ryosys known-gap 续弧收平进度",
  key: "stage",
  status: "status",
  columns: ["进行中", "完成", "受阻"],
  detail: [{ field: "note" }],
});

function brief(out: { stdout: string; stderr: string }): string {
  const text = (out.stdout + "\n" + out.stderr).trim();
  return text.length > 1200 ? text.slice(0, 600) + " ……(中略)…… " + text.slice(-600) : text;
}

function reportStage(stage: string, status: string, note: string) {
  report({ stage: stage, status: status, note: note }, "progress");
}

phase("盘点基线:三闸");
const baseBuild = await world.run("cargo", ["build", "--workspace"], { timeoutMs: 900000 });
const baseTest = await world.run("cargo", ["test", "--workspace"], { timeoutMs: 1800000 });
const baseRegress = await world.run("bash", ["scripts/regress.sh"], { timeoutMs: 1800000 });
const baseOk = baseBuild.exitCode === 0 && !/warning/i.test(baseBuild.stderr + baseBuild.stdout) && baseTest.exitCode === 0 && baseRegress.exitCode === 0;
reportStage("基線", baseOk ? "完成" : "受阻", "build/test/裸跑regress " + (baseOk ? "全绿" : "红——實作員先修到绿再续弧"));
log("基线: build=" + baseBuild.exitCode + " test=" + baseTest.exitCode + " regress=" + baseRegress.exitCode);

phase("源码对齐尝试");
await world.run("git", ["-C", "vendor-reference/yosys", "fetch", "origin"], { timeoutMs: 600000 });
const shaCheck = await world.run("git", ["-C", "vendor-reference/yosys", "cat-file", "-t", ORACLE_SHA]);
let srcAligned = false;
if (shaCheck.exitCode === 0) {
  const co = await world.run("git", ["-C", "vendor-reference/yosys", "checkout", ORACLE_SHA]);
  srcAligned = co.exitCode === 0;
}
const alignNote = srcAligned
  ? "克隆已 checkout 到 " + ORACLE_SHA + "(与 oracle 二进制同源,源码机制可信)"
  : "无法对齐到 " + ORACLE_SHA + "(fetch 失败或远端无此 commit);以现有克隆源码为准读,所有序差结论必须以二进制实跑仲裁并在 evidence 标注";
reportStage("源碼對齊", srcAligned ? "完成" : "受阻", alignNote);
log("源码对齐: " + alignNote);

phase("归类:known-gap 分形态");
const grouper = agent("歸類員", { system: "你是 ryosys 的 known-gap 歸類員,只读:禁止编辑任何文件,可用 Bash 跑只读命令(含 ry-difftest)。职责:把收口弧留下的 known-gap 案例按文档已载的根因形态分组,分组必须与执法名单逐字一致、不漏不重;不确定归属的单独成组并注明。全程繁体中文;指令与现实矛盾就如实上报。" });
const groups = await grouper.ask<GapGroup[]>(
  "把本轮 known-gap 案例按根因形态分组。" + (CASES_ARG.length > 0 ? "案例名单(入参,逐字):" + CASES_ARG.join(",") : "案例名单从 scripts/regress.sh 的 ryosys 执法块(known-gap 名单与精确数行)解析,以脚本为准逐字提取") + "。依据:读 " + PLAN_DOC + " 最新续弧/收口节、corpus/ys-e2e/PROVENANCE 的对应普查与补记节、docs/REGISTRY.md 的 D-ys 分歧行。按文档已载分类分组(如 M3 的三形 D-ys8 纯序差/D-ys8b update_assign_map/opt_merge 级联;文档分类已演化则以文档最新为准);每组 priorRootCause 一句话带出处(文件:行)。只读。",
);
let totalCases = 0;
for (const g of groups) {
  totalCases = totalCases + g.cases.length;
  reportStage("歸類·" + g.form, "完成", g.cases.length + " 例 | " + g.priorRootCause.slice(0, 130));
}
log("分组 " + groups.length + " 组,共 " + totalCases + " 例");

phase("形态对读与可复刻性分类");
// join:實作員需要全部形态的 fixPlan 才能排序分派
const plans = await Promise.all(
  groups.map((g) =>
    agent(`源碼對讀·${g.form}`, { system: "你是 ryosys 的 oracle 源码对读员,只读:禁止编辑任何文件;可用 Bash 跑只读命令与本地 yosys oracle(" + YOSYS + ")实证。职责:验证一个根因形态在当前代码上是否仍成立,归因到 oracle 源码机制,并做可复刻性分类。纪律:oracle 即 spec,二进制仲裁字节,源码只解释机制;internal-order 必须给源码级不可复刻证据(如 interning 序/全局静态状态),对照 docs/REGISTRY.md 既有 D-ys 分歧先例;证据不足就如实降级并注明。全程繁体中文;指令与现实矛盾就如实上报。" }).ask<FormPlan>(
      "形态「" + g.form + "」,覆盖案例:" + g.cases.join(",") + "。文档已载根因:" + g.priorRootCause + "。步骤:①读 " + PLAN_DOC + " 对应续弧节与 corpus/ys-e2e/PROVENANCE 对应补记;②抽 2-3 个代表案例,重跑 oracle(" + YOSYS + ") 与我方 ryosys(crates/ry-cli,bin 名 ryosys)对比 diff,确认形态仍成立;③定点读源码:" + alignNote + "。对应文件按形态机制选读(passes/proc/、passes/opt/opt_expr.cc、kernel/);④分类 logic-diff(组级 fixPlan 到函数/行为级)/internal-order(给不可复刻源码证据)/mixed(拆开说明);⑤列 exceptions(不符本形态预期的案例及去向)。返回 FormPlan:mechanism/fixPlan/evidence 全部带 文件:行 或实跑输出摘录。",
    ),
  ),
);
const logicForms = plans.filter((p) => p.class !== "internal-order");
for (const p of plans) {
  reportStage("對讀·" + p.form, "完成", p.class + " | " + p.mechanism.slice(0, 140));
}
log("分类: logic-diff/mixed=[" + logicForms.map((p) => p.form).join(",") + "] internal-order=[" + plans.filter((p) => p.class === "internal-order").map((p) => p.form).join(",") + "]");
const plansForImpl = plans.map((p) => ({ form: p.form, class: p.class, mechanism: p.mechanism, fixPlan: p.fixPlan, covers: p.covers, exceptions: p.exceptions }));

phase("实作收平循环");
const impl = agent("實作員", { system: "你是 rust-cad-suite 的实现工程师,负责 ryosys known-gap 续弧收平。铁则:cargo build 零警告;golden 是 oracle 真值禁改(唯一例外=发现 golden 与当前二进制实跑不一致,停下如实上报);未确认机制不动手;scripts/regress.sh 的 ryosys 执法块(精确 SUMMARY+known-gap 名单+名单 diff 检查)必须与实跑逐字一致,任何翻绿/登记同轮同步(铁则 3);禁止任何 git add/commit;crates/riv-*、crates/vl-* 与 docs/e2e-report(机械生成)禁碰;工作树已有上一轮未提交成果,禁回退、只叠加。闸无解或指令与现实矛盾就如实上报,不要造假绕过。全程繁体中文回复。" });
let done = false;
let note = "首轮,无前次记录";
let flippedAll: string[] = [];
let registeredAll: string[] = [];
let lastRemaining: string[] = CASES_ARG.length > 0 ? CASES_ARG : [];
for (let round = 1; round <= 8; round++) {
  const step = await impl.ask<RoundStatus>(
    "续弧收平第 " + round + " 轮。目标终态:regress.sh ryosys 执法块与 difftest 实跑逐字一致且 known-gap 面收窄到可收平的最小集。形态对读结论(JSON):" + JSON.stringify(plansForImpl) + "。纪律:①logic-diff/mixed 按形态 fixPlan 修我方 crates/ry-*(只动结论指到的行为;每收平一例=byte-exact 对上既有 golden,禁改 golden);②internal-order 不硬磕——按 docs/REGISTRY.md 的 D-ys 先例登记分歧行(带源码级证据),同轮更新执法块精确数与名单;③每轮先盘点再动手,只补缺口。上一轮:" + note + "。完成后自查;全部收平/登记完成且执法块一致时回复 done=true,如实填 flipped/registered/remaining。背景:基线三闸 " + (baseOk ? "全绿" : "红,先修到绿") + ";源码对齐:" + alignNote,
  );
  done = step.done;
  lastRemaining = step.remaining;
  flippedAll = flippedAll.concat(step.flipped);
  registeredAll = registeredAll.concat(step.registered);
  const build = await world.run("cargo", ["build", "--workspace"], { timeoutMs: 900000 });
  const buildOk = build.exitCode === 0 && !/warning/i.test(build.stderr + build.stdout);
  const test = await world.run("cargo", ["test", "--workspace"], { timeoutMs: 1800000 });
  let diffSummary = "";
  if (buildOk && test.exitCode === 0) {
    const diff = await world.run("cargo", ["run", "-q", "-p", "ry-difftest", "--", "--corpus", "corpus/ys-e2e"], { timeoutMs: 1200000 });
    diffSummary = brief(diff).slice(0, 300);
  }
  reportStage("實作·第" + round + "輪", done && buildOk && test.exitCode === 0 ? "完成" : "受阻", "flipped=[" + step.flipped.join(",") + "] registered=[" + step.registered.join(",") + "] remaining=[" + step.remaining.join(",") + "] " + diffSummary);
  if (done && buildOk && test.exitCode === 0) {
    log("收平循环完成(第 " + round + " 轮): flipped=[" + flippedAll.join(",") + "] registered=[" + registeredAll.join(",") + "]");
    break;
  }
  note = "done=" + done + "(" + step.did.slice(0, 150) + ")" + (buildOk ? "" : " build:" + brief(build)) + (test.exitCode === 0 ? "" : " test:" + brief(test)) + (diffSummary ? " difftest:" + diffSummary : "");
}

phase("独立对抗審");
const adversary = agent("對抗審", { system: "你是独立对抗审查员,只读:禁止编辑任何文件;可用 Bash 跑只读命令与本地 yosys oracle(" + YOSYS + ")实证。职责是找真问题:收平声明是否被闸证明、机制归因是否站得住、分歧登记证据是否充分、执法块是否与实跑一致。byte-exact 由闸机械裁决,你不要重跑全量 regress;没有真问题才准 approved=true。全程繁体中文;发现与现实根本矛盾就如实上报。" });
let approved = false;
let feedback = "首轮,无前次意见";
for (let round = 1; round <= 3; round++) {
  const rev = await adversary.ask<Review>(
    "第 " + round + " 轮对抗审(上轮意见:" + feedback + ")。审本轮续弧收平:①git diff HEAD 逐文件读,核对每处改动与形态对读结论的一致性(JSON):" + JSON.stringify(plansForImpl) + ";②亲跑 oracle " + YOSYS + " 与我方 ryosys 抽验至少 3 个本轮翻绿案例,核对 byte-exact(或分歧登记的证据链);③跑 cargo run -q -p ry-difftest -- --corpus corpus/ys-e2e 核对 SUMMARY 与 scripts/regress.sh 执法块的精确数、known-gap 名单逐字互恰;④golden 红线:corpus/ys-e2e 的 expected.* 若有改动=红旗,须能解释为录制漂移上报。逐条 issues 附证据;没有真问题才 approved=true。",
  );
  if (rev.approved) {
    approved = true;
    log("对抗审第 " + round + " 轮通过");
    break;
  }
  feedback = rev.issues.join("; ");
  reportStage("對抗審·第" + round + "輪", "受阻", feedback.slice(0, 200));
  await impl.ask<string>("按对抗审意见修订(直接编辑,只改必要处;改完简述):" + feedback);
}
reportStage("對抗審", approved ? "完成" : "受阻", approved ? "通过" : "三轮未全过,按现状继续,总闸兜底");

phase("換人複審");
const fresh = await agent("換人複審", { system: "你是换人复审员:从未参与之前的讨论与修订。只读,禁止编辑任何文件。唯一问题:一位没参与讨论的工程师接手这批改动,会在哪里卡住?全程繁体中文。" }).ask<Review>(
  "只凭 git diff HEAD、" + PLAN_DOC + " 最新续弧节与 docs/REGISTRY.md 新增行,列出自相矛盾、不可验收、照做必卡住之处;没有就 approved=true。",
);
if (!fresh.approved) {
  reportStage("換人複審", "受阻", fresh.issues.join("; ").slice(0, 200));
  await impl.ask<string>("换人复审提出以下问题,做最后一轮修订(直接编辑,只改必要处):" + fresh.issues.join("; "));
} else {
  reportStage("換人複審", "完成", "通过");
}

phase("总闸与修复循环");
let finalOk = false;
let fixNote = "无";
for (let round = 1; round <= 3; round++) {
  const build = await world.run("cargo", ["build", "--workspace"], { timeoutMs: 900000 });
  const test = await world.run("cargo", ["test", "--workspace"], { timeoutMs: 1800000 });
  const regress = await world.run("bash", ["scripts/regress.sh"], { timeoutMs: 1800000 });
  const buildOk = build.exitCode === 0 && !/warning/i.test(build.stderr + build.stdout);
  if (buildOk && test.exitCode === 0 && regress.exitCode === 0) {
    finalOk = true;
    reportStage("總閘", "完成", "build 零警告 / 全仓测试 / 裸跑 regress.sh 全绿");
    log("总闸全绿(第 " + round + " 轮)");
    break;
  }
  let diag = "";
  if (regress.exitCode !== 0) {
    const diagRun = await world.run("env", ["DEVELOPER_DIR=" + REGRESS_ENV_DIR, "bash", "scripts/regress.sh"], { timeoutMs: 1800000 });
    diag = diagRun.exitCode === 0
      ? "诊断:env DEVELOPER_DIR=CLT 下 PASS→机器工具链漂移(检查 regress.sh 防御对齐触发条件)"
      : "诊断:env 前缀也红→代码问题:" + brief(diagRun);
  }
  fixNote = [
    buildOk ? null : "build:" + brief(build),
    test.exitCode === 0 ? null : "test:" + brief(test),
    regress.exitCode === 0 ? null : "regress:" + brief(regress) + "\n" + diag,
  ].filter((x) => x !== null).join("\n");
  reportStage("總閘·第" + round + "輪", "受阻", fixNote.slice(0, 200));
  await impl.ask<string>("总闸失败(第 " + round + " 轮),修复:" + fixNote + "\n边界:只修 ry-* 系列与本轮新增文档/闸改动;其余既有失败如实报告但不要动。");
}
const st = await git.status();

await artifact.file("plan", PLAN_DOC, {
  title: "ryosys 续弧收平成果",
  description: "known-gap 形态对读分类、收平/登记终态与执法块同步记录。",
  primary: true,
});

const findings: Finding[] = [];
if (!baseOk) {
  findings.push({ where: "workspace(基线)", what: "盘点基线非绿", evidence: brief(baseBuild) + " | " + brief(baseTest) + " | " + brief(baseRegress), status: "verified", severity: "medium" });
}
if (!srcAligned) {
  findings.push({ where: "vendor-reference/yosys", what: "源码克隆未能对齐到 " + ORACLE_SHA + ",对读以现有克隆为准、序差结论以二进制实跑仲裁", evidence: alignNote, status: "verified", severity: "low" });
}
const orderPlans = plans.filter((p) => p.class === "internal-order");
for (const p of orderPlans) {
  findings.push({ where: "docs/REGISTRY.md", what: "形态「" + p.form + "」internal-order 注册分歧:" + p.mechanism.slice(0, 180), evidence: p.evidence.slice(0, 350), status: approved ? "verified" : "unconfirmed", severity: "low" });
}
const stillOpen = lastRemaining.filter((c) => flippedAll.indexOf(c) === -1 && registeredAll.indexOf(c) === -1);
if (stillOpen.length > 0) {
  findings.push({ where: "crates/ry-*", what: stillOpen.length + " 例未收平(既非翻绿也非登记):" + stillOpen.join(","), evidence: "实作循环末轮 remaining;总闸 " + (finalOk ? "绿(名单已同步)" : "状态见总闸条"), status: "verified", severity: finalOk ? "low" : "high" });
}
if (!approved) {
  findings.push({ where: "workspace", what: "对抗审三轮未完全通过,按最后一轮修订继续", evidence: "对抗审循环记录", status: "verified", severity: "medium" });
}
if (!fresh.approved) {
  findings.push({ where: PLAN_DOC, what: "换人复审提出问题后做了最后一轮修订(未再复审)", evidence: fresh.issues.join("; ").slice(0, 400), status: "verified", severity: "low" });
}
if (!finalOk) {
  findings.push({ where: "workspace", what: "总闸 3 轮未全绿:build/test/regress 至少一项失败", evidence: fixNote.slice(0, 500), status: "verified", severity: "high" });
}
const result: WorkflowReport = {
  conclusion: "known-gap 续弧(" + totalCases + " 例):收平 " + flippedAll.length + " 例(" + flippedAll.join(",") + "),登记 internal-order 分歧 " + registeredAll.length + " 例(" + registeredAll.join(",") + ")," + (stillOpen.length > 0 ? "仍有 " + stillOpen.length + " 例未收口(" + stillOpen.join(",") + ")," : "") + (finalOk ? "总闸全绿(build 零警告/全仓测试/裸跑 regress.sh,执法块同轮同步)" : "总闸未全绿,见 findings") + "。工作树呈现全部改动未提交(git status " + (st.clean ? "clean" : "含未提交改动") + "),commit 由你定。",
  findings: findings,
  verified: [
    "基线盘点:cargo build --workspace + cargo test --workspace + 裸跑 bash scripts/regress.sh 各一次",
    "源码对齐:git fetch + checkout " + ORACLE_SHA + " 尝试(结果见 alignNote/finding)",
    "每形态由独立对读员带 oracle 实证做机制验证与可复刻性分类",
    "每轮实作闸:cargo build --workspace(零警告)+ cargo test --workspace + ry-difftest 实跑 SUMMARY",
    "对抗审(oracle 抽验至少 3 例+difftest 实跑核对执法块)+ 换人复审",
    "总闸:cargo build 零警告 + cargo test --workspace + 裸跑 bash scripts/regress.sh" + (finalOk ? "(全绿)" : "(未全绿,含 env DEVELOPER_DIR 诊断对照)") + "。",
  ],
  notCovered: [
    srcAligned ? [] : ["源码与二进制的 commit 差异面未逐 commit 对账(对读结论已按二进制实跑仲裁)"],
    "未抽验案例的 byte-exact 由 difftest 全量闸覆盖,本报告不逐例复述",
    "wasm/FFI 面未接(既有闸不受影响)",
  ].flat(),
};
return result;