//! Local demo server for the wasm playground (repo rule: local preview
//! servers must send `Cache-Control: no-store` — heuristic caches eat
//! stale bundles). Serves wasm-demo/, the built pkg/ (../pkg), and a
//! prefix-allowlisted slice of the repo corpus/ (the XLS verification
//! cards fetch design + TB + recorded expectation from the single source
//! of truth instead of an inline copy that would drift).

const demoRoot = new URL(".", import.meta.url).pathname;
const pkgRoot = new URL("../pkg/", import.meta.url).pathname;
const repoRoot = new URL("../../..", import.meta.url).pathname;
// Prefix allowlist — the demo server must NOT become a repo-wide file reader.
const CORPUS_ALLOW = new Set(["51_vcd_counter", "57_xls_add3", "58_xls_acc_pipe"]);

Bun.serve({
  port: Number(process.env.PORT ?? 8031),
  fetch(req) {
    const url = new URL(req.url);
    const rel = decodeURIComponent(url.pathname === "/" ? "/index.html" : url.pathname);
    if (rel.startsWith("/corpus/e2e/")) {
      const rest = rel.slice("/corpus/e2e/".length);
      const caseDir = rest.split("/")[0];
      if (!CORPUS_ALLOW.has(caseDir) || !rest.includes("/")) {
        return new Response("forbidden", { status: 403 });
      }
      const path = require("node:path").join(repoRoot, "corpus/e2e", rest);
      if (!path.startsWith(require("node:path").join(repoRoot, "corpus/e2e"))) {
        return new Response("forbidden", { status: 403 });
      }
      return new Response(Bun.file(path), { headers: { "Cache-Control": "no-store" } });
    }
    const root = rel.startsWith("/pkg/") ? pkgRoot : demoRoot;
    const path = require("node:path").join(root, rel.slice("/pkg/".length * Number(rel.startsWith("/pkg/"))));
    if (!path.startsWith(demoRoot) && !path.startsWith(pkgRoot)) {
      return new Response("forbidden", { status: 403 });
    }
    return new Response(Bun.file(path), { headers: { "Cache-Control": "no-store" } });
  },
});
console.log(`rverilator.wasm demo → http://127.0.0.1:${process.env.PORT ?? 8031}`);
