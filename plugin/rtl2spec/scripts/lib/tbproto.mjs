// tbproto.mjs — TB 協議 v1 共享庫(§19.3.3/§19.5 D-C8;C-S3)。
// 消費者=conformance 閘(scripts/conformance-eval.mjs)與復原對拍床
// (scripts/reimpl-eval.mjs,遷用後 parity 斷言:verdict legs/segments 深等+runs/out_* byte 相等)。
// 三件事,全部唯讀無副作用(寫面在呼叫端):
//   ①丟行白名單正規化(按模擬器分腿;禁內容改寫,AGENTS 10.5/10.8 同律);
//   ②協議解析(SCENE/kv/DONE 文法+x/z 值+場景切塊);
//   ③帶看門狗的子進程執行器(AGENTS §4:每個外部模擬必帶 timeout,掛死=分類不是卡死)。

import { spawnSync } from 'node:child_process';

// ---------- ① 正規化(丟行白名單,禁內容改寫) ----------

// iverilog 腿:`$finish` 橫幅(檔名:行號 前綴,時間單位兩形 (1s)/(1ps)…)與 VCD info 行。
const IV_FINISH = /^[^ ]+:[0-9]+: \$finish called at [0-9]+ \([0-9]+[munpf]?s\)$/;
export function normalizeIverilog(raw) {
  return raw
    .split('\n')
    .filter((l) => !IV_FINISH.test(l) && !l.startsWith('VCD info: '))
    .join('\n');
}

// verilator 腿:沿 vl-difftest normalize_oracle_stdout 精確四前綴(crates/vl-difftest/src/lib.rs;
// corpus_lint.rs normalization_contract 敵意測試釘死 `-7div2=-3` 設計行保留)。尾換行規整同 Rust 契約。
export function normalizeVerilator(raw) {
  const kept = raw
    .split('\n')
    .filter(
      (l) =>
        !(l.startsWith('- ') || l.startsWith('-Info') || l.startsWith('-Warning') || l.startsWith('-Error')),
    )
    .join('\n');
  return kept.replace(/\n+$/, '') + '\n';
}

// 兩腿可比形:各自正規化後再規整尾換行(iverilog 腿只丟行不動尾,比較前統一)。
export const comparable = (txt) => txt.replace(/\n+$/, '') + '\n';

// ---------- ② 協議解析 ----------

const SCENE_RE = /^SCENE([0-9]+) ([a-z0-9_]+)$/;
const KV_ITEM = '[A-Za-z_][A-Za-z0-9_$]*=[^ ]+';
const KV_LINE = new RegExp(`^  ${KV_ITEM}( ${KV_ITEM})*$`);
const VAL_OK = /^[0-9a-fA-F-]+$/; // %h(不產負號)與 %0d(signed 可產 -);x/z/?=非 X-clean
const VAL_XZ = /^[0-9a-fA-FxXzZ?-]+$/;

/// 解析正規化輸出。回傳:scenes(序)、doneCount/doneLast、watchdog/timeouts、
/// xz(含 x/z 的 kv 值;Q4)、violations(文法外行/場景外觀測行/值字元集外;Q5)。
/// 敵意面:場景名/key 含字母 x 不誤報——只掃 `=` 右側值。
export function parseProtocol(norm) {
  const scenes = [];
  const violations = [];
  const xz = [];
  const timeouts = [];
  let watchdog = 0;
  let doneCount = 0;
  let lastKind = null;
  let cur = null;
  for (const raw of norm.split('\n')) {
    const l = raw.replace(/\r$/, '');
    if (l === '') continue;
    let m;
    if ((m = l.match(SCENE_RE))) {
      cur = { n: Number(m[1]), name: m[2], lines: [] };
      scenes.push(cur);
      lastKind = 'scene';
      continue;
    }
    if (KV_LINE.test(l)) {
      if (!cur) violations.push(`場景外觀測行: ${l.slice(0, 80)}`);
      for (const item of l.trim().split(' ')) {
        const val = item.slice(item.indexOf('=') + 1);
        if (VAL_OK.test(val)) continue;
        if (VAL_XZ.test(val)) xz.push(`${cur ? `SCENE${cur.n}` : '-'} ${item}`);
        else violations.push(`值字元集外: ${item.slice(0, 60)}`);
      }
      if (cur) cur.lines.push(l);
      lastKind = 'kv';
      continue;
    }
    if (l === 'DONE') {
      doneCount++;
      lastKind = 'done';
      continue;
    }
    if (l === 'WATCHDOG') {
      watchdog++;
      lastKind = 'watchdog';
      continue;
    }
    if ((m = l.match(/^TIMEOUT ([A-Za-z0-9_]+)$/))) {
      timeouts.push(m[1]);
      lastKind = 'timeout';
      continue;
    }
    violations.push(`文法外行: ${l.slice(0, 80)}`);
    lastKind = 'other';
  }
  return { scenes, violations, xz, timeouts, watchdog, doneCount, doneLast: lastKind === 'done' };
}

/// 場景切塊(reimpl-eval 既有語義逐字移植:`SCENE<n>\b` 開塊;塊內=非空行去尾空白;
/// 塊名=`SCENE<n>`)。parity 面:遷用前後 matched/mismatched 必逐字相等。
export function sceneBlocks(txt) {
  const blocks = new Map();
  let cur = null;
  for (const l of txt.split('\n')) {
    const m = l.match(/^SCENE(\d+)\b/);
    if (m) {
      cur = `SCENE${m[1]}`;
      blocks.set(cur, []);
    } else if (cur && l.trim() !== '') {
      blocks.get(cur).push(l.trimEnd());
    }
  }
  return blocks;
}

/// TB 源碼 `// SCENES: N` 宣告數(缺=null)。
export function declaredScenes(tbText) {
  const m = tbText.match(/^\/\/ SCENES: ([0-9]+)$/m);
  return m ? Number(m[1]) : null;
}

/// 兩份正規化輸出逐場景比對:回傳差異場景名(`SCENE<n> <name>`;序=期望側出現序)+
/// 協議尾差(DONE/WATCHDOG 等場景外行差異記為 `<tail>`)。
export function diffScenes(expectedNorm, actualNorm) {
  const a = parseProtocol(expectedNorm);
  const b = parseProtocol(actualNorm);
  const key = (s) => `SCENE${s.n} ${s.name}`;
  const bm = new Map(b.scenes.map((s) => [key(s), s.lines.join('\n')]));
  const out = [];
  for (const s of a.scenes) {
    if (bm.get(key(s)) !== s.lines.join('\n')) out.push(key(s));
  }
  const tailA = `${a.doneCount}|${a.doneLast}|${a.watchdog}|${a.timeouts.join(',')}|${a.scenes.length}`;
  const tailB = `${b.doneCount}|${b.doneLast}|${b.watchdog}|${b.timeouts.join(',')}|${b.scenes.length}`;
  if (tailA !== tailB) out.push('<tail>');
  return out;
}

// ---------- ③ 帶看門狗的執行器 ----------

/// 同步執行(spawnSync+timeout);逾時=timedOut:true(signal 非空),不拋。
export function run(cmd, args, { cwd, timeoutMs = 30000, env } = {}) {
  const t0 = Date.now();
  const r = spawnSync(cmd, args, {
    cwd,
    env: env ?? process.env,
    encoding: 'utf8',
    timeout: timeoutMs,
    maxBuffer: 256 * 1024 * 1024,
  });
  const timedOut = r.error?.code === 'ETIMEDOUT' || (r.signal !== null && r.status === null);
  return {
    code: r.status ?? (timedOut ? 124 : 1),
    stdout: r.stdout ?? '',
    stderr: r.stderr ?? (r.error ? String(r.error.message) : ''),
    timedOut,
    ms: Date.now() - t0,
    spawnError: r.error && !timedOut ? String(r.error.message) : null,
  };
}

/// iverilog 編譯+vvp 運行(Q1 計數口徑=stderr 非空行數;編譯敗=stillborn 判據)。
export function iverilogRun({ flags = ['-g2005'], top, files, workDir, outName, timeoutMs = 30000 }) {
  const out = `${workDir}/${outName}`;
  const args = [...flags, '-o', out];
  if (top) args.push('-s', top);
  args.push(...files);
  const comp = run('iverilog', args, { cwd: workDir, timeoutMs });
  const warnLines = comp.stderr.split('\n').filter((l) => l.trim() !== '');
  if (comp.code !== 0 || comp.timedOut) {
    return { compiled: false, warnLines, compErr: comp.stderr, stdout: '', timedOut: comp.timedOut, ms: comp.ms };
  }
  const sim = run('vvp', ['-n', out], { cwd: workDir, timeoutMs });
  return { compiled: true, warnLines, stdout: sim.stdout, simCode: sim.code, timedOut: sim.timedOut, ms: sim.ms };
}

/// verilator 建置+運行(`--binary --timing -Wno-fatal`;**顯式 -o 固定二進制名**——頂層名含 `__`
/// 會被 mangle,§19.19 實錄)。DEVELOPER_DIR 三行守衛由呼叫端 shell 設置(regress.sh:12-14 同形)。
export function verilatorRun({ top, files, mdir, timeoutMs = 600000, runTimeoutMs = 30000 }) {
  const args = ['--binary', '--timing', '-Wno-fatal', '--top-module', top, '-Mdir', mdir, '-o', 'vtb', ...files];
  const b = run('verilator', args, { timeoutMs });
  const diag = (b.stdout + '\n' + b.stderr).split('\n').filter((l) => /^%(Warning|Error)/.test(l));
  if (b.code !== 0 || b.timedOut) {
    return { built: false, diag, buildErr: (b.stderr || b.stdout).slice(0, 400), timedOut: b.timedOut };
  }
  const r = run(`${mdir}/vtb`, [], { cwd: mdir, timeoutMs: runTimeoutMs });
  return { built: true, diag, stdout: r.stdout, runCode: r.code, timedOut: r.timedOut, ms: r.ms };
}

export function toolVersion(cmd, args) {
  const r = run(cmd, args, { timeoutMs: 20000 });
  if (r.spawnError || (r.code !== 0 && !r.stdout && !r.stderr)) return null;
  return (r.stdout + r.stderr).split('\n').find((l) => l.trim() !== '')?.trim() ?? null;
}
