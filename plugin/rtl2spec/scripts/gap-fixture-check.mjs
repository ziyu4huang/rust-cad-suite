// gap-fixture-check.mjs — 破口 fixture 矩陣執行器(§16.2;閘端三鎖精神複刻)。
//
// 斷言規則:
//   正樣=種洞必報且 confidence 契約成立;負樣=乾淨必靜默;斷言**按檔級
//   (module,class,confidence) 多重集合相等**(多報少報皆紅,禁 contains
//   弱斷言;R16-2 教訓:G6/G7 共報低估初稿會逼出弱斷言)。
//   expected findings=斷言規格(手寫合法,與 sv_rules.rs gap_positive/
//   negative_fixtures 單測同源;golden=運行輸出快照禁手寫,兩者劃界
//   §16.2/ADR-0012 D15)。
//   --selfcheck(內建,每次運行):把正樣 expected 誤配到負樣檔必紅——
//   防斷言表恆真假綠(閘自證判別力)。
//   終止性守衛行(§16.2 矩陣最後一行):gap_hang_repro.v(F2 復現形)
//   wasm 腿零 trap 且 12 跑逐字不變+G5 恰 4 條(雙自環×2+雙向環×2,
//   與 sv_rules gap_comb_loop_self_edges_terminate_and_report 同源)。
//
// fixture 樹兩處(R16-6):複用正負樣=crates/rtl-spec/tests/fixtures/
//   gaps_positive.v|gaps_negative.v(跨包唯讀引用,不搬移不複製);
//   新增檔=plugin/rtl2spec/fixtures/。
//
// 用法: bun scripts/gap-fixture-check.mjs
// exit:0=全過;1=判據敗(含 selfcheck 紅);2=fixture 檔缺。
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const pluginRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const repoRoot = dirname(dirname(pluginRoot));

const distWasm = join(pluginRoot, 'dist', 'wasm');
const glue = await import(join(distWasm, 'rtl_spec_wasm.js'));
await glue.default({
  module_or_path: new Uint8Array(readFileSync(join(distWasm, 'rtl_spec_wasm_bg.wasm'))),
});

const POS = 'crates/rtl-spec/tests/fixtures/gaps_positive.v';
const NEG = 'crates/rtl-spec/tests/fixtures/gaps_negative.v';
const NO_ARM = 'plugin/rtl2spec/fixtures/gap_missing_reset_no_arm.v';
const HANG = 'plugin/rtl2spec/fixtures/gap_hang_repro.v';

// 矩陣全表(§16.2;與 sv_rules.rs:342-370 斷言同源;檔級 multiset)。
// **條級 vs 類級劃界(S10 實錄)**:sv_rules classes_of=去重後類集
// (dedup 斷言);本閘=**條級**多重集合——雙信號環 G5 報 2 條(兩向
// 各 1)、G6/G7 雙行程各報 2 條 G4(每行程 1 條「無復位臂」)=實跑
// 全集 14 條,兩種斷言形不矛盾。expected: [module, class, confidence][];
// msgSubs: message/suggested_test 關鍵詞抽點(sv_rules 同款釘死面)。
const TABLE = [
  {
    file: POS,
    desc: 'G1..G7 正樣 8 模組(條級共報全集 14 條)',
    expected: [
      ['latch_trap', 'latch', 'certain'],
      ['latch_default_matrix', 'latch', 'certain'],
      ['incomplete_case', 'incomplete_case', 'heuristic'],
      ['undriven_read', 'undriven_read', 'heuristic'],
      ['missing_reset', 'missing_reset', 'certain'],
      ['comb_loop', 'comb_loop', 'certain'],
      ['comb_loop', 'comb_loop', 'certain'],
      ['multi_driver', 'missing_reset', 'certain'],
      ['multi_driver', 'missing_reset', 'certain'],
      ['multi_driver', 'multi_driver', 'certain'],
      ['blocking_race', 'blocking_in_seq', 'certain'],
      ['blocking_race', 'missing_reset', 'certain'],
      ['blocking_race', 'missing_reset', 'certain'],
      ['blocking_race', 'multi_driver', 'certain'],
    ],
    msgSubs: [
      ['latch_default_matrix', 'message', 'y', true],
      ['latch_default_matrix', 'message', 'x', false],
      ['latch_default_matrix', 'suggested_test', 'x', false],
      ['incomplete_case', 'message', '2/4', true],
      ['comb_loop', 'message', 't1', true],
      ['comb_loop', 'message', 't2', true],
    ],
  },
  {
    file: NEG,
    desc: 'G1..G7 負樣 8 模組(乾淨必靜默)',
    expected: [],
  },
  {
    file: NO_ARM,
    desc: 'G4 無復位臂正樣(F1a 有序取樣:字典序首名進 suggested_test)',
    expected: [['gap_missing_reset_no_arm', 'missing_reset', 'certain']],
    msgSubs: [
      ['gap_missing_reset_no_arm', 'message', '2 個寄存器上電值未定義', true],
      ['gap_missing_reset_no_arm', 'suggested_test', 'ra', true],
      ['gap_missing_reset_no_arm', 'suggested_test', 'rb', false],
    ],
  },
  {
    // S4/M4 正樣:DelayCtrl/Wait 容器內死分支命中(觸發形 always #1 if(1'b0))。
    file: 'plugin/rtl2spec/fixtures/gap_delay_dead.v',
    desc: 'G8 容器展開正樣(DelayCtrl/Wait 內死分支)',
    expected: [
      ['delay_dead', 'dead_branch', 'certain'],
      ['wait_dead', 'dead_branch', 'certain'],
    ],
  },
  {
    // S4/M4 負樣:容器內可變條件=零死分支。
    file: 'plugin/rtl2spec/fixtures/gap_delay_live.v',
    desc: 'G8 容器展開負樣(容器內可變條件零命中)',
    expected: [],
  },
  {
    // S4/M5:真缺口 certain+無條件純常量逐拍清除(strobe)不報(誤報形紅路面)。
    file: 'plugin/rtl2spec/fixtures/gap_missing_reset_strobe.v',
    desc: 'G4 分級正確(M5 約束:mr_data certain+mr_strobe 不報)',
    expected: [['mr_data', 'missing_reset', 'certain']],
  },
  {
    // S4/M6:4/4 覆蓋樣不報+稀疏樣仍報 heuristic。
    file: 'plugin/rtl2spec/fixtures/gap_case_covered.v',
    desc: 'G2 覆蓋守衛(M6:case_covered 不報+case_sparse 仍報)',
    expected: [['case_sparse', 'incomplete_case', 'heuristic']],
  },
];

const fail = (msg) => {
  console.log(`FAIL ${msg}`);
  process.exitCode = 1;
};
const scan = (f) => {
  try {
    return JSON.parse(glue.rtl_scan_gaps_sv(f, readFileSync(join(repoRoot, f), 'utf8')));
  } catch (e) {
    fail(`${f} wasm trap/解析敗: ${String(e).slice(0, 120)}`);
    return { items: [] };
  }
};
const sig = (it) => `${it.module}|${it.class}|${it.confidence}`;

let checked = 0;
for (const row of TABLE) {
  const rep = scan(row.file);
  checked++;
  const actual = rep.items.map(sig).sort();
  const want = row.expected.map((e) => e.join('|')).sort();
  if (JSON.stringify(actual) === JSON.stringify(want)) {
    console.log(`PASS ${row.desc} (${row.file.split('/').pop()} items=${actual.length})`);
  } else {
    fail(
      `${row.desc} 檔級集合不相等:\n  want=${JSON.stringify(want)}\n  actual=${JSON.stringify(actual)}`,
    );
    continue;
  }
  for (const [module, field, sub, shouldContain] of row.msgSubs ?? []) {
    const it = rep.items.find((i) => i.module === module);
    if (!it) {
      fail(`${row.file.split('/').pop()} ${module} 無 items(msgSubs 無法核)`);
      continue;
    }
    const has = it[field].includes(sub);
    if (has === shouldContain) {
      console.log(`PASS msgSubs ${module}.${field} ${shouldContain ? '含' : '不含'}「${sub}」`);
    } else {
      fail(`${module}.${field} ${shouldContain ? '必含' : '必不含'}「${sub}」,實得:${it[field].slice(0, 60)}`);
    }
  }
}

// 終止性守衛行:wasm 腿 12 跑逐字不變(進程內重複調用;F2 修復後必終止)
// + G5 恰 4 條 certain(x/y 雙自環+雙向環)。
{
  const src = readFileSync(join(repoRoot, HANG), 'utf8');
  let last = '';
  let stable = true;
  for (let k = 0; k < 12; k++) {
    let out;
    try {
      out = glue.rtl_scan_gaps_sv(HANG, src);
    } catch (e) {
      fail(`${HANG} wasm trap(第 ${k + 1} 跑): ${String(e).slice(0, 120)}`);
      stable = false;
      break;
    }
    if (k > 0 && out !== last) {
      fail(`${HANG} 12 跑非逐字不變(第 ${k + 1} 跑與前不同;終止性/確定性契約)`);
      stable = false;
      break;
    }
    last = out;
  }
  if (stable) {
    const rep = JSON.parse(last);
    const g5 = rep.items.filter((i) => i.class === 'comb_loop');
    const ok = g5.length === 4 && g5.every((i) => i.confidence === 'certain');
    if (ok) {
      console.log(`PASS 終止性守衛 ${HANG.split('/').pop()}(12 跑逐字不變+G5 恰 4 條 certain)`);
    } else {
      fail(`終止性守衛 G5 形期望 4 條 certain,實得 ${JSON.stringify(g5.map((i) => i.id))}`);
    }
    checked++;
  }
}

// --selfcheck(閘自證判別力,每次運行):把正樣 expected 誤配到負樣檔——
// 集合相等斷言**必紅**;若意外成立=斷言表無判別力(恆真假綠防線)。
{
  const negRep = scan(NEG);
  const wouldPass = JSON.stringify(negRep.items.map(sig).sort()) === JSON.stringify(['latch_trap|latch|certain']);
  if (!wouldPass) {
    console.log('PASS selfcheck 正樣誤配負樣必紅(斷言表判別力自證)');
  } else {
    fail('selfcheck:負樣檔竟通過正樣期望=斷言表恆真,紅路失效');
  }
}

// ---- S2 規則面矩陣(N3;§5.2 S2)——extract_rules 腿(與 gap 檔級矩陣同形:
// 正樣條級 multiset 相等+負樣零規則+selfcheck 判別力;fixture=自著 MIT)。
{
  const extract = (f) => {
    try {
      return JSON.parse(
        glue.rtl_extract_rules_sv(f, readFileSync(join(repoRoot, f), 'utf8'), 128),
      );
    } catch (e) {
      fail(`${f} wasm trap/解析敗: ${String(e).slice(0, 120)}`);
      return { modules: [] };
    }
  };
  const relSig = (rb) =>
    (rb.modules ?? []).flatMap((m) =>
      (m.rules ?? [])
        .filter((r) => r.check?.form === 'relation')
        .map((r) => `${r.module}|${r.check.lhs}|${r.check.rhs}`),
    );
  const POS2 = 'plugin/rtl2spec/fixtures/rel_positive.v';
  const NEG2 = 'plugin/rtl2spec/fixtures/rel_negative.v';
  {
    const rb = extract(POS2);
    const actual = relSig(rb).sort();
    const want = [
      "rel_positive|arm_c|4'd1",
      "rel_positive|arm_c|4'd2", // S4/M2:default 臂=未被覆蓋域
      "rel_positive|scaled|{1'b0, b['sd7:'sd3]}",
      "rel_positive|slice|a['sd7:'sd3]",
    ].sort();
    if (JSON.stringify(actual) === JSON.stringify(want)) {
      console.log(`PASS S2 正樣 條級 multiset (rel_positive.v relations=${actual.length})`);
    } else {
      fail(`S2 正樣集合不等:\n  want=${JSON.stringify(want)}\n  actual=${JSON.stringify(actual)}`);
    }
    const armc = (rb.modules ?? [])
      .flatMap((m) => m.rules ?? [])
      .find((r) => r.check?.lhs === 'arm_c');
    if (armc && armc.check.when === "sel == 1'b0") {
      console.log(`PASS S2 分支域 when (arm_c 攜 sel == 1'b0)`);
    } else {
      fail(`S2 arm_c when 期望 sel == 1'b0,實得 ${JSON.stringify(armc?.check?.when)}`);
    }
  }
  {
    const rb = extract(NEG2);
    const total = (rb.modules ?? []).reduce((n, m) => n + (m.rules?.length ?? 0), 0);
    if (total === 0) {
      console.log('PASS S2 負樣 零規則 (rel_negative.v:純常量直賦/直通別名/無條件行程常量)');
    } else {
      fail(`S2 負樣應零規則,實得 ${total}`);
    }
  }
  // selfcheck:正樣單條期望誤配負樣必紅(斷言表判別力自證)。
  {
    const actual = relSig(extract(NEG2)).sort();
    const wouldPass = JSON.stringify(actual) === JSON.stringify(["rel_positive|slice|a['sd7:'sd3]"]);
    if (!wouldPass) {
      console.log('PASS S2 selfcheck 正樣期望誤配負樣必紅');
    } else {
      fail('S2 selfcheck:負樣竟通過正樣單條期望=斷言表恆真,紅路失效');
    }
  }
  // S4/M1 正樣:多標簽臂逐標簽展開(2'b01 觸發域可見)+M3 包膜。
  // (時序行程→sequence 形;sig=form 無關:module|lhs/signal|rhs/becomes)
  {
    const rb = extract('plugin/rtl2spec/fixtures/m1_min_case.v');
    const rules = (rb.modules ?? []).flatMap((m) => (m.rules ?? []));
    const sig = (r) => {
      const lhs = r.check.effects?.[0]?.lhs ?? r.check.lhs ?? r.check.signal;
      const rhs = r.check.effects?.[0]?.value ?? r.check.rhs ?? r.check.becomes;
      return `${r.module}|${lhs}|${rhs}`;
    };
    const actual = rules.map(sig).sort();
    const want = [
      "m1min|y|1'b0", // guard:復位臂 y<=1'b0
      "m1min|s|2'b00", // guard:復位臂 s<=2'b00
      "m1min|y|1'b1", // sequence 2'b00 臂
      "m1min|y|1'b1", // sequence 2'b01 臂(M1)
      "m1min|y|din['sd0]", // sequence 2'b10 臂
      "m1min|y|1'b1", // sequence default 臂(M2)
    ].sort();
    const whens = rules.map((r) => `${r.check.form}:${r.check.when ?? ''}`);
    const whenOk =
      whens.filter((w) => w === `guard:rst`).length === 2 &&
      whens.filter((w) => w === `sequence:!(rst) 且 s == 2\'b00`).length === 1 &&
      whens.filter((w) => w === `sequence:!(rst) 且 s == 2\'b01`).length === 1 && // M1:2'b01 觸發域
      whens.filter((w) => w === `sequence:!(rst) 且 s == 2\'b10`).length === 1 &&
      whens.filter((w) => w === `sequence:!(rst) 且 !(s == 2\'b00) 且 !(s == 2\'b01) 且 !(s == 2\'b10)`).length === 1; // M2 default 臂
    if (JSON.stringify(actual) === JSON.stringify(want) && whenOk) {
      console.log('PASS S4/M1+M2 正樣 條級 multiset+逐標簽觸發域+default 臂否定合取 (m1_min_case.v)');
    } else {
      fail(`S4/M1+M2 集合不等:\n  actual=${JSON.stringify(actual)}\n  whens=${JSON.stringify(whens)}`);
    }
  }
  // S4/M1+M2 負樣:單標簽臂不增殖(4 條,各攜自身標簽域)+無 default 不產
  // default 臂規則(僅顯式臂 2 條,無否定合取 when)。
  {
    const rb = extract('plugin/rtl2spec/fixtures/m12_negative.v');
    const rules = (rb.modules ?? []).flatMap((m) => (m.rules ?? [])).filter((r) => r.check.form === 'sequence');
    const m1neg = rules.filter((r) => r.module === 'm1neg');
    const m2neg = rules.filter((r) => r.module === 'm2neg');
    const ok =
      m1neg.length === 4 &&
      m2neg.length === 2 &&
      m1neg.every((r) => ['2\'b00', '2\'b01', '2\'b10', '2\'b11'].every((l) => !(r.check.when ?? '').includes(`!(sel == ${l})`)) && ['2\'b00', '2\'b01', '2\'b10', '2\'b11'].some((l) => (r.check.when ?? '') === `sel == ${l}`)) &&
      m2neg.every((r) => !(r.check.when ?? '').includes('!('));
    if (ok) {
      console.log('PASS S4/M1+M2 負樣 單標簽不增殖+無 default 零規則 (m12_negative.v)');
    } else {
      fail(`S4/M1+M2 負樣形不符: m1neg=${m1neg.length} m2neg=${m2neg.length} ${JSON.stringify(rules.map((r) => [r.module, r.check.when]))}`);
    }
  }
}

console.log(`== gap-fixture-check: ${process.exitCode === 1 ? 'FAIL' : 'ALL PASS'}(檔級矩陣 ${TABLE.length + 1} 行+S2 規則面) ==`);
process.exit(process.exitCode ?? 0);
