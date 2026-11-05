#!/usr/bin/env bash
# run-realworld-eval.sh — 真實評測擴面閘(總閘成員直調,不進 regress;§16.4)。
# ①守衛=bun/wasm-pack 在場+語料根在場(缺→exit 2)
# ②pkg 陳舊檢查+同步 dist/wasm(6 個傳遞依賴目錄 find -newer,D8 同形)
# ③bun scripts/eval-all.mjs(L1=93 檔全提取+破口掃描;L2=260 檔容錯面;
#   verdict 寫 build/eval/,gitignored;單 bun 進程循環)
# ④bun scripts/gen-eval-report.mjs(機械生成 docs/RTL2SPEC-EVAL.md+自洽斷言)
# ⑤任一敗→exit 1
# 語料唯讀:只讀 corpus/ 與 corpus-external/,寫只落 plugin/rtl2spec/build/
# 與 docs/RTL2SPEC-EVAL.md(報告=唯一出 plugin 的寫面,機械生成)。
set -u
PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd)"

echo "== rtl2spec run-realworld-eval =="

# ① 守衛
command -v bun >/dev/null 2>&1 || { echo "FAIL guard: bun not in PATH"; exit 2; }
command -v wasm-pack >/dev/null 2>&1 || { echo "FAIL guard: wasm-pack not in PATH"; exit 2; }
for d in corpus-external/yosys-simple corpus-external/sv-tests corpus; do
  [ -d "$REPO_ROOT/$d" ] || { echo "FAIL guard: 語料根缺 $d"; exit 2; }
done
export RTL2SPEC_BUN_VERSION="$(bun --version)"
echo "PASS guard: bun $RTL2SPEC_BUN_VERSION, wasm-pack $(wasm-pack --version), 語料根在場"

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

# ③④ 執行(verdict 殘留清空防 stale 檔混入)
rm -rf "$PLUGIN_ROOT/build/eval"
mkdir -p "$PLUGIN_ROOT/build/eval"
cd "$REPO_ROOT"
bun "$PLUGIN_ROOT/scripts/eval-all.mjs"
rc1=$?
bun "$PLUGIN_ROOT/scripts/gen-eval-report.mjs"
rc2=$?

if [ "$rc1" -ne 0 ] || [ "$rc2" -ne 0 ]; then
  echo "== run-realworld-eval FAIL (eval-all=$rc1 gen-report=$rc2) =="
  exit 1
fi
echo "== run-realworld-eval PASS =="
exit 0
