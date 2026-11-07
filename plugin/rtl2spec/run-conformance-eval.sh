#!/usr/bin/env bash
# run-conformance-eval.sh — conformance 閉環評測閘(§19.5;D-C7;C-S3 確定性床)。
# 總閘直調,**不進 regress**(評測層:10² 次編譯+模擬+verilator 6 次建置,分鐘級;§19.5 閘落點)。
# ①守衛:iverilog/bun 在場;dist/wasm 在場且不陳舊(6 傳遞依賴目錄 find -newer,與
#   run-golden-regression.sh 同清單)——陳舊=exit 2 並提示先跑 run-golden-regression.sh
#   (**本閘不重建 pkg**,守寫邊界);設計目錄數==EXPECTED_DESIGNS 由 conformance-eval.mjs 斷言。
# ②bun scripts/conformance-eval.mjs [--record --slice <C-Sn> [design…]]
#   (selfcheck 每次必跑;Q1..Q9;SUMMARY 行;默認模式寫只落 build/conformance/)。
# exit:0=全部 ADMITTED 且快照全等;1=判據敗/DRIFT/selfcheck 敗/證據頁陳舊/TB-SHA-ALARM(入庫 tb.v 手改);2=守衛/用法。
# 語料唯讀:只讀 corpus/ 與 corpus-external/;寫只落 plugin/rtl2spec/(build/ 與 --record 時的
# fixtures/conformance/+conformance-report.md)。
set -u
PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd)"

# Defensive only: export CLT when DEVELOPER_DIR is unset AND CLT exists;
# explicit DEVELOPER_DIR wins (user sovereignty); CLT-less machines no-op.
if [ -z "${DEVELOPER_DIR:-}" ] && [ -d /Library/Developer/CommandLineTools/usr/bin ]; then
    export DEVELOPER_DIR=/Library/Developer/CommandLineTools
fi

echo "== rtl2spec run-conformance-eval${*:+ $*} =="

MISSING=()
command -v bun >/dev/null 2>&1 || MISSING+=("bun")
command -v iverilog >/dev/null 2>&1 || MISSING+=("iverilog")
command -v vvp >/dev/null 2>&1 || MISSING+=("vvp")
if [ "${#MISSING[@]}" -gt 0 ]; then
  echo "FAIL guard: 缺 ${MISSING[*]}"
  exit 2
fi
command -v verilator >/dev/null 2>&1 || echo "INFO verilator 不在場:Q7 逐設計 SKIP(不靜默,verdict.json 記 crossSim=SKIP)"

DIST_WASM="$PLUGIN_ROOT/dist/wasm/rtl_spec_wasm_bg.wasm"
[ -f "$DIST_WASM" ] || { echo "FAIL guard: dist/wasm 缺——先跑 bash plugin/rtl2spec/run-golden-regression.sh(本閘不重建 pkg)"; exit 2; }
DEP_DIRS=(
  "$REPO_ROOT/crates/rtl-spec/src"
  "$REPO_ROOT/crates/rtl-spec-wasm/src"
  "$REPO_ROOT/crates/analyzer-core/src"
  "$REPO_ROOT/crates/vl-syntax/src"
  "$REPO_ROOT/crates/ry-ir/src"
  "$REPO_ROOT/crates/rcs-diag/src"
)
for d in "${DEP_DIRS[@]}"; do
  if [ -n "$(find "$d" -name '*.rs' -newer "$DIST_WASM" -print -quit 2>/dev/null)" ]; then
    echo "FAIL guard: dist/wasm 陳舊($d 有更新的 .rs)——先跑 bash plugin/rtl2spec/run-golden-regression.sh(本閘不重建 pkg)"
    exit 2
  fi
done
echo "PASS guard: bun $(bun --version), $(iverilog -V 2>&1 | head -1), dist/wasm fresh"

cd "$REPO_ROOT"
bun "$PLUGIN_ROOT/scripts/conformance-eval.mjs" "$@"
rc=$?
if [ "$rc" -ne 0 ]; then
  echo "== run-conformance-eval FAIL (exit $rc) =="
  exit "$rc"
fi
echo "== run-conformance-eval PASS =="
exit 0
