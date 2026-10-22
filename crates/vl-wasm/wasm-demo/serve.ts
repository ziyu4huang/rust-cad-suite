//! Local demo server for the wasm playground (repo rule: local preview
//! servers must send `Cache-Control: no-store` — heuristic caches eat
//! stale bundles). Serves wasm-demo/ plus the built pkg/ (../pkg).

const demoRoot = new URL(".", import.meta.url).pathname;
const pkgRoot = new URL("../pkg/", import.meta.url).pathname;

Bun.serve({
  port: Number(process.env.PORT ?? 8031),
  fetch(req) {
    const url = new URL(req.url);
    const rel = decodeURIComponent(url.pathname === "/" ? "/index.html" : url.pathname);
    const root = rel.startsWith("/pkg/") ? pkgRoot : demoRoot;
    const path = require("node:path").join(root, rel.slice("/pkg/".length * Number(rel.startsWith("/pkg/"))));
    if (!path.startsWith(demoRoot) && !path.startsWith(pkgRoot)) {
      return new Response("forbidden", { status: 403 });
    }
    return new Response(Bun.file(path), { headers: { "Cache-Control": "no-store" } });
  },
});
console.log(`rverilator.wasm demo → http://127.0.0.1:${process.env.PORT ?? 8031}`);
