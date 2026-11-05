#!/usr/bin/env bash
# run-golden-regression.sh — golden 回歸閘(總閘成員;§16.1.3 五段形)。
# ①守衛=bun/wasm-pack 在場+37 檔釘選清單在場(缺→exit 2 逐一列出)
# ②pkg 陳舊檢查+同步 dist/wasm(6 個傳遞依賴目錄 find -newer,D8 同形)
# ③逐檔跑 extract_rules+rtl_scan_gaps(wasm;單 bun 進程內循環,
#   scripts/golden-check.mjs),產物寫 build/golden-current/(gitignored)
# ④逐份 byte-diff vs golden/(golden-check.mjs 內;diff 清單逐行打印
#   檔名+首個差異鍵路徑)
# ⑤任何 diff→exit 1;全等→exit 0+PROVENANCE sha256 表核對(golden-check 內)
# re-record 顯式機制(§16.1.4):--record 前置自檢(進程內每檔每模式 3 跑
#   hash 全等,兩階段防部分錄製;不過=exit 3 拒錄)→寫 golden/+PROVENANCE
#   機械重寫。錄後必讀 git diff plugin/rtl2spec/golden 逐案對帳——
#   re-record=語義事件。
# 用法: bash run-golden-regression.sh [--record]
# 語料唯讀:只讀 corpus/ 與 corpus-external/,寫只落 plugin/rtl2spec/。
set -u
PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd)"
RECORD_FLAG=""
if [ "${1:-}" = "--record" ]; then RECORD_FLAG="--record"; fi

echo "== rtl2spec run-golden-regression${RECORD_FLAG:+ $RECORD_FLAG} =="

# ① 守衛
command -v bun >/dev/null 2>&1 || { echo "FAIL guard: bun not in PATH"; exit 2; }
command -v wasm-pack >/dev/null 2>&1 || { echo "FAIL guard: wasm-pack not in PATH"; exit 2; }
export RTL2SPEC_BUN_VERSION="$(bun --version)"
echo "PASS guard: bun $RTL2SPEC_BUN_VERSION, wasm-pack $(wasm-pack --version)"

PINNED="$PLUGIN_ROOT/pinned-files.list"
[ -f "$PINNED" ] || { echo "FAIL guard: 釘選清單缺 $PINNED"; exit 2; }
MISSING=()
while IFS= read -r f; do
  f="${f#\"}"; f="${f%\"}"
  [ -z "$f" ] && continue
  case "$f" in \#*) continue ;; esac
  [ -f "$REPO_ROOT/$f" ] || MISSING+=("$f")
done < "$PINNED"
if [ "${#MISSING[@]}" -gt 0 ]; then
  echo "FAIL guard: 釘選集缺 ${#MISSING[@]} 檔:"
  for f in "${MISSING[@]}"; do echo "  - $f"; done
  exit 2
fi
echo "PASS guard: 釘選集 $(grep -c -v -e '^\s*#' -e '^\s*$' "$PINNED") 檔在場"

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

# ③④⑤ 產出+比對+PROVENANCE 核對(單 bun 進程;語料唯讀,寫只落 plugin/rtl2spec/)
cd "$REPO_ROOT"
bun "$PLUGIN_ROOT/scripts/golden-check.mjs" $RECORD_FLAG
rc=$?

if [ "$rc" -ne 0 ]; then
  echo "== run-golden-regression FAIL (exit $rc) =="
  exit "$rc"
fi
if [ -n "$RECORD_FLAG" ]; then
  echo "== run-golden-regression --record PASS — 錄後必讀: git diff plugin/rtl2spec/golden 逐案對帳 =="
else
  echo "== run-golden-regression PASS =="
fi
exit 0
