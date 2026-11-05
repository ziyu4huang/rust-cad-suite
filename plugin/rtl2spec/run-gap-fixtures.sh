#!/usr/bin/env bash
# run-gap-fixtures.sh — 破口 fixture 矩陣閘(總閘成員;§16.2)。
# ①守衛=bun/wasm-pack 在場+矩陣 fixture 檔在場(缺→exit 2 逐一列出)
# ②pkg 陳舊檢查+同步 dist/wasm(6 個傳遞依賴目錄 find -newer,D8 同形;
#   閘驗出貨物=比對對象是交付的 dist wasm 裝載面)
# ③bun scripts/gap-fixture-check.mjs(單進程;檔級 (module,class,confidence)
#   多重集合相等斷言+msgSubs 抽點+終止性守衛 12 跑+selfcheck 判別力自證)
# ④任一判據敗→exit 1
# 與 sv_rules.rs 單測的分工(§16.2):單測=開發期紅綠;本閘=交付面
# (dist wasm 裝載)執法+expected findings 全表結構化+終止性守衛。
set -u
PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd)"

echo "== rtl2spec run-gap-fixtures =="

# ① 守衛
command -v bun >/dev/null 2>&1 || { echo "FAIL guard: bun not in PATH"; exit 2; }
command -v wasm-pack >/dev/null 2>&1 || { echo "FAIL guard: wasm-pack not in PATH"; exit 2; }
export RTL2SPEC_BUN_VERSION="$(bun --version)"
echo "PASS guard: bun $RTL2SPEC_BUN_VERSION, wasm-pack $(wasm-pack --version)"

FIXTURES=(
  "crates/rtl-spec/tests/fixtures/gaps_positive.v"
  "crates/rtl-spec/tests/fixtures/gaps_negative.v"
  "plugin/rtl2spec/fixtures/gap_missing_reset_no_arm.v"
  "plugin/rtl2spec/fixtures/gap_hang_repro.v"
)
MISSING=()
for f in "${FIXTURES[@]}"; do
  [ -f "$REPO_ROOT/$f" ] || MISSING+=("$f")
done
if [ "${#MISSING[@]}" -gt 0 ]; then
  echo "FAIL guard: 矩陣 fixture 缺 ${#MISSING[@]} 檔:"
  for f in "${MISSING[@]}"; do echo "  - $f"; done
  exit 2
fi
echo "PASS guard: 矩陣 fixture ${#FIXTURES[@]} 檔在場"

# ② pkg 陳舊檢查 + 同步(與 run-rtl-validation.sh 同形;自包含)
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

# ③④ 判據
cd "$REPO_ROOT"
bun "$PLUGIN_ROOT/scripts/gap-fixture-check.mjs"
rc=$?
if [ "$rc" -ne 0 ]; then
  echo "== run-gap-fixtures FAIL (exit $rc) =="
  exit "$rc"
fi
echo "== run-gap-fixtures PASS =="
exit 0
