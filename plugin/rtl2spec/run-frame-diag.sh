#!/usr/bin/env bash
# run-frame-diag.sh — 波形幀診斷腿機械段(RTL2SPEC-FINISH-PLAN §4;CONFORMANCE-PROTOCOL §8)。輔助證據,**不進任何閘**,直調。
# counterexample VCD → vl-vcd vcd2svg 統一窗 SVG → sips/qlmanage 柵格化 PNG → 雙檢定 modality → prompt 交視覺 LLM。
# 用法:bash plugin/rtl2spec/run-frame-diag.sh [design…] [--mutant <design>=<id>]
#       bash plugin/rtl2spec/run-frame-diag.sh --validate [design…]   驗 diag-vision/<design>.json(只驗工件,不驗 LLM 內容)
# 構建守衛=拒跑形:腿內不構建(寫面只落 plugin/rtl2spec/build/;run-conformance-eval.sh「本閘不重建」同款)。
# exit:0=機械段過(text-fallback 不是失敗);1=工件紅;2=守衛/用法。
set -u
PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd)"

echo "== rtl2spec run-frame-diag${*:+ $*} =="

MISSING=()
command -v bun >/dev/null 2>&1 || MISSING+=("bun")
case " $* " in
  *" --validate "*) ;;
  *)
    command -v iverilog >/dev/null 2>&1 || MISSING+=("iverilog")
    command -v vvp >/dev/null 2>&1 || MISSING+=("vvp")
    ;;
esac
if [ "${#MISSING[@]}" -gt 0 ]; then
  echo "FAIL guard: 缺 ${MISSING[*]}"
  exit 2
fi

case " $* " in
  *" --validate "*) ;;
  *)
    VCD2SVG="$REPO_ROOT/target/debug/vcd2svg"
    if [ ! -x "$VCD2SVG" ]; then
      echo "FAIL guard: target/debug/vcd2svg 缺——先跑 cargo build -p vl-vcd --bin vcd2svg(腿內不構建)"
      exit 2
    fi
    STALE="$(find "$REPO_ROOT/crates/vl-vcd/src" -name '*.rs' -newer "$VCD2SVG" -print -quit 2>/dev/null)"
    if [ -n "$STALE" ]; then
      echo "FAIL guard: vcd2svg 陳舊($STALE 較新)——先跑 cargo build -p vl-vcd --bin vcd2svg(腿內不構建)"
      exit 2
    fi
    command -v sips >/dev/null 2>&1 || command -v qlmanage >/dev/null 2>&1 || echo "INFO sips/qlmanage 皆缺:全部設計降級 text-fallback(不靜默)"
    echo "PASS guard: bun $(bun --version), $(iverilog -V 2>&1 | head -1), $("$VCD2SVG" --version)"
    ;;
esac

cd "$REPO_ROOT"
bun "$PLUGIN_ROOT/scripts/frame-diag.mjs" "$@"
rc=$?
if [ "$rc" -ne 0 ]; then
  echo "== run-frame-diag FAIL (exit $rc) =="
  exit "$rc"
fi
echo "== run-frame-diag PASS =="
exit 0
