#!/usr/bin/env bash
# run-rtl-validation.sh — 真實 RTL 驗證閘(總閘成員;§8.2)。
# ①守衛=bun/wasm-pack 在場+驗證集 13 檔在場(缺→exit 2,檔名逐一列出)
# ②構建/同步同 run-mcp-e2e.sh(自包含;陳舊檢查含全部傳遞依賴目錄)
# ③逐檔 bun scripts/check-rulebook.mjs <file>(裝載 dist 同一 wasm,D8)
# ④bun scripts/gen-report.mjs 匯總全集判據+生成 validation-report.md
# ⑤任一檔任一判據 FAIL→exit 1
# 語料唯讀:只讀 corpus/ 與 corpus-external/,寫只落 plugin/rtl2spec/。
set -u
PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd)"

echo "== rtl2spec run-rtl-validation =="

# ① 守衛
command -v bun >/dev/null 2>&1 || { echo "FAIL guard: bun not in PATH"; exit 2; }
command -v wasm-pack >/dev/null 2>&1 || { echo "FAIL guard: wasm-pack not in PATH"; exit 2; }
export RTL2SPEC_BUN_VERSION="$(bun --version)"
echo "PASS guard: bun $RTL2SPEC_BUN_VERSION, wasm-pack $(wasm-pack --version)"

FILES=(
  "corpus/e2e/59_i2c_master/i2c_master.v"
  "crates/spec-i2c/rtl/i2c_master.v"
  "corpus/e2e/58_xls_acc_pipe/acc_pipe.v"
  "corpus/e2e/57_xls_add3/add3.v"
  "corpus-external/yosys-simple/subbytes.v"
  "corpus-external/yosys-simple/fiedler-cooley.v"
  "corpus-external/yosys-simple/sincos.v"
  "corpus-external/yosys-simple/multiplier.v"
  "corpus-external/yosys-simple/memory.v"
  "corpus-external/yosys-simple/carryadd.v"
  "corpus-external/yosys-simple/generate.v"
  "corpus-external/yosys-simple/mem2reg.v"
  "corpus-external/yosys-simple/dff_different_styles.v"
)
MISSING=()
for f in "${FILES[@]}"; do
  [ -f "$REPO_ROOT/$f" ] || MISSING+=("$f")
done
if [ "${#MISSING[@]}" -gt 0 ]; then
  echo "FAIL guard: 驗證集缺 ${#MISSING[@]} 檔:"
  for f in "${MISSING[@]}"; do echo "  - $f"; done
  exit 2
fi
echo "PASS guard: 驗證集 13 檔在場"

# ② pkg 陳舊檢查 + 同步(與 run-mcp-e2e.sh 同形;自包含)
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

# 清空上一輪 verdicts(殘留會讓 V6 覆蓋面誤綠)
rm -rf "$PLUGIN_ROOT/build/verdicts"
mkdir -p "$PLUGIN_ROOT/build/verdicts"

# ③ 逐檔機械判據(語料唯讀;寫只落 plugin/rtl2spec/build/)
FAILED_FILES=()
for f in "${FILES[@]}"; do
  echo "-- check $f"
  (cd "$REPO_ROOT" && bun "$PLUGIN_ROOT/scripts/check-rulebook.mjs" "$f") > /dev/null 2>&1
  rc=$?
  if [ "$rc" -ne 0 ]; then
    FAILED_FILES+=("$f")
    echo "   FAIL (exit $rc)"
  fi
done
if [ "${#FAILED_FILES[@]}" -gt 0 ]; then
  echo "FAIL ${#FAILED_FILES[@]} 檔判據不過:${FAILED_FILES[*]}"
fi

# ④ 全集判據 + 報告生成
cd "$REPO_ROOT"
bun "$PLUGIN_ROOT/scripts/gen-report.mjs"
rc=$?

# ⑤ 退出碼表意
if [ "$rc" -ne 0 ] || [ "${#FAILED_FILES[@]}" -gt 0 ]; then
  echo "== run-rtl-validation FAIL =="
  exit 1
fi
echo "== run-rtl-validation PASS =="
exit 0
