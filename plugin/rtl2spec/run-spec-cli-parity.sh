#!/usr/bin/env bash
# run-spec-cli-parity.sh — CLI↔dist wasm byte-equal parity 閘(D-S2;D-PLAN §5.1)。
# 判據:同一名(name=入參路徑逐字)同源文本下,rtl-spec 資料子命令 stdout
# raw JSON 與 bun 載 dist 同一 wasm 的對應導出 byte-equal;任一不等或 CLI
# exit 不符=exit 1。
# 守衛(雙守衛降級形):bun/pkg(target/debug/rtl-spec 缺場=exit 2 降級,
# 不構建——cargo 產物由快閘保證在場)。
# 陳舊防線(D8 同形):pkg 對 6 個傳遞依賴目錄 find -newer;陳舊→重建+同步
# dist/wasm(與 run-golden-regression 同款,非本閘新增形)。
# 負向探針:CLI 渲染面任何改動(默認值/換行/鍵序)必紅本閘(D-S2 實錄)。
# 寫面=plugin/rtl2spec/build/parity/(gitignored);語料唯讀。
# spec 腿(D-S3 起):CLI spec parse/lint vs wasm rtl_spec_spec_parse/lint(§5.1)。
set -u
PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd)"
BIN="$REPO_ROOT/target/debug/rtl-spec"
PARITY_DIR="$PLUGIN_ROOT/build/parity"

echo "== rtl2spec run-spec-cli-parity =="

# ① 守衛(缺場=exit 2 降級)
command -v bun >/dev/null 2>&1 || { echo "SKIP guard: bun not in PATH"; exit 2; }
[ -f "$REPO_ROOT/crates/rtl-spec-wasm/pkg/rtl_spec_wasm_bg.wasm" ] \
  || { echo "SKIP guard: rtl-spec-wasm pkg missing (wasm-pack build)"; exit 2; }
[ -x "$BIN" ] || { echo "SKIP guard: target/debug/rtl-spec missing (cargo build first)"; exit 2; }
export RTL2SPEC_BUN_VERSION="$(bun --version)"
echo "PASS guard: bun $RTL2SPEC_BUN_VERSION, pkg+bin in place"

# ② pkg 陳舊檢查+同步(與 run-golden-regression.sh 同形;自包含)
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

# ③ CLI 腿(name=路徑逐字;stdout 逐位元組落檔+exit code 落檔)
mkdir -p "$PARITY_DIR"
SV="crates/rtl-spec/tests/fixtures/rulebook_sample.v"
IL="crates/rtl-spec/tests/fixtures/sample.il"
SPEC="plugin/rtl2spec/fixtures/d-spec/counter8.spec.md"
CORPUS="corpus/e2e/01_arith_widths/tb.v"
run_leg() {
  local leg="$1"; shift
  (cd "$REPO_ROOT" && "$BIN" "$@" > "$PARITY_DIR/cli-$leg.json" 2> "$PARITY_DIR/cli-$leg.stderr")
  echo $? > "$PARITY_DIR/cli-$leg.exit"
}
run_leg analyze-sv      analyze "$SV"
run_leg analyze-rtlil   analyze "$IL"
run_leg extract-sv-def  extract "$SV"
run_leg extract-sv-128  extract "$SV" --max-rules 128
run_leg extract-rtlil   extract "$IL"
run_leg extract-dspec   extract "$SPEC"
run_leg extract-corpus  extract "$CORPUS"
run_leg gaps-sv         gaps "$SV"
run_leg gaps-rtlil      gaps "$IL"
run_leg tb-sv           tb "$SV" --module rulebook_sample
run_leg tb-rtlil        tb "$IL" --module m
# spec 腿(D-S3):CLI spec parse/lint vs wasm rtl_spec_spec_parse/lint(§5.1)。
run_leg spec-parse-counter8  spec parse plugin/rtl2spec/fixtures/d-spec/counter8.spec.md
run_leg spec-parse-accpipe   spec parse plugin/rtl2spec/fixtures/d-spec/acc_pipe.spec.md
run_leg spec-parse-i2c       spec parse plugin/rtl2spec/fixtures/d-spec/i2c_master.spec.md
run_leg spec-lint-counter8   spec lint plugin/rtl2spec/fixtures/d-spec/counter8.spec.md
run_leg spec-lint-accpipe    spec lint plugin/rtl2spec/fixtures/d-spec/acc_pipe.spec.md
run_leg spec-lint-i2c        spec lint plugin/rtl2spec/fixtures/d-spec/i2c_master.spec.md

# ④ 比對(單 bun 進程載 dist 同一 wasm;byte-equal+exit 對拍)
cd "$REPO_ROOT"
bun "$PLUGIN_ROOT/scripts/spec-cli-parity.mjs" "$PARITY_DIR"
rc=$?
if [ "$rc" -ne 0 ]; then
  echo "== run-spec-cli-parity FAIL (exit $rc) =="
  exit "$rc"
fi
echo "== run-spec-cli-parity PASS =="
