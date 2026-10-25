/**
 * Client half of the rverilator.wasm playground bundle.
 *
 * The page is a React panel in the Harness' `main` slot (reached from its own
 * icon in `sidebar.panellist`), styled with the host theme tokens. It embeds the
 * repo's wasm playground:
 *
 *   - playground: source + `$readmem` files + engine selector
 *     (rverilator → WasmSim | riverilog → riv_build + riv_run), stdout, and the
 *     VCD rendered by the SAME Rust SVG renderer the CLI uses (no canvas).
 *   - corpus cards: run the oracle-recorded e2e cases and byte-diff stdout.
 *   - spec card: RTL ↔ golden-model lockstep with named protocol violations.
 *
 * No iframe and no Host-rendered HTML page: the panel is a slot component so it
 * inherits tokens, theme switching, and locale. The only thing the Host serves
 * is the wasm artifact and the allowlisted corpus slice (`/rverilator-wasm/`).
 *
 * Embedding seam: `window.__rverilatorWasmModule` lets a host page supply an
 * already-imported engine module (used by the bundle's Node smoke test); absent
 * it, the panel imports the artifact from the Host route.
 */
window.__ModuleLoader__.load({
  id: '@local/rverilator-wasm-playground',
  factory(require) {
    const React = require('react');
    const h = React.createElement;

    const NS = 'rverilatorWasm';
    const PANEL_ID = 'rverilator-wasm';
    const BASE = '/rverilator-wasm/';

    const DEFAULT_SRC = `\`timescale 1ns/1ps
module tb;
  reg clk = 0;
  reg [7:0] q = 0;
  always #5 clk = ~clk;
  always @(posedge clk) q <= q + 1;
  initial begin
    $dumpfile("dump.vcd");
    $dumpvars;
    repeat (8) @(posedge clk);
    #1 $display("q=%0d", q);
    $finish;
  end
endmodule
`;

    // The three cases the Host route allowlists. `forceDump` is the facade
    // capability for XLS TBs that carry no `$dumpvars` (not a TB edit).
    const CASES = [
      {
        dir: '51_vcd_counter',
        titleKey: 'caseCounterTitle',
        descKey: 'caseCounterDesc',
        files: ['tb.v'],
        top: 'tb',
        forceDump: false,
      },
      {
        dir: '57_xls_add3',
        titleKey: 'caseAdd3Title',
        descKey: 'caseAdd3Desc',
        files: ['add3.v', 'tb.v'],
        top: 'tb',
        forceDump: false,
      },
      {
        dir: '58_xls_acc_pipe',
        titleKey: 'caseAccTitle',
        descKey: 'caseAccDesc',
        files: ['acc_pipe.v', 'tb.v'],
        top: 'tb',
        forceDump: true,
      },
    ];

    const VARIANTS = ['variantCorrect', 'variantBitOrder', 'variantNoAck'];

    const EN = {
      panelTitle: 'Verilog Playground',
      panelSubtitle: 'One wasm engine, two CLI faces: rverilator (vl-sim) and riverilog → rivvp (riv-elab + riv-kernel). Waves are the shared vl-vcd Rust renderer.',
      wasmLoading: 'loading wasm engine…',
      wasmFailed: 'wasm engine failed: ',
      srcLabel: 'Verilog source (top auto-detected if empty)',
      topLabel: 'top module (optional)',
      filesLabel: '$readmem files (JSON: name → content)',
      engineVl: 'engine: rverilator',
      engineRiv: 'engine: riverilog → rivvp',
      rivNote: 'riverilog face: sources compile to the RIV-1 design, then rivvp interprets it — the same two-step contract as the native CLIs. The $finish banner is the iverilog-face form.',
      rivBytes: 'RIV-1 bytes',
      run: 'Run',
      running: 'running…',
      stdout: 'stdout',
      noRun: '(no run yet)',
      noOutput: '(no output)',
      waveform: 'waveform (from $dumpvars; click a name to toggle)',
      noDump: 'no dump',
      signals: 'signals',
      done: 'done',
      error: 'error',
      verifyHeading: 'Independent verification cards',
      verifyIntro: 'Oracle-recorded corpus, run here in wasm and byte-diffed against the Verilator-recorded expectation — the browser does not trust its own engine.',
      runVerify: 'Run + verify',
      matchBadge: '✓ byte-exact against the recorded expectation',
      diffBadge: '✗ differs from the recorded expectation',
      specHeading: 'Spec verification: RTL ↔ golden-model lockstep',
      specIntro: 'The same composition `cargo test -p spec-i2c` gates natively: an executable golden model plus per-cycle protocol invariants. Pick a surgically broken RTL variant and watch the spec name the violation.',
      runSpec: 'Run spec check',
      variantCorrect: 'correct RTL',
      variantBitOrder: 'broken: LSB-first bit order',
      variantNoAck: 'broken: no ack release',
      specOk: '✓ spec holds',
      specBad: '✗ spec violated',
      cycles: 'cycles',
      violations: 'violations',
      rtl: 'RTL source',
      colCycle: 'cycle',
      colTime: 'time',
      colKind: 'kind',
      colExpected: 'expected',
      colActual: 'actual',
      caseCounterTitle: 'counter — reference case',
      caseCounterDesc: 'Repo corpus case with $dumpvars; the shared-reader gate parses this exact VCD.',
      caseAdd3Title: 'XLS add3 (DSLX → IR → pipeline → Verilog)',
      caseAdd3Desc: 'Three-stage pipelined adder emitted by the XLS toolchain, harvested by the repo pipeline.',
      caseAccTitle: 'XLS acc_pipe (stateful pipeline)',
      caseAccDesc: 'Accumulator pipeline from XLS codegen; the TB has no $dumpvars, so the page asks the facade to force the dump.',
    };

    const ZH_TW = {
      panelTitle: 'Verilog 模擬台',
      panelSubtitle: '同一個 wasm 引擎、兩個 CLI 面:rverilator(vl-sim)與 riverilog → rivvp(riv-elab + riv-kernel)。波形由共用的 vl-vcd Rust 渲染器輸出。',
      wasmLoading: 'wasm 引擎載入中…',
      wasmFailed: 'wasm 引擎載入失敗:',
      srcLabel: 'Verilog 原始碼(top 留空則自動偵測)',
      topLabel: 'top module(選填)',
      filesLabel: '$readmem 檔案(JSON:檔名 → 內容)',
      engineVl: '引擎:rverilator',
      engineRiv: '引擎:riverilog → rivvp',
      rivNote: 'riverilog 面:原始碼先編成 RIV-1 設計,再由 rivvp 直譯——與原生 CLI 相同的兩段式契約。$finish 橫幅為 iverilog 面格式。',
      rivBytes: 'RIV-1 位元組',
      run: '執行',
      running: '執行中…',
      stdout: '標準輸出',
      noRun: '(尚未執行)',
      noOutput: '(無輸出)',
      waveform: '波形(來自 $dumpvars;點名稱可切換)',
      noDump: '無 dump',
      signals: '個訊號',
      done: '完成',
      error: '錯誤',
      verifyHeading: '獨立驗證卡',
      verifyIntro: 'oracle 錄製的語料在此以 wasm 重跑,對 Verilator 錄下的期望值逐位元組比對——瀏覽器不信任自己的引擎。',
      runVerify: '執行 + 驗證',
      matchBadge: '✓ 與錄製期望值逐位元組相符',
      diffBadge: '✗ 與錄製期望值不同',
      specHeading: '規格驗證:RTL ↔ 黃金模型 lockstep',
      specIntro: '與原生 `cargo test -p spec-i2c` 相同組合:可執行黃金模型 + 每週期協定不變式。選一個刻意破壞的 RTL 變體,看規格層指名違規。',
      runSpec: '執行規格檢查',
      variantCorrect: '正確 RTL',
      variantBitOrder: '破壞:LSB-first 位元順序',
      variantNoAck: '破壞:未釋放 ack',
      specOk: '✓ 規格成立',
      specBad: '✗ 規格違反',
      cycles: '週期',
      violations: '違規',
      rtl: 'RTL 原始碼',
      colCycle: '週期',
      colTime: '時間',
      colKind: '種類',
      colExpected: '期望',
      colActual: '實際',
      caseCounterTitle: '計數器 — 參考案例',
      caseCounterDesc: '帶 $dumpvars 的 repo 語料案例;共用讀取器閘解析這份 VCD。',
      caseAdd3Title: 'XLS add3(DSLX → IR → pipeline → Verilog)',
      caseAdd3Desc: 'XLS 工具鏈產出的三級管線加法器,經 repo 收割管線收編。',
      caseAccTitle: 'XLS acc_pipe(有狀態管線)',
      caseAccDesc: 'XLS codegen 的累加器管線;TB 沒有 $dumpvars,由頁面要求 facade 強制 dump。',
    };

    // Simplified Chinese dictionary: the shipped Harness UI runs zh-CN by default,
    // and an absent dictionary silently falls back to English.
    const ZH_CN = {
      panelTitle: 'Verilog 模拟台',
      panelSubtitle: '同一个 wasm 引擎、两个 CLI 面:rverilator(vl-sim)与 riverilog → rivvp(riv-elab + riv-kernel)。波形由共用的 vl-vcd Rust 渲染器输出。',
      wasmLoading: 'wasm 引擎载入中…',
      wasmFailed: 'wasm 引擎载入失败:',
      srcLabel: 'Verilog 源码(top 留空则自动检测)',
      topLabel: 'top module(可选)',
      filesLabel: '$readmem 文件(JSON:文件名 → 内容)',
      engineVl: '引擎:rverilator',
      engineRiv: '引擎:riverilog → rivvp',
      rivNote: 'riverilog 面:源码先编译成 RIV-1 设计,再由 rivvp 直译——与原生 CLI 相同的两段式契约。$finish 横幅为 iverilog 面格式。',
      rivBytes: 'RIV-1 字节',
      run: '运行',
      running: '运行中…',
      stdout: '标准输出',
      noRun: '(尚未运行)',
      noOutput: '(无输出)',
      waveform: '波形(来自 $dumpvars;点名称可切换)',
      noDump: '无 dump',
      signals: '个信号',
      done: '完成',
      error: '错误',
      verifyHeading: '独立验证卡',
      verifyIntro: 'oracle 录制的语料在此以 wasm 重跑,对 Verilator 录下的期望值逐字节比对——浏览器不信任自己的引擎。',
      runVerify: '运行 + 验证',
      matchBadge: '✓ 与录制期望值逐字节相符',
      diffBadge: '✗ 与录制期望值不同',
      specHeading: '规格验证:RTL ↔ 黄金模型 lockstep',
      specIntro: '与原生 `cargo test -p spec-i2c` 相同组合:可执行黄金模型 + 每周期协议不变式。选一个刻意破坏的 RTL 变体,看规格层指名违规。',
      runSpec: '运行规格检查',
      variantCorrect: '正确 RTL',
      variantBitOrder: '破坏:LSB-first 位顺序',
      variantNoAck: '破坏:未释放 ack',
      specOk: '✓ 规格成立',
      specBad: '✗ 规格违反',
      cycles: '周期',
      violations: '违规',
      rtl: 'RTL 源码',
      colCycle: '周期',
      colTime: '时间',
      colKind: '种类',
      colExpected: '期望',
      colActual: '实际',
      caseCounterTitle: '计数器 — 参考案例',
      caseCounterDesc: '带 $dumpvars 的 repo 语料案例;共用读取器闸解析这份 VCD。',
      caseAdd3Title: 'XLS add3(DSLX → IR → pipeline → Verilog)',
      caseAdd3Desc: 'XLS 工具链产出的三级流水加法器,经 repo 收割流水线收录。',
      caseAccTitle: 'XLS acc_pipe(有状态流水线)',
      caseAccDesc: 'XLS codegen 的累加器流水线;TB 没有 $dumpvars,由页面要求 facade 强制 dump。',
    };

    const CSS = `
.rvw-root { display: flex; flex-direction: column; height: 100%; min-height: 0; overflow: auto;
  background: var(--dsw-alias-bg-base); color: var(--dsw-alias-label-primary);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 13px; line-height: 1.5; }
.rvw-inner { padding: 16px 18px 32px; max-width: 1320px; width: 100%; box-sizing: border-box; margin: 0 auto; }
.rvw-head { display: flex; align-items: baseline; gap: 12px; flex-wrap: wrap; }
.rvw-title { font-size: 17px; font-weight: 600; margin: 0; }
.rvw-sub { color: var(--dsw-alias-label-secondary); font-size: 12px; margin: 4px 0 14px; max-width: 980px; }
.rvw-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 16px; align-items: start; }
@media (max-width: 980px) { .rvw-grid { grid-template-columns: minmax(0, 1fr); } }
.rvw-label { display: block; color: var(--dsw-alias-label-secondary); font-size: 12px; margin: 8px 0 4px; }
.rvw-ta, .rvw-in, .rvw-sel { width: 100%; box-sizing: border-box; background: var(--dsw-alias-bg-layer-2);
  color: var(--dsw-alias-label-primary); border: 1px solid var(--dsw-alias-border-l1); border-radius: 6px;
  font: inherit; padding: 7px 8px; }
.rvw-ta { resize: vertical; }
.rvw-ta.src { min-height: 292px; }
.rvw-ta.files { min-height: 56px; }
.rvw-row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; margin: 10px 0 6px; }
.rvw-in.top { width: 9rem; }
.rvw-sel { width: auto; }
.rvw-btn { background: var(--dsw-alias-bg-layer-1); color: var(--dsw-alias-label-primary);
  border: 1px solid var(--dsw-alias-border-l2); border-radius: 6px; padding: 5px 14px; font: inherit; cursor: pointer; }
.rvw-btn:hover:not(:disabled) { border-color: var(--dsw-alias-brand-primary); }
.rvw-btn:disabled { opacity: .55; cursor: default; }
.rvw-btn.primary { border-color: var(--dsw-alias-brand-primary); color: var(--dsw-alias-brand-primary); }
.rvw-status { color: var(--dsw-alias-label-secondary); font-size: 12px; }
.rvw-note { color: var(--dsw-alias-label-secondary); font-size: 11.5px; margin: 6px 0 0; }
.rvw-pre { background: var(--dsw-alias-bg-layer-2); border: 1px solid var(--dsw-alias-border-l1);
  border-radius: 6px; padding: 8px 10px; margin: 4px 0 0; min-height: 3rem; max-height: 22rem;
  overflow: auto; white-space: pre-wrap; word-break: break-word; font: inherit; font-size: 12px; }
.rvw-pre.err, .rvw-err { color: var(--dsw-alias-state-error-primary); }
.rvw-sigs { font-size: 11.5px; color: var(--dsw-alias-label-secondary); margin: 6px 0 4px; display: flex; flex-wrap: wrap; gap: 2px 10px; }
.rvw-sig { cursor: pointer; user-select: none; }
.rvw-sig.on { color: var(--dsw-alias-label-primary); }
.rvw-wave { background: var(--dsw-alias-bg-layer-2); border: 1px solid var(--dsw-alias-border-l1);
  border-radius: 6px; padding: 6px; min-height: 40px; overflow: auto; }
.rvw-wave svg { display: block; width: 100%; height: auto; }
.rvw-h2 { font-size: 14px; font-weight: 600; margin: 26px 0 4px; color: var(--dsw-alias-label-primary); }
.rvw-card { border: 1px solid var(--dsw-alias-border-l1); border-radius: 8px; padding: 12px 14px;
  margin: 10px 0; background: var(--dsw-alias-bg-layer-1); }
.rvw-card h3 { font-size: 13px; font-weight: 600; margin: 0 0 2px; }
.rvw-card .rvw-desc { color: var(--dsw-alias-label-secondary); font-size: 11.5px; }
.rvw-badge { font-size: 12px; }
.rvw-badge.ok { color: var(--dsw-alias-state-success-primary); }
.rvw-badge.bad { color: var(--dsw-alias-state-error-primary); }
.rvw-badge.busy { color: var(--dsw-alias-label-secondary); }
.rvw-table { border-collapse: collapse; font-size: 11.5px; margin: 8px 0; }
.rvw-table td { padding: 0 12px 0 0; vertical-align: top; }
.rvw-table th { text-align: left; color: var(--dsw-alias-label-secondary); font-weight: 500; padding: 0 12px 2px 0; }
.rvw-details { margin-top: 10px; }
.rvw-details summary { font-size: 12px; color: var(--dsw-alias-label-secondary); cursor: pointer; }
.rvw-details .rvw-pre { max-height: 18rem; }
.rvw-variants { display: flex; flex-wrap: wrap; gap: 4px 16px; font-size: 12px; margin-bottom: 8px; }
.rvw-variants label { cursor: pointer; color: var(--dsw-alias-label-primary); }
`;

    // ---------------------------------------------------------------- engine

    let wasmPromise = null;
    function loadWasm() {
      if (wasmPromise) return wasmPromise;
      wasmPromise = (async () => {
        const provided = typeof window !== 'undefined' ? window.__rverilatorWasmModule : undefined;
        if (provided) {
          if (typeof provided.default === 'function') await provided.default();
          return provided;
        }
        const origin = typeof location !== 'undefined' ? location.origin : '';
        const mod = await import(new URL(BASE + 'pkg/vl_wasm.js', origin).href);
        await mod.default();
        return mod;
      })().catch((error) => {
        wasmPromise = null;
        throw error;
      });
      return wasmPromise;
    }

    function parseFiles(text) {
      const trimmed = (text || '').trim();
      if (!trimmed) return { names: [], contents: [] };
      const map = JSON.parse(trimmed);
      return { names: Object.keys(map), contents: Object.values(map) };
    }

    /** Run one playground submission; mirrors the CLI's two faces. */
    function runPlayground(mod, input) {
      const files = parseFiles(input.filesText);
      if (input.engine === 'riv') {
        const riv = mod.riv_build(input.top, [input.src]);
        const result = mod.riv_run(riv, files.names);
        return { stdout: result.stdout, vcdText: result.vcd || '', rivBytes: riv.length };
      }
      const sim = new mod.WasmSim(input.top, [input.src], files.names, files.contents);
      sim.run();
      return { stdout: sim.take_output(), vcdText: sim.take_vcd() || '', rivBytes: 0 };
    }

    /** Byte-diff one corpus case: sources come from the Host allowlisted route. */
    async function runCorpusCase(mod, target) {
      const sources = [];
      for (const file of target.files) {
        const res = await fetch(BASE + 'corpus/e2e/' + target.dir + '/' + file);
        if (!res.ok) throw new Error(file + ': HTTP ' + res.status);
        sources.push(await res.text());
      }
      const expRes = await fetch(BASE + 'corpus/e2e/' + target.dir + '/expected.stdout');
      if (!expRes.ok) throw new Error('expected.stdout: HTTP ' + expRes.status);
      const expected = await expRes.text();
      const sim = new mod.WasmSim(target.top, sources, [], []);
      if (target.forceDump) sim.force_dump();
      sim.run();
      const got = sim.take_output();
      return { stdout: got, match: got === expected, vcdText: sim.take_vcd() || '' };
    }

    function signalNames(vcd) {
      const names = [];
      for (let i = 0; i < vcd.varCount; i++) names.push(vcd.var_name(i));
      names.sort();
      return names;
    }

    function renderWave(vcd, names, shown) {
      const selected = names.filter((name) => shown.has(name));
      return vcd.to_svg(selected, 0, 0, 0n, 0n, 0);
    }

    // ------------------------------------------------------------------- i18n

    function useTranslate(ctx) {
      const [, bump] = React.useState(0);
      React.useEffect(() => ctx.locale.subscribe(() => bump((n) => n + 1)), [ctx]);
      return ctx.locale.bind(NS);
    }

    // ------------------------------------------------------------------- view

    /** Waveform host: parses once per VCD and re-renders on signal toggles. */
    function Wave({ t, mod, vcdText }) {
      const [view, setView] = React.useState(null);
      React.useEffect(() => {
        if (!mod || !vcdText) {
          setView(null);
          return;
        }
        try {
          const vcd = mod.vcd_parse(vcdText);
          const names = signalNames(vcd);
          const shown = new Set(names.slice(0, 8));
          setView({ vcd, names, shown, svg: renderWave(vcd, names, shown), error: '' });
        } catch (error) {
          setView({ vcd: null, names: [], shown: new Set(), svg: '', error: String(error) });
        }
      }, [mod, vcdText]);

      if (!view) return h('div', { className: 'rvw-wave' }, h('span', { className: 'rvw-status' }, t('noDump')));
      if (view.error) return h('div', { className: 'rvw-wave rvw-err' }, view.error);

      const toggle = (name) => {
        const shown = new Set(view.shown);
        if (shown.has(name)) shown.delete(name);
        else shown.add(name);
        setView({ ...view, shown, svg: renderWave(view.vcd, view.names, shown) });
      };

      return h(
        'div',
        null,
        h(
          'div',
          { className: 'rvw-sigs' },
          view.names.map((name) =>
            h(
              'span',
              {
                key: name,
                className: 'rvw-sig' + (view.shown.has(name) ? ' on' : ''),
                onClick: () => toggle(name),
                role: 'button',
                tabIndex: 0,
                onKeyDown: (event) => {
                  if (event.key === 'Enter' || event.key === ' ') toggle(name);
                },
              },
              (view.shown.has(name) ? '■ ' : '□ ') + name,
            ),
          ),
        ),
        h('div', { className: 'rvw-wave', dangerouslySetInnerHTML: { __html: view.svg } }),
      );
    }

    function Playground({ t, mod, modError }) {
      const [src, setSrc] = React.useState(DEFAULT_SRC);
      const [top, setTop] = React.useState('');
      const [filesText, setFilesText] = React.useState('');
      const [engine, setEngine] = React.useState('vl');
      const [busy, setBusy] = React.useState(false);
      const [stdout, setStdout] = React.useState('');
      const [failed, setFailed] = React.useState(false);
      const [status, setStatus] = React.useState('');
      const [vcdText, setVcdText] = React.useState('');
      const [rivBytes, setRivBytes] = React.useState(0);

      const run = async () => {
        if (!mod) return;
        setBusy(true);
        setStatus(t('running'));
        setFailed(false);
        try {
          const result = runPlayground(mod, { src, top, filesText, engine });
          setStdout(result.stdout || t('noOutput'));
          setVcdText(result.vcdText);
          setRivBytes(result.rivBytes);
          setStatus(t('done'));
        } catch (error) {
          setStdout(String(error));
          setFailed(true);
          setStatus(t('error'));
        }
        setBusy(false);
      };

      return h(
        'div',
        null,
        h('div', { className: 'rvw-row' },
          h('select', {
            className: 'rvw-sel',
            value: engine,
            onChange: (event) => setEngine(event.target.value),
            'aria-label': t('engineVl'),
          },
            h('option', { value: 'vl' }, t('engineVl')),
            h('option', { value: 'riv' }, t('engineRiv')),
          ),
          h('button', { className: 'rvw-btn primary', onClick: run, disabled: busy || !mod }, t('run')),
          h('span', { className: 'rvw-status' },
            mod ? (engine === 'riv' && rivBytes ? t('rivBytes') + ': ' + rivBytes : status) : (modError ? t('wasmFailed') + ' ' + modError : t('wasmLoading'))),
        ),
        h('div', { className: 'rvw-grid' },
          h('div', null,
            h('label', { className: 'rvw-label' }, t('srcLabel')),
            h('textarea', {
              className: 'rvw-ta src',
              value: src,
              spellCheck: false,
              onChange: (event) => setSrc(event.target.value),
            }),
            h('div', { className: 'rvw-row' },
              h('input', {
                className: 'rvw-in top',
                placeholder: t('topLabel'),
                value: top,
                onChange: (event) => setTop(event.target.value),
              }),
            ),
            h('label', { className: 'rvw-label' }, t('filesLabel')),
            h('textarea', {
              className: 'rvw-ta files',
              value: filesText,
              spellCheck: false,
              placeholder: '{"data.hex": "11\\n22"}',
              onChange: (event) => setFilesText(event.target.value),
            }),
            engine === 'riv' ? h('p', { className: 'rvw-note' }, t('rivNote')) : null,
            h('label', { className: 'rvw-label' }, t('stdout')),
            h('pre', { className: 'rvw-pre' + (failed ? ' err' : '') }, stdout || t('noRun')),
          ),
          h('div', null,
            h('label', { className: 'rvw-label' }, t('waveform')),
            h(Wave, { t, mod, vcdText }),
          ),
        ),
      );
    }

    function VerifyCard({ t, mod, target }) {
      const [busy, setBusy] = React.useState(false);
      const [stage, setStage] = React.useState('idle');
      const [stdout, setStdout] = React.useState('');
      const [match, setMatch] = React.useState(null);
      const [vcdText, setVcdText] = React.useState('');

      const run = async () => {
        setBusy(true);
        setMatch(null);
        setStage('running');
        try {
          const result = await runCorpusCase(mod, target);
          setStdout(result.stdout);
          setMatch(result.match);
          setVcdText(result.vcdText);
          setStage('done');
        } catch (error) {
          setStdout(String(error));
          setStage('error');
        }
        setBusy(false);
      };

      let badge = '';
      let badgeClass = 'rvw-badge';
      if (stage === 'running') {
        badge = t('running');
        badgeClass += ' busy';
      } else if (stage === 'error') {
        badge = t('error');
        badgeClass += ' bad';
      } else if (stage === 'done') {
        badge = match ? t('matchBadge') : t('diffBadge');
        badgeClass += match ? ' ok' : ' bad';
      }

      return h(
        'div',
        { className: 'rvw-card' },
        h('h3', null, t(target.titleKey)),
        h('div', { className: 'rvw-desc' }, t(target.descKey)),
        h('div', { className: 'rvw-row' },
          h('button', { className: 'rvw-btn', onClick: run, disabled: busy || !mod }, t('runVerify')),
          badge ? h('span', { className: badgeClass }, badge) : null,
        ),
        h('pre', { className: 'rvw-pre' }, stdout || t('noRun')),
        h(Wave, { t, mod, vcdText }),
      );
    }

    function SpecCard({ t, mod }) {
      const [variant, setVariant] = React.useState(0);
      const [busy, setBusy] = React.useState(false);
      const [result, setResult] = React.useState(null);
      const [failed, setFailed] = React.useState('');
      const [rtl, setRtl] = React.useState('');

      React.useEffect(() => {
        if (!mod) return;
        try {
          setRtl(mod.i2c_rtl(variant));
        } catch (error) {
          setRtl(String(error));
        }
      }, [mod, variant]);

      const run = () => {
        if (!mod) return;
        setBusy(true);
        setFailed('');
        try {
          const check = mod.i2c_spec_check(variant);
          const rows = [];
          const limit = Math.min(check.violationCount, 20);
          for (let i = 0; i < limit; i++) {
            rows.push({
              cycle: String(check.violation_cycle(i)),
              time: String(check.violation_time(i)),
              kind: check.violation_kind(i),
              expected: check.violation_expected(i),
              actual: check.violation_actual(i),
            });
          }
          setResult({
            cycles: check.cyclesRun,
            count: check.violationCount,
            rows,
            more: check.violationCount - limit,
            summary: check.render_text(),
            vcdText: check.vcd || '',
          });
        } catch (error) {
          setFailed(String(error));
          setResult(null);
        }
        setBusy(false);
      };

      let badge = null;
      if (failed) badge = h('span', { className: 'rvw-badge bad' }, t('error'));
      else if (result) {
        const ok = result.count === 0;
        badge = h('span', { className: 'rvw-badge ' + (ok ? 'ok' : 'bad') },
          (ok ? t('specOk') : t('specBad')) + ': ' + result.cycles + ' ' + t('cycles') + ', ' + result.count + ' ' + t('violations'));
      }

      return h(
        'div',
        { className: 'rvw-card' },
        h('div', { className: 'rvw-variants' },
          VARIANTS.map((key, index) =>
            h('label', { key },
              h('input', {
                type: 'radio',
                name: 'rvw-spec-variant',
                checked: variant === index,
                onChange: () => setVariant(index),
              }),
              ' ' + t(key),
            ),
          ),
        ),
        h('div', { className: 'rvw-row' },
          h('button', { className: 'rvw-btn', onClick: run, disabled: busy || !mod }, t('runSpec')),
          badge,
        ),
        result && result.rows.length
          ? h('table', { className: 'rvw-table' },
              h('thead', null,
                h('tr', null,
                  h('th', null, t('colCycle')),
                  h('th', null, t('colTime')),
                  h('th', null, t('colKind')),
                  h('th', null, t('colExpected')),
                  h('th', null, t('colActual')),
                ),
              ),
              h('tbody', null,
                result.rows.map((row, index) =>
                  h('tr', { key: index },
                    h('td', null, row.cycle),
                    h('td', null, row.time),
                    h('td', null, row.kind),
                    h('td', null, row.expected),
                    h('td', null, row.actual),
                  ),
                ),
              ),
            )
          : null,
        result && result.more > 0
          ? h('div', { className: 'rvw-status' }, '… +' + result.more)
          : null,
        failed ? h('pre', { className: 'rvw-pre err' }, failed) : null,
        result ? h('pre', { className: 'rvw-pre' }, result.summary) : null,
        result ? h(Wave, { t, mod, vcdText: result.vcdText }) : null,
        h('details', { className: 'rvw-details' },
          h('summary', null, t('rtl')),
          h('pre', { className: 'rvw-pre' }, rtl),
        ),
      );
    }

    function Panel() {
      const t = useTranslate(ctx);
      const [mod, setMod] = React.useState(null);
      const [modError, setModError] = React.useState('');

      React.useEffect(() => {
        let alive = true;
        loadWasm()
          .then((engine) => {
            if (alive) setMod(engine);
          })
          .catch((error) => {
            if (alive) setModError(String(error));
          });
        return () => {
          alive = false;
        };
      }, []);

      return h(
        'div',
        { className: 'rvw-root' },
        h('style', null, CSS),
        h(
          'div',
          { className: 'rvw-inner' },
          h('div', { className: 'rvw-head' },
            h('h1', { className: 'rvw-title' }, t('panelTitle')),
          ),
          h('p', { className: 'rvw-sub' }, t('panelSubtitle')),
          h(Playground, { t, mod, modError }),
          h('h2', { className: 'rvw-h2' }, t('verifyHeading')),
          h('p', { className: 'rvw-sub' }, t('verifyIntro')),
          CASES.map((target) => h(VerifyCard, { key: target.dir, t, mod, target })),
          h('h2', { className: 'rvw-h2' }, t('specHeading')),
          h('p', { className: 'rvw-sub' }, t('specIntro')),
          h(SpecCard, { t, mod }),
        ),
      );
    }

    /** Sidebar icon; `active` tracks whether this panel is the selected one. */
    function PanelIcon({ size = 18, active = false }) {
      return h(
        'svg',
        {
          width: size,
          height: size,
          viewBox: '0 0 24 24',
          fill: 'none',
          stroke: 'currentColor',
          strokeWidth: active ? 2 : 1.6,
          strokeLinecap: 'round',
          strokeLinejoin: 'round',
          'aria-hidden': true,
          style: { display: 'block', color: active ? 'var(--dsw-alias-brand-primary)' : 'inherit' },
        },
        h('path', { d: 'M3 9h6l2-4 2 8 2-4h6' }),
        h('path', { d: 'M3 16h18' }),
      );
    }

    // ------------------------------------------------------------------ apply

    let ctx = null;

    return {
      inject: ['slots', 'locale'],
      apply(pluginCtx) {
        ctx = pluginCtx;
        // The shipped built-in locale ids are `en` and `zh` (dsh-client-locale
        // README: "register(ns, { zh, en })"). Registering only zh-CN/zh-TW
        // silently falls back to English while the UI is visibly Chinese.
        pluginCtx.effect(() => pluginCtx.locale.register(NS, 'en', EN), 'rverilator-wasm: en dictionary');
        pluginCtx.effect(() => pluginCtx.locale.register(NS, 'zh', ZH_CN), 'rverilator-wasm: zh dictionary');
        pluginCtx.effect(() => pluginCtx.locale.register(NS, 'zh-CN', ZH_CN), 'rverilator-wasm: zh-CN dictionary');
        pluginCtx.effect(() => pluginCtx.locale.register(NS, 'zh-TW', ZH_TW), 'rverilator-wasm: zh-TW dictionary');

        const t = pluginCtx.locale.bind(NS);
        pluginCtx.slots.inject('sidebar.panellist', () =>
          pluginCtx.slots.register(
            {
              name: 'sidebar.panellist',
              id: PANEL_ID,
              order: 20,
              label: () => t('panelTitle'),
            },
            PanelIcon,
          ),
        );
        pluginCtx.slots.inject('main', () =>
          pluginCtx.slots.register({ name: 'main', key: PANEL_ID }, Panel),
        );
      },
    };
  },
});
