#!/usr/bin/env bash
# run-mcp-e2e.sh — MCP 端到端閘(總閘成員;§8.1)。
# ①守衛=bun/wasm-pack 在場(缺→exit 2 表意,不靜默跳過)
# ②pkg 陳舊檢查(find 全傳遞依賴目錄 -newer pkg/...bg.wasm → wasm-pack
#   rebuild --target web;漏列傳遞依賴=雙閘驗陳舊 wasm,「閘驗出貨物」
#   (D8)即謊言)→ 同步 pkg→plugin/rtl2spec/dist/wasm
# ③bun scripts/mcp-client.mjs 回環斷言(19 條=實跑 PASS 行數,iverilog 在場時;§19 C-S2 起含 ⑧ max_rules 四調用+⑨⑩⑪ rtl_tb_plan)
# ④斷言失敗→exit 1;構建失敗→exit 3;成功→exit 0
set -u
PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd)"

echo "== rtl2spec run-mcp-e2e =="

# ① 守衛
command -v bun >/dev/null 2>&1 || { echo "FAIL guard: bun not in PATH"; exit 2; }
command -v wasm-pack >/dev/null 2>&1 || { echo "FAIL guard: wasm-pack not in PATH"; exit 2; }
echo "PASS guard: bun $(bun --version), wasm-pack $(wasm-pack --version)"

# ② pkg 陳舊檢查 + 同步(傳遞依賴目錄必須列全——R1-6)
PKG_WASM="$REPO_ROOT/crates/rtl-spec-wasm/pkg/rtl_spec_wasm_bg.wasm"
DEP_DIRS=(
  "$REPO_ROOT/crates/rtl-spec/src"
  "$REPO_ROOT/crates/rtl-spec-wasm/src"
  "$REPO_ROOT/crates/analyzer-core/src"
  "$REPO_ROOT/crates/vl-syntax/src"
  "$REPO_ROOT/crates/ry-ir/src"
  "$REPO_ROOT/crates/rcs-diag/src"
)
STALE=0
if [ ! -f "$PKG_WASM" ]; then
  STALE=1
else
  for d in "${DEP_DIRS[@]}"; do
    if [ -n "$(find "$d" -name '*.rs' -newer "$PKG_WASM" -print -quit 2>/dev/null)" ]; then
      STALE=1
      break
    fi
  done
fi
if [ "$STALE" = "1" ]; then
  echo "INFO pkg stale (or missing) — rebuilding"
  (cd "$REPO_ROOT" && wasm-pack build crates/rtl-spec-wasm --target web --out-dir pkg) \
    || { echo "FAIL wasm-pack build"; exit 3; }
else
  echo "PASS pkg fresh"
fi
mkdir -p "$PLUGIN_ROOT/dist/wasm"
cp "$REPO_ROOT/crates/rtl-spec-wasm/pkg/rtl_spec_wasm.js" "$PLUGIN_ROOT/dist/wasm/" \
  || { echo "FAIL sync glue"; exit 3; }
cp "$REPO_ROOT/crates/rtl-spec-wasm/pkg/rtl_spec_wasm_bg.wasm" "$PLUGIN_ROOT/dist/wasm/" \
  || { echo "FAIL sync wasm"; exit 3; }
cp "$REPO_ROOT/crates/rtl-spec-wasm/pkg/rtl_spec_wasm.d.ts" "$PLUGIN_ROOT/dist/wasm/" 2>/dev/null \
  || echo "INFO glue .d.ts not present (optional)"
echo "PASS sync dist/wasm"

# ③ 回環斷言(19 條,iverilog 在場時)
echo "== MCP stdio loopback =="
bun "$PLUGIN_ROOT/scripts/mcp-client.mjs"
CLIENT_EXIT=$?
if [ "$CLIENT_EXIT" -ne 0 ]; then
  echo "FAIL mcp-client assertions (exit $CLIENT_EXIT)"
  exit 1
fi

# manifest 靜態校驗(裝載驗證第一步;同閘附帶)
bun "$PLUGIN_ROOT/scripts/validate-manifest.mjs"
MANIFEST_EXIT=$?
if [ "$MANIFEST_EXIT" -ne 0 ]; then
  echo "FAIL manifest validation (exit $MANIFEST_EXIT)"
  exit 1
fi

echo "== run-mcp-e2e PASS =="
exit 0
