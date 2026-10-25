/**
 * Host half of the rverilator.wasm playground bundle.
 *
 * The simulation itself runs in the browser (WebAssembly), so this half has one
 * job: hand the browser exactly three things over the Harness' own HTTP origin,
 * without turning the GUI into a repo-wide file reader.
 *
 *   /rverilator-wasm/pkg/*                    the wasm-bindgen artifact
 *                                             (crates/vl-wasm/pkg, rebuilt by
 *                                             scripts/wasm-build or regress)
 *   /rverilator-wasm/corpus/e2e/<case>/<file> the oracle-recorded corpus slice
 *                                             the verification cards compare
 *                                             against (three allowlisted cases)
 *
 * Repo rules that the route honours:
 *   - `Cache-Control: no-store` on every response (local preview servers must
 *     not let a heuristic cache serve a stale wasm bundle — AGENTS/corpus rule).
 *   - Prefix allowlist, no traversal: the corpus route only serves the three
 *     cases the demo verifies, and a filename must be a single plain path
 *     segment. There is no query-string or `..` surface.
 */
import { createReadStream } from 'node:fs'
import { stat } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

// Registered WITHOUT a trailing slash: the carrier matches a prefix as
// `pathname === prefix || pathname.startsWith(prefix + '/')`, so a trailing
// slash here would compile to a `//` test and never match.
const PREFIX = '/rverilator-wasm'

/** Plugin package directory: <repo>/dsh-plugins/rverilator-wasm-playground/ */
const PLUGIN_ROOT = fileURLToPath(new URL('.', import.meta.url))
const PKG_ROOT = resolve(PLUGIN_ROOT, '../../crates/vl-wasm/pkg')
const CORPUS_ROOT = resolve(PLUGIN_ROOT, '../../corpus/e2e')

/** The only corpus cases the browser may read (mirrors the demo's allowlist). */
const ALLOWED_CASES = new Set(['51_vcd_counter', '57_xls_add3', '58_xls_acc_pipe'])

/** One plain path segment: no separators, no `..`, no absolute paths. */
const SEGMENT = /^[A-Za-z0-9_][A-Za-z0-9_.-]*$/

const MIME = {
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.json': 'application/json; charset=utf-8',
  '.v': 'text/plain; charset=utf-8',
  '.txt': 'text/plain; charset=utf-8',
  '.stdout': 'text/plain; charset=utf-8',
  '.d.ts': 'text/plain; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
}

function mimeOf(path) {
  if (path.endsWith('.d.ts')) return MIME['.d.ts']
  const dot = path.lastIndexOf('.')
  const ext = dot === -1 ? '' : path.slice(dot)
  return MIME[ext] ?? 'application/octet-stream'
}

function fail(res, status, message) {
  if (res.headersSent) return
  res.writeHead(status, {
    'Content-Type': 'text/plain; charset=utf-8',
    'Cache-Control': 'no-store',
  })
  res.end(message + '\n')
}

async function serveFile(path, res) {
  let info
  try {
    info = await stat(path)
  } catch {
    return fail(res, 404, 'not found')
  }
  if (!info.isFile()) return fail(res, 404, 'not found')
  res.writeHead(200, {
    'Content-Type': mimeOf(path),
    'Content-Length': String(info.size),
    'Cache-Control': 'no-store',
  })
  const stream = createReadStream(path)
  stream.on('error', () => res.destroy())
  stream.pipe(res)
}

/**
 * Route handler. `req`/`res` are the Node HTTP pair the Web carrier hands over;
 * only prefix paths under `/rverilator-wasm/` reach here.
 */
async function handle(req, res) {
  if (req.method !== 'GET' && req.method !== 'HEAD') {
    return fail(res, 405, 'method not allowed')
  }
  let rel
  try {
    const pathname = new URL(req.url ?? '/', 'http://localhost').pathname
    if (!pathname.startsWith(PREFIX + '/')) return fail(res, 404, 'not found')
    rel = decodeURIComponent(pathname.slice(PREFIX.length + 1))
  } catch {
    return fail(res, 400, 'bad path')
  }
  if (rel.endsWith('/')) rel = rel.slice(0, -1)

  if (rel.startsWith('pkg/')) {
    const name = rel.slice('pkg/'.length)
    if (!SEGMENT.test(name)) return fail(res, 403, 'forbidden')
    return serveFile(join(PKG_ROOT, name), res)
  }

  if (rel.startsWith('corpus/e2e/')) {
    const parts = rel.slice('corpus/e2e/'.length).split('/')
    if (parts.length !== 2) return fail(res, 403, 'forbidden')
    const [caseDir, file] = parts
    if (!ALLOWED_CASES.has(caseDir) || !SEGMENT.test(file)) return fail(res, 403, 'forbidden')
    return serveFile(join(CORPUS_ROOT, caseDir, file), res)
  }

  return fail(res, 404, 'not found')
}

export const inject = ['webServer']

export function apply(ctx) {
  ctx.effect(
    () => ctx.webServer.register({ kind: 'prefix', path: PREFIX, handler: handle }),
    'rverilator-wasm: wasm artifact + corpus asset route',
  )
}
