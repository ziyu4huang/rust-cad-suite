/**
 * Bundle smoke test (Node).
 *
 * Three layers, all against the shipped files, no browser required:
 *
 *   1. Host half — mount the route handler with a fake ctx and assert the
 *      asset contract: 200 + MIME + bytes for the wasm artifact, the corpus
 *      allowlist, 403 on a non-allowlisted case and on traversal, 405 on POST.
 *   2. Registration — evaluate client.js against a fake module loader, apply
 *      the returned plugin to a fake ctx, and assert the `main` panel key, the
 *      `sidebar.panellist` id that addresses it, and both locale dictionaries.
 *   3. Rendering + real simulation — with react-dom (installed outside the
 *      repo; see the header of the run command in README) render the panel in
 *      jsdom, click Run, and assert stdout `q=8` plus a rendered SVG wave; then
 *      the riverilog face (`$finish called at 76000`) and a corpus card's
 *      byte-exact badge.
 *
 * The wasm engine is the real artifact; jsdom cannot execute `import()` over
 * HTTP, so the test supplies the imported module through the documented
 * `window.__rverilatorWasmModule` embedding seam.
 *
 * Run: node test/smoke.mjs   (NODE_PATH must point at a node_modules holding
 * react / react-dom / jsdom)
 */
import { readFile } from 'node:fs/promises'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { createRequire } from 'node:module'
import { Writable } from 'node:stream'
import { dirname, resolve } from 'node:path'

const require = createRequire(import.meta.url)
const here = dirname(fileURLToPath(import.meta.url))
const pluginRoot = resolve(here, '..')
const repoRoot = resolve(pluginRoot, '../..')
const pkgRoot = resolve(repoRoot, 'crates/vl-wasm/pkg')

const React = require('react')
const { renderToString } = require('react-dom/server')
const { JSDOM } = require('jsdom')

let failures = 0
function check(name, condition, detail) {
  if (condition) {
    console.log('  PASS  ' + name)
  } else {
    failures += 1
    console.log('  FAIL  ' + name + (detail === undefined ? '' : ' :: ' + detail))
  }
}

// ---------------------------------------------------------------- 1. host

console.log('Host asset route')

const index = await import(pathToFileURL(resolve(pluginRoot, 'index.js')).href)
const routes = []
const hostCtx = {
  effect(fn) {
    const disposer = fn()
    return () => disposer?.()
  },
  webServer: {
    register(route) {
      routes.push(route)
      return () => {}
    },
  },
}
index.apply(hostCtx)
check('registers exactly one route', routes.length === 1, String(routes.length))
const route = routes[0]
check('route is a prefix without a trailing slash', route?.kind === 'prefix' && route?.path === '/rverilator-wasm', JSON.stringify(route?.path))

// The carrier's matcher: `pathname === prefix || pathname.startsWith(prefix + '/')`.
// Re-derive it here so a trailing slash regression fails this test, not the page.
const carrierMatches = (pathname) => pathname === route.path || pathname.startsWith(route.path + '/')
check('carrier-style prefix match hits the asset path', carrierMatches('/rverilator-wasm/pkg/vl_wasm.js'))
check('carrier-style prefix match rejects a sibling path', !carrierMatches('/rverilator-wasmx/pkg/vl_wasm.js'))
check('carrier-style prefix match rejects the bare prefix', !carrierMatches('/rverilator-wasm-only'))

function fakeRes() {
  const out = { status: 0, headers: {}, body: Buffer.alloc(0) }
  const res = new Writable({
    write(chunk, _encoding, callback) {
      out.body = Buffer.concat([out.body, Buffer.from(chunk)])
      callback()
    },
  })
  res.writeHead = (status, headers) => {
    out.status = status
    out.headers = headers ?? {}
  }
  res.destroy = () => {}
  return { res, out, done: () => new Promise((done) => res.on('finish', done)) }
}

async function callRoute(path, method = 'GET') {
  const { res, out, done } = fakeRes()
  await route.handler({ url: path, method }, res)
  await done()
  return out
}

const jsAsset = await callRoute('/rverilator-wasm/pkg/vl_wasm.js')
check('pkg/vl_wasm.js → 200', jsAsset.status === 200, String(jsAsset.status))
check('pkg/vl_wasm.js → javascript MIME', /text\/javascript/.test(jsAsset.headers['Content-Type'] ?? ''), jsAsset.headers['Content-Type'])
check('pkg/vl_wasm.js → no-store', jsAsset.headers['Cache-Control'] === 'no-store', jsAsset.headers['Cache-Control'])
check('pkg/vl_wasm.js body matches disk', jsAsset.body.equals(await readFile(resolve(pkgRoot, 'vl_wasm.js'))))

const wasmAsset = await callRoute('/rverilator-wasm/pkg/vl_wasm_bg.wasm')
check('pkg/vl_wasm_bg.wasm → 200 + application/wasm', wasmAsset.status === 200 && wasmAsset.headers['Content-Type'] === 'application/wasm', wasmAsset.headers['Content-Type'])
check('wasm body is the real artifact', wasmAsset.body.equals(await readFile(resolve(pkgRoot, 'vl_wasm_bg.wasm'))))

const goodCase = await callRoute('/rverilator-wasm/corpus/e2e/51_vcd_counter/tb.v')
check('allowlisted corpus file → 200', goodCase.status === 200, String(goodCase.status))

const otherCase = await callRoute('/rverilator-wasm/corpus/e2e/01_hello/tb.v')
check('non-allowlisted case → 403', otherCase.status === 403, String(otherCase.status))

const traversal = await callRoute('/rverilator-wasm/corpus/e2e/51_vcd_counter/..%2f..%2fCargo.toml')
check('traversal → 403', traversal.status === 403, String(traversal.status))

const escapePkg = await callRoute('/rverilator-wasm/pkg/..%2f..%2fCargo.toml')
check('pkg traversal → 403', escapePkg.status === 403, String(escapePkg.status))

const post = await callRoute('/rverilator-wasm/pkg/vl_wasm.js', 'POST')
check('POST → 405', post.status === 405, String(post.status))

// The XLS provenance surface: manifest + the three source forms + licence.
const manifestRes = await callRoute('/rverilator-wasm/xls/manifest.json')
const manifestBody = JSON.parse(manifestRes.body.toString('utf8'))
check('xls manifest → 200 JSON', manifestRes.status === 200 && /application\/json/.test(manifestRes.headers['Content-Type'] ?? ''), manifestRes.headers['Content-Type'])
check('xls manifest is no-store', manifestRes.headers['Cache-Control'] === 'no-store', manifestRes.headers['Cache-Control'])
check('xls manifest lists the harvested cases', manifestBody.cases?.map((c) => c.id).join(',') === 'acc_pipe,add3', JSON.stringify(manifestBody.cases?.map((c) => c.id)))
const add3Entry = manifestBody.cases?.find((c) => c.id === 'add3')
check('manifest joins DSLX/IR/Verilog to the e2e case', add3Entry?.dslx === 'dslx/add3.x' && add3Entry?.ir === 'generated/add3.ir' && add3Entry?.verilog === 'generated/add3.v' && add3Entry?.e2e?.dir === '57_xls_add3', JSON.stringify(add3Entry))
check('manifest marks the facade forced dump', add3Entry?.forceDump === true && add3Entry?.e2e?.expected === 'expected.stdout', JSON.stringify(add3Entry && { forceDump: add3Entry.forceDump, expected: add3Entry.e2e?.expected }))

const dslxAsset = await callRoute('/rverilator-wasm/xls/dslx/add3.x')
check('xls dslx → 200 text', dslxAsset.status === 200 && /text\/plain/.test(dslxAsset.headers['Content-Type'] ?? ''), dslxAsset.headers['Content-Type'])
check('xls dslx body is the real source', dslxAsset.body.toString('utf8').includes('fn add3'))

const irAsset = await callRoute('/rverilator-wasm/xls/generated/add3.ir')
check('xls IR → 200 with the lowered ops', irAsset.status === 200 && irAsset.body.toString('utf8').includes('zero_ext'), String(irAsset.status))

const xlsVerilog = await callRoute('/rverilator-wasm/xls/generated/add3.v')
check('xls generated Verilog → 200', xlsVerilog.status === 200 && xlsVerilog.body.toString('utf8').includes('__add3__add3'), String(xlsVerilog.status))

const xlsLicense = await callRoute('/rverilator-wasm/xls/XLS-LICENSE.txt')
check('XLS licence travels with the snapshot', xlsLicense.status === 200 && xlsLicense.body.toString('utf8').includes('Apache License'), String(xlsLicense.status))

const xlsTraversal = await callRoute('/rverilator-wasm/xls/dslx/..%2f..%2fCargo.toml')
check('xls traversal → 403', xlsTraversal.status === 403, String(xlsTraversal.status))

const xlsWrongExt = await callRoute('/rverilator-wasm/xls/generated/top.sh')
check('xls unexpected extension → 403', xlsWrongExt.status === 403, String(xlsWrongExt.status))

// --------------------------------------------------------- 2. registration

console.log('\nClient registration')

const clientCode = await readFile(resolve(pluginRoot, 'client.js'), 'utf8')

const wasmMod = await import(pathToFileURL(resolve(pkgRoot, 'vl_wasm.js')).href)
await wasmMod.default({ module_or_path: await readFile(resolve(pkgRoot, 'vl_wasm_bg.wasm')) })

function localeService() {
  const dicts = new Map()
  let active = 'en'
  return {
    dicts,
    setActive(id) {
      active = id
    },
    register(ns, locale, dict) {
      dicts.set(ns + '/' + locale, dict)
      return () => {}
    },
    bind(ns) {
      return (key) => dicts.get(ns + '/' + active)?.[key] ?? dicts.get(ns + '/en')?.[key] ?? key
    },
    subscribe() {
      return () => {}
    },
  }
}

function makePluginCtx() {
  const slots = []
  const locale = localeService()
  const ctx = {
    locale,
    effect(fn) {
      const disposer = fn()
      return () => disposer?.()
    },
    slots: {
      inject(slot, callback) {
        slots.push({ slot, registration: callback() })
      },
      register(options, component) {
        return { ...options, component }
      },
    },
  }
  return { ctx, slots, locale }
}

let captured
const fakeWindow = {
  __ModuleLoader__: {
    load(definition) {
      captured = definition
    },
  },
  __rverilatorWasmModule: wasmMod,
}
const fakeLocation = new URL('http://localhost/')
const fakeFetch = async () => {
  throw new Error('fetch is not used by this layer')
}
new Function('window', 'location', 'fetch', 'URL', clientCode)(fakeWindow, fakeLocation, fakeFetch, URL)

check('client bundle registers one factory', Boolean(captured), JSON.stringify(captured?.id))
check('factory id equals the package name', captured?.id === '@local/rverilator-wasm-playground', captured?.id)

const plugin = captured.factory((id) => {
  if (id === 'react') return React
  throw new Error('unexpected module: ' + id)
})
check('plugin injects slots + locale', Array.isArray(plugin.inject) && plugin.inject.includes('slots') && plugin.inject.includes('locale'), JSON.stringify(plugin.inject))

const primary = makePluginCtx()
plugin.apply(primary.ctx)

const mainRegistration = primary.slots.find((entry) => entry.slot === 'main')?.registration
const panelRegistration = primary.slots.find((entry) => entry.slot === 'sidebar.panellist')?.registration
check('registers the main panel key', mainRegistration?.name === 'main' && mainRegistration?.key === 'rverilator-wasm', JSON.stringify(mainRegistration && { name: mainRegistration.name, key: mainRegistration.key }))
check('registers the panellist id that addresses it', panelRegistration?.name === 'sidebar.panellist' && panelRegistration?.id === 'rverilator-wasm' && panelRegistration?.order === 20, JSON.stringify(panelRegistration && { id: panelRegistration.id, order: panelRegistration.order }))
check('panellist label comes from the locale dictionary', panelRegistration?.label?.() === 'Verilog Playground', panelRegistration?.label?.())
primary.locale.setActive('zh-TW')
check('panellist label follows the active locale', panelRegistration?.label?.() === 'Verilog 模擬台', panelRegistration?.label?.())
check('every dictionary registered', ['en', 'zh', 'zh-CN', 'zh-TW'].every((id) => primary.locale.dicts.has('rverilatorWasm/' + id)), [...primary.locale.dicts.keys()].join(','))
primary.locale.setActive('zh')
check('the shipped `zh` locale resolves the dictionary', panelRegistration?.label?.() === 'Verilog 模拟台', panelRegistration?.label?.())
primary.locale.setActive('zh-CN')
check('zh-CN dictionary follows the zh-CN UI locale', panelRegistration?.label?.() === 'Verilog 模拟台', panelRegistration?.label?.())
primary.locale.setActive('en')

// --------------------------------------------------- 3. render + simulate

console.log('\nPanel render + real wasm simulation')

primary.locale.setActive('en')

const markup = renderToString(React.createElement(mainRegistration.component))
check('SSR renders the panel title', markup.includes('Verilog Playground'), markup.slice(0, 200))
check('SSR renders the source editor', markup.includes('<textarea') && markup.includes('rvw-ta src'))
check('SSR renders the Run button', markup.includes('Run</button>'))
check('SSR renders the engine selector', markup.includes('riverilog'))
check('SSR renders the XLS bridge heading', markup.includes('XLS bridge: DSLX'))

// jsdom pass: the wasm engine is genuinely exercised through the component.
const dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
  url: 'http://localhost/',
  pretendToBeVisual: true,
})
const { window: domWindow } = dom
globalThis.window = domWindow
globalThis.document = domWindow.document
Object.defineProperty(globalThis, 'navigator', { value: domWindow.navigator, configurable: true })
globalThis.HTMLElement = domWindow.HTMLElement
globalThis.IS_REACT_ACT_ENVIRONMENT = true
// Route-backed fetch: the DOM pass goes through the real Host handler, so both
// the XLS manifest scan and the corpus/XLS file allowlists are exercised.
const routeFetch = async (url) => {
  const pathname = new URL(String(url), 'http://localhost').pathname
  const out = await callRoute(pathname)
  const text = out.body.toString('utf8')
  return {
    ok: out.status === 200,
    status: out.status,
    text: async () => text,
    json: async () => JSON.parse(text),
  }
}

let domCaptured
domWindow.__ModuleLoader__ = {
  load(definition) {
    domCaptured = definition
  },
}
domWindow.__rverilatorWasmModule = wasmMod
new Function('window', 'location', 'fetch', 'URL', clientCode)(
  domWindow,
  new URL('http://localhost/'),
  routeFetch,
  URL,
)
const domPlugin = domCaptured.factory((id) => {
  if (id === 'react') return React
  throw new Error('unexpected module: ' + id)
})
const domCtx = makePluginCtx()
domPlugin.apply(domCtx.ctx)
const DomPanel = domCtx.slots.find((entry) => entry.slot === 'main').registration.component

const settle = async (ms = 60) => {
  await React.act(async () => {
    await new Promise((done) => setTimeout(done, ms))
  })
}

const { createRoot } = require('react-dom/client')
const container = domWindow.document.getElementById('root')
const root = createRoot(container)
await React.act(async () => {
  root.render(React.createElement(DomPanel))
})
const byClass = (name) => container.querySelectorAll('.' + name)

check('panel mounted with the playground source', byClass('rvw-ta')[0]?.value.includes('always @(posedge clk)'))
check('engine became available', container.textContent.includes('loading wasm engine') === false, container.textContent.slice(0, 160))

const runButton = [...container.querySelectorAll('button')].find((button) => button.textContent === 'Run')
await React.act(async () => {
  runButton.dispatchEvent(new domWindow.MouseEvent('click', { bubbles: true }))
})
await settle()
const stdoutAfterRun = byClass('rvw-pre')[0]?.textContent ?? ''
check('rverilator face prints q=8', /q=8/.test(stdoutAfterRun), JSON.stringify(stdoutAfterRun))
check('waveform SVG rendered from the shared renderer', byClass('rvw-wave')[0]?.innerHTML.includes('<svg'), byClass('rvw-wave')[0]?.innerHTML.slice(0, 120))
check('signal toggles present', byClass('rvw-sig').length > 0, String(byClass('rvw-sig').length))

const select = container.querySelector('select')
await React.act(async () => {
  select.value = 'riv'
  select.dispatchEvent(new domWindow.Event('change', { bubbles: true }))
})
await settle(10)
await React.act(async () => {
  const rivRun = [...container.querySelectorAll('button')].find((button) => button.textContent === 'Run')
  rivRun.dispatchEvent(new domWindow.MouseEvent('click', { bubbles: true }))
})
await settle()
const stdoutRiv = byClass('rvw-pre')[0]?.textContent ?? ''
check('riverilog face prints the iverilog $finish banner', /\$finish called at 76000/.test(stdoutRiv), JSON.stringify(stdoutRiv))
check('riverilog face reports RIV-1 bytes', container.textContent.includes('RIV-1 bytes'), container.textContent.slice(0, 200))

// The XLS bridge: the Host scans the snapshot, the client fetches the
// provenance panes through the same route, then byte-diffs in wasm.
await settle(120)
const xlsCards = [...container.querySelectorAll('.rvw-xls-card')]
check('XLS bridge mounted both harvested cases', xlsCards.length === 2, String(xlsCards.length))
check('XLS section names the four faces', container.textContent.includes('rv-xls-sim'), container.textContent.slice(0, 200))
const add3Card = xlsCards.find((card) => card.textContent.includes('fn add3'))
check('XLS card loaded its DSLX source', Boolean(add3Card), xlsCards.map((card) => card.textContent.slice(0, 60)).join(' | '))
if (add3Card) {
  const panes = [...add3Card.querySelectorAll('.rvw-pre.pane')].map((node) => node.textContent)
  check('XLS card shows the DSLX source', panes[0]?.includes('fn add3'), JSON.stringify(panes[0]?.slice(0, 80)))
  check('XLS card shows the lowered XLS IR', panes[1]?.includes('zero_ext'), JSON.stringify(panes[1]?.slice(0, 80)))
  check('XLS card shows the generated Verilog', panes[2]?.includes('__add3__add3'), JSON.stringify(panes[2]?.slice(0, 80)))
  check('XLS card shows the race-safe testbench', panes[3]?.includes('__add3__add3 dut'), JSON.stringify(panes[3]?.slice(0, 80)))
  const xlsRun = [...add3Card.querySelectorAll('button')].find((button) => button.textContent === 'Run + verify')
  await React.act(async () => {
    xlsRun.dispatchEvent(new domWindow.MouseEvent('click', { bubbles: true }))
  })
  await settle(200)
  check('XLS add3 run is byte-exact against the recorded expectation', add3Card.textContent.includes('byte-exact against the recorded expectation'), add3Card.textContent.match(/✓[^\n]{0,80}/)?.[0] ?? '(no badge)')
  check('XLS add3 stdout is the oracle expectation', /add3=6[\s\S]*add3=355/.test(add3Card.querySelector('.rvw-pre:not(.pane)')?.textContent ?? ''), JSON.stringify(add3Card.querySelector('.rvw-pre:not(.pane)')?.textContent))
  check('XLS add3 waveform came from the forced dump', add3Card.querySelector('.rvw-wave')?.innerHTML.includes('<svg') && add3Card.querySelectorAll('.rvw-sig').length > 0, String(add3Card.querySelectorAll('.rvw-sig').length))
}

const verifyButtons = [...container.querySelectorAll('.rvw-card:not(.rvw-xls-card) button')].filter((button) => button.textContent === 'Run + verify')
check('counter verification card mounted', verifyButtons.length === 1, String(verifyButtons.length))
await React.act(async () => {
  verifyButtons[0].dispatchEvent(new domWindow.MouseEvent('click', { bubbles: true }))
})
await settle(120)
check('counter card is byte-exact against the recorded expectation', container.textContent.includes('byte-exact against the recorded expectation'), container.textContent.match(/✓[^\n]{0,80}/)?.[0] ?? '(no badge)')

// The spec card: a broken variant must be caught and named.
const radios = [...container.querySelectorAll('input[name="rvw-spec-variant"]')]
check('three spec variants mounted', radios.length === 3, String(radios.length))
await React.act(async () => {
  radios[2].dispatchEvent(new domWindow.MouseEvent('click', { bubbles: true }))
})
await settle(10)
await React.act(async () => {
  const specRun = [...container.querySelectorAll('button')].find((button) => button.textContent === 'Run spec check')
  specRun.dispatchEvent(new domWindow.MouseEvent('click', { bubbles: true }))
})
await settle()
check('broken spec variant is caught', container.textContent.includes('spec violated'), container.textContent.match(/✗[^\n]{0,90}/)?.[0] ?? '(no badge)')
check('spec violations table is populated', container.querySelectorAll('.rvw-table tbody tr').length > 0, String(container.querySelectorAll('.rvw-table tbody tr').length))

await React.act(async () => {
  root.unmount()
})

console.log('\n' + (failures === 0 ? 'ALL CHECKS PASS' : failures + ' CHECK(S) FAILED'))
process.exit(failures === 0 ? 0 : 1)
