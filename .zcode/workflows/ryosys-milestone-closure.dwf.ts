/* zcode-workflow
description: 提炼自 run dwfrun-856f9d98-531b-4695-9693-6d28f1d644e7(「ryosys
  M1:regression 收口與下一輪開發」,总闸全绿):里程碑收口弧——总结员只读归纳上一轮解决了什么(带证据)+遗留事项;脚本实测三闸基线(含裸跑
  regress.sh);规划者把 planDoc 里程碑表下一档展开成详案(硬验收:裸跑 regress 全绿、oracle byte-exact
  对拍扩量、上一轮末轮修订交独立复核);对抗审≤3轮(上游源码级复核+oracle 实证)+换人复审;先闸后修实作循环(build
  零警告+全仓测试);总闸=裸跑 regress.sh,红时 env 前缀多跑一次做机器 vs 代码诊断。工作树呈现、不 commit。subagent
  模型随 GUI/计划设置,metadata 不构成运行时约束(2026-10-30 实测 GLM-5.3-Flash)。
whenToUse: ryosys 每个里程碑收口时:清残项、把 regression 裸跑收绿、扩 oracle byte-exact
  对拍语料、复核上一轮遗留;或作为「总结上一轮→收口→下一里程碑详案→审→实作→总闸」弧的模板。收口余 known-gap(序差类
  残差)时改走「源码定点对读→可复刻性分类→收平循环→同轮翻闸」续弧收平弧(模板=.zcode/workflow-drafts/ryosys-M2-续弧收平.dwf.ts,
  改 CASES 常量与例数/里程碑字面量即可复用;run dwfrun-95f7e568 实录 401/406→406/406)。六点须知:①oracle yosys
  二进制绝对路径硬编码在脚本 YOSYS 常量(world.run 命令名须编译期字面量),换机器先改该常量;②阶段名里的 M1 是 compile-time
  字面量,实际推进哪个里程碑由规划者读 planDoc 的里程碑表决定;③假定工作树已有上一轮未提交成果且可写,回归闸以裸跑 bash
  scripts/regress.sh 为准(需要机器已过四层工具链对齐);④长跑弧启动即挂 caffeinate(2026-10-28/10-29 两次主机休眠冻死
  ask 实录:對抗審 11h20m/實作員 3h26m);⑤ask 冻结(updated_at 小时级不动)先以 AmendWorkflow 加最小注释热替换续跑,
  phases 从缓存重放、勿重建弧——判据与手法照 workflow-drafts 两次 restart 注释实录;⑥planDoc 重入态:本弧可对同一
  planDoc 照跑多轮,规划 ask 已带重入守卫——planDoc 已含下一档未实作收口的详案章节时,规划者消费/审校既有章节、
  禁止追加第二份详章(双口径=红),对抗审/换人复审/实作员的「本轮详案」随规划者的消费/修订锚定到同一章。
args:
  planDoc:
    type: string
    description: MVP 正典候选文档路径:上一轮详案所在、本轮新章节追加的目标(总结员/规划者/对抗审/换人复审全程引用;规划者承诺不删改上一轮内容)
    required: false
    default: docs/RYOSYS-MVP.md
  adrDoc:
    type: string
    description: 上一轮 ADR 路径,总结员的收据来源之一
    required: false
    default: docs/adr/0010-ryosys-mvp-rtlil-first.md
  corpusSimple:
    type: string
    description: 语料扩量目标目录(规划 ask 中「全量收编为 golden」所指的 .v 语料)
    required: false
    default: corpus-external/yosys-simple
  envDeveloperDir:
    type: string
    description: 总闸 regress 红灯时的诊断对照前缀(env DEVELOPER_DIR=… 多跑一次,区分机器工具链漂移 vs
      代码问题);它不是闸本身,闸=裸跑 regress.sh
    required: false
    default: /Library/Developer/CommandLineTools
*/
interface RoundSummary {
  /** 上一轮(以 planDoc 里程碑表上一档为准)解决的问题:问题一句话+怎么解的一句话+证据位置 */
  solved: { problem: string; fix: string; evidence: string }[];
  /** 遗留事项:未复审的修订、未收编语料、机器问题等,每条一句话 */
  carriedOver: string[];
}

interface PlanMeta {
  /** 本轮详案所在文档(planDoc,追加章节、不删改既有内容) */
  planPath: string;
  /** 本轮里程碑范围一句话 */
  scope: string;
  /** 验收清单:每条必须可机械执行(具体命令或测试名) */
  acceptance: string[];
  /** 有序实现 slice 清单 */
  slices: string[];
}

interface Review {
  /** 是否通过 */
  approved: boolean;
  /** 真问题清单,每条一句话并附证据位置 */
  issues: string[];
}

interface ImplStep {
  /** 规划的 slices 是否已全部完成 */
  done: boolean;
  /** 一段话:本轮做了什么 */
  did: string;
}

interface GateOutcome {
  /** 闸名 */
  gate: string;
  /** 是否通过 */
  ok: boolean;
  /** 证据:退出码与输出摘录 */
  evidence: string;
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
  /** 直接回答:上一轮解决了什么;本轮做了什么、闸是否全绿 */
  conclusion: string;
  findings: Finding[];
  /** 本次 run 实际检查了什么、怎么检查的 */
  verified: string[];
  /** 没看/没法看的部分及原因 */
  notCovered: string[];
}

// world.run 的命令名必须是编译期字面量,故 oracle 绝对路径在此硬编码、不参数化;换机器须改此常量。
const YOSYS = "/Users/huangziyu/proj/rust-cad-suite/vendor-reference/suite-dl/oss-cad-suite/bin/yosys";
const REGRESS_ENV_DIR = String(args.envDeveloperDir ?? "/Library/Developer/CommandLineTools");
const PLAN_DOC = String(args.planDoc ?? "docs/RYOSYS-MVP.md");
const ADR_DOC = String(args.adrDoc ?? "docs/adr/0010-ryosys-mvp-rtlil-first.md");
const YS_CORPUS = String(args.corpusSimple ?? "corpus-external/yosys-simple");

artifact.board("progress", {
  title: "ryosys 里程碑收口进度",
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

phase("總結上一輪:解決了什麼");
const summarizer = agent("總結員", { system: "你是 rust-cad-suite 的轮次总结员,只读:禁止编辑任何文件,可用 Bash 跑只读命令。职责是把上一轮开发的收据变成可信的总结——每条结论必须带证据(文件路径/命令输出),归纳不出来的就注明。全程繁体中文回复;若指令与现实矛盾,如实回报。" });
const prev = await summarizer.ask<RoundSummary>(
  "总结 " + PLAN_DOC + " 里程碑表上一档里程碑那一轮(以文档当前状态为准)解决了什么问题。读:" + PLAN_DOC + "(里程碑表+最新收口节)、" + ADR_DOC + "。收据取法:先 git log --oneline -10 定位上一轮收口提交——已提交的轮次以提交单与 message 为收据(git show --stat <提交>);工作树仍有未提交增补时再辅以 git diff HEAD -- docs/REGISTRY.md PLAN.md AGENTS.md scripts/regress.sh .agents/skills/eda-dev/SKILL.md(未提交增补=上一轮收据)与 git status --short。solved 按「工具缺口/语义坑/流程坑」归类,每条=问题一句话+怎么解的一句话+证据位置;carriedOver 从 " + PLAN_DOC + " 最新收口节/ADR 的遗留清单与下一里程碑指针派生——只列登记在案且未清偿的项(收口节已标记完成的不涵盖),并注明各项是否已被后续提交清偿。solved 最多 12 条、carriedOver 最多 8 条。",
);
reportStage("總結上一輪", "完成", prev.solved.length + " 项问题有收据");
log("上一轮解决:" + prev.solved.map((s) => s.problem).join("; "));

phase("盤點現狀:三閘基線");
const baseBuild = await world.run("cargo", ["build", "--workspace"], { timeoutMs: 900000 });
const baseTest = await world.run("cargo", ["test", "--workspace"], { timeoutMs: 1800000 });
const baseRegress = await world.run("bash", ["scripts/regress.sh"], { timeoutMs: 1800000 });
const baseCoreOk = baseBuild.exitCode === 0 && !/warning/i.test(baseBuild.stderr + baseBuild.stdout) && baseTest.exitCode === 0;
reportStage("基線盤點", baseCoreOk && baseRegress.exitCode === 0 ? "完成" : "受阻", "build/test " + (baseCoreOk ? "绿" : "红") + ";裸跑 regress " + (baseRegress.exitCode === 0 ? "绿" : "红(预期:机器工具链漂移,待本里程碑收口)"));
log("基线:build/test " + (baseCoreOk ? "绿" : "红") + ";裸跑 regress.sh " + (baseRegress.exitCode === 0 ? "绿" : "红"));

phase("規劃下一輪:M1 詳案");
const planner = agent("規劃者", { system: "你是 ryosys 的首席规划者(沿用上一轮职责):写规划、按审查意见修订。铁则:oracle 即 spec,期望一律从 oracle 实跑录制、禁手写;未支援构造=带里程碑硬错;闸必须可机械执行;写入 docs/ 的规划用简体中文对齐 PLAN.md 风格;不执行任何 git 提交。若指令与现实矛盾,如实回报而不是硬绕。全程繁体中文回复。" });
const plan = await planner.ask<PlanMeta>(
  "把 " + PLAN_DOC + " 里程碑表的下一个里程碑(以文档为准)展开成可执行的详案,直接追加进该文档(新章节;不删改既有章节内容)。重入守卫:先读 planDoc——若已含下一档里程碑的详案章节且该档尚未实作收口(里程碑表无该档行或行未翻 ✅),则消费/审校该既有章节(按本轮硬性要求修订补充),禁止重写追加第二份详章(双口径=红);仅当既有章节已随其实作轮收口,才为再下一档追加新章。硬性要求取自文档、禁沿用历史轮文本:①以 " + PLAN_DOC + " 最新收口节列出的未竟项与遗留(对照总结员 carriedOver)为必答题,逐条给出处置——清偿、排入本里程碑、或显式顺延并写明续弧入口;②通用底线:regression 侧裸跑 bash scripts/regress.sh 的收绿缺口必须排入、oracle byte-exact 对拍按详案扩量(语料源如 " + YS_CORPUS + ")、上一轮如有未经独立复核的修订交本轮对抗审独立复核,复核结论写进详案;③凡触 oracle 行为的 slice,把「源码定点对读+logic-diff/internal-order 可复刻性分类」排为其前置备料 slice——序差结论一律二进制实跑仲裁,流程按 AGENTS.md 10.9-r「序差类收平流程」。详案必含:范围与显式不做、可机械执行的验收清单、有序 slices、风险。返回 PlanMeta(planPath/scope/acceptance/slices)。基线现状:build/test " + (baseCoreOk ? "绿" : "红") + ";裸跑 regress " + (baseRegress.exitCode === 0 ? "绿" : "红") + "(摘要:" + brief(baseRegress).slice(0, 400) + ")。上一轮总结:" + JSON.stringify(prev),
);
reportStage("規劃 M1", "完成", plan.scope);
log("详案落地 " + plan.planPath + ";slices:" + plan.slices.join(" → "));

phase("獨立對抗審 M1 規劃");
const adversary = agent("對抗審", { system: "你是独立对抗审查员,只读:禁止编辑任何文件,可用 Bash 做只读实验(含跑本地 yosys oracle)。职责是找真问题,不是给好评;没有真问题才准 approved=true。全程繁体中文;发现规划与仓库现实根本矛盾就如实上报。" });
let approved = false;
let feedback = "首轮,无前次意见";
for (let round = 1; round <= 3; round++) {
  const rev = await adversary.ask<Review>(
    "第 " + round + " 轮对抗审(上轮意见:" + feedback + ")。审 " + PLAN_DOC + " 的本轮详案:①读全文,核对与既有章节无矛盾;②独立复核上一轮遗留中声明「本轮复核/清偿」的项是否真修好(清单=总结员 carriedOver 与详案收口节处置表;先例形=M1 轮复核的 registry 清单一致性、process-bearing 录制管线定义、「;」切分契约对齐 oracle 实测 register.cc:246-257:仅空白定界 token 的尾分号才切分,help;help 不切分)——必要时跑 " + YOSYS + " 实证;③抽验详案验收清单每条是否真可机械执行;④核对详案排入的闸/立法改动四层(脚本/AGENTS/REGISTRY/SKILL)是否都排了。逐条 issues 附证据路径;没有真问题才 approved=true。",
  );
  if (rev.approved) {
    approved = true;
    log("对抗审第 " + round + " 轮通过");
    break;
  }
  feedback = rev.issues.join("; ");
  reportStage("對抗審·第" + round + "輪", "受阻", feedback.slice(0, 200));
  await planner.ask<string>("按对抗审意见修订 " + plan.planPath + " 的详案(直接编辑,只改必要处;改完简述改了什么):" + feedback);
}
reportStage("對抗審", approved ? "完成" : "受阻", approved ? "通过" : "三轮未全过,按现状继续,总闸兜底");

phase("換人複審 M1 規劃");
const fresh = await agent("換人複審", { system: "你是换人复审员:从未参与之前的讨论与修订。只读,禁止编辑任何文件。唯一问题:一位没参与讨论的工程师照这份详案实现,会在哪里卡住?全程繁体中文。" }).ask<Review>(
  "只凭 " + PLAN_DOC + " 的本轮详案与对仓库/vendor-reference/yosys 的快速抽查,列出自相矛盾、不可验收、缺关键决策、照做必卡住之处;没有就 approved=true。",
);
if (!fresh.approved) {
  reportStage("換人複審", "受阻", fresh.issues.join("; ").slice(0, 200));
  await planner.ask<string>("换人复审提出以下问题,做最后一轮修订详案(直接编辑):" + fresh.issues.join("; "));
} else {
  reportStage("換人複審", "完成", "通过");
}

phase("實作 M1 切片");
const impl = agent("實作員", { system: "你是 rust-cad-suite 的实现工程师,负责当轮里程碑(以规划文档详案为准)。铁则:cargo build 零警告;期望一律从 oracle(" + YOSYS + ")实跑录制入 git,测试运行时不得依赖 yosys 二进制存在;未支援构造=error 带里程碑硬错;禁止任何 git add/commit;crates/riv-*、crates/vl-* 既有代码与 docs/e2e-report 禁碰(详案明确要求的 scripts/regress.sh 与四层文档除外);收平类残差(golden byte-diff 深树形/序差类)先按 AGENTS.md 10.9-r「序差类收平流程」做「定点读 oracle 对应 pass 源码+logic-diff/internal-order 可复刻性分类」再动手,internal-order 类按 D-ys5 登记先例走 REGISTRY 分歧登记、同轮翻 regress 精确数与 known-gap 名单,不硬磕,序差结论一律二进制实跑仲裁。若闸与现实矛盾(如闸无解),如实上报,不要造假绕过。全程繁体中文回复。" });
let done = false;
let note = "首轮:无前次记录";
let sliceOk = false;
for (let round = 1; round <= 6; round++) {
  const step = await impl.ask<ImplStep>(
    "按 " + plan.planPath + " 的本轮详案实作。slices(有序):" + plan.slices.join("; ") + "。验收清单:" + plan.acceptance.join("; ") + "。上一轮情况:" + note + "。本轮:先盘点详案相关现状(工作树可能已有上一轮未提交成果),逐个补完未完 slice,只补缺口、不重做;规划的 slices 全部完成且你自查验收清单可达时,本轮回复 done=true。",
  );
  done = step.done;
  const build = await world.run("cargo", ["build", "--workspace"], { timeoutMs: 900000 });
  const buildOk = build.exitCode === 0 && !/warning/i.test(build.stderr + build.stdout);
  const test = await world.run("cargo", ["test", "--workspace"], { timeoutMs: 1800000 });
  if (buildOk && test.exitCode === 0 && done) {
    sliceOk = true;
    reportStage("切片閘", "完成", step.did.slice(0, 200));
    log("切片完成且闸绿(第 " + round + " 轮)");
    break;
  }
  note = "done=" + done + "(" + step.did.slice(0, 150) + ");" + (buildOk ? "" : "build:" + brief(build)) + (test.exitCode === 0 ? "" : "test:" + brief(test));
  reportStage("切片閘·第" + round + "輪", "受阻", note.slice(0, 200));
}
if (!sliceOk) log("切片循环未收口,进入总闸修复循环");

phase("全倉總閘與修復循環");
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
    log("总闸全绿(第 " + round + " 轮):裸跑 regress.sh(无 DEVELOPER_DIR 前缀)通过");
    break;
  }
  let diag = "";
  if (regress.exitCode !== 0) {
    const diagRun = await world.run("env", ["DEVELOPER_DIR=" + REGRESS_ENV_DIR, "bash", "scripts/regress.sh"], { timeoutMs: 1800000 });
    diag = diagRun.exitCode === 0
      ? "诊断:env DEVELOPER_DIR=CLT 下全 PASS → 失败仍属机器工具链漂移,检查 scripts/regress.sh 防御对齐是否落地且触发条件正确"
      : "诊断:env 前缀下也红 → 是代码问题:" + brief(diagRun);
  }
  fixNote = [
    buildOk ? null : "build:" + brief(build),
    test.exitCode === 0 ? null : "test:" + brief(test),
    regress.exitCode === 0 ? null : "regress:" + brief(regress) + "\n" + diag,
  ].filter((x) => x !== null).join("\n");
  reportStage("總閘·第" + round + "輪", "受阻", fixNote.slice(0, 200));
  await impl.ask<string>("总闸失败(第 " + round + " 轮),修复:" + fixNote + "\n边界:只修 ry-* 系列、本轮详案排入的文档/语料及其配套闸脚本与四层条文;其余既有失败如实报告但不要动。");
}
const st = await git.status();

await artifact.file("plan", plan.planPath, {
  title: "ryosys 里程碑規劃與成果",
  description: "本轮详案(追加于正典候选文档):regression 收口、oracle byte-exact 对拍扩量、上一轮遗留复核;附上一轮总结收据。",
  primary: true,
});

const findings: Finding[] = [];
if (!baseCoreOk) {
  findings.push({ where: "workspace(基线)", what: "盘点基线 build/test 非绿", evidence: brief(baseBuild) + " | " + brief(baseTest), status: "verified", severity: "medium" });
}
if (baseRegress.exitCode !== 0) {
  findings.push({ where: "scripts/regress.sh(基线)", what: "裸跑 regress.sh 基线红,详案已排修法 slice;本条在总闸后若仍红则为未收口项", evidence: brief(baseRegress).slice(0, 400), status: "verified", severity: finalOk ? "low" : "high" });
}
if (!approved) {
  findings.push({ where: plan.planPath, what: "对抗审三轮未完全通过,规划按最后一轮修订稿继续", evidence: "对抗审循环记录", status: "verified", severity: "medium" });
}
if (!fresh.approved) {
  findings.push({ where: plan.planPath, what: "换人复审提出问题后做了最后一轮修订(未再复审)", evidence: fresh.issues.join("; "), status: "verified", severity: "low" });
}
if (!sliceOk) {
  findings.push({ where: "crates/ry-*", what: "切片循环 6 轮未收口,依赖总闸修复循环", evidence: "切片闸记录", status: "verified", severity: "medium" });
}
if (!finalOk) {
  findings.push({ where: "workspace", what: "总闸 3 轮未全绿:build/test/regress 至少一项失败", evidence: fixNote.slice(0, 500), status: "verified", severity: "high" });
}
for (const c of prev.carriedOver) {
  findings.push({ where: "carry-over", what: c, evidence: "总结员从上一轮收据归纳(未逐条复核实测),详见 " + plan.planPath, status: "unconfirmed", severity: "low" });
}
const result: WorkflowReport = {
  conclusion: "上一轮解决了 " + prev.solved.length + " 件有收据的问题(首三条:" + prev.solved.slice(0, 3).map((s) => s.problem).join(";") + "……详见规划文档与总结报告)。本轮(" + plan.scope + ")" + (finalOk ? "已完成且总闸全绿:裸跑 regress.sh 通过、MVP 与 oracle byte-exact 对拍按验收清单收口" : "已实现但总闸未全绿,失败内容见 findings") + "。工作树呈现全部改动,未执行任何 git 提交,commit 由你定。",
  findings: findings,
  verified: [
    "上一轮总结:总结员只读归纳,每条带证据位置;遗留事项以 unconfirmed findings 列出",
    "基线盘点:cargo build --workspace + cargo test --workspace + 裸跑 bash scripts/regress.sh 一次(记录于基线盘点点)",
    "详案经对抗审(含上一轮遗留修订的独立复核与 oracle 实证)与换人复审",
    "每轮实现闸:cargo build --workspace(零警告检查)+ cargo test --workspace",
    "总闸:cargo build 零警告 + cargo test --workspace + 裸跑 bash scripts/regress.sh" + (finalOk ? "(全绿)" : "(未全绿,含 env DEVELOPER_DIR 诊断对照)") + "。",
  ],
  notCovered: [
    "后续里程碑未实作部分,以规划文档里程碑表为准",
    "ryosys 弧不触 wasm/FFI/模拟器面(各自既有闸不受影响);详案范围外的产品面未接",
    "语料扩量以当轮详案排入为准;详案未排入的语料源(如 yosys tests/ 其他目录)未收编",
    prev.carriedOver.length + " 条上一轮遗留以 unconfirmed 列出,未逐条复核实测",
  ],
};
return result;