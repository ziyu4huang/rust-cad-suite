#!/usr/bin/env bash
# run-reimpl-eval.sh — 復原對拍評測(S5;§5.2 S5(c)(d);總閘成員外=直調不進 regress)。
# ①守衛=iverilog 在場+版本記錄(缺場 exit 2;守衛形套 run-golden-regression.sh:24-28 先例)
# ②bun scripts/reimpl-eval.mjs 對拍(23 場景逐腿;兩組分列禁混報;寫只落 build/reimpl-eval/)
# ③產物缺=exit 3(盲寫腿未跑);判據敗=exit 1
# 盲寫=輔助證據永不進確定性閘;機械面=iverilog 編譯成敗+TB 逐場景 diff(REIMPL-PROTOCOL.md)。
set -u
PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd)"

echo "== rtl2spec run-reimpl-eval =="

# ① iverilog 守衛
command -v iverilog >/dev/null 2>&1 || {
  echo "FAIL guard: iverilog not in PATH(E1 對拍依賴 Icarus Verilog;缺場 exit 2)"
  exit 2
}
IV_VER="$(iverilog -V 2>/dev/null | head -1)"
export RTL2SPEC_IVERILOG_VERSION="$IV_VER"
echo "PASS guard: iverilog ($IV_VER)"
command -v bun >/dev/null 2>&1 || { echo "FAIL guard: bun not in PATH"; exit 2; }
echo "PASS guard: bun $(bun --version)"

# ② 對拍(機械面=編譯成敗+TB 逐場景 diff)
cd "$REPO_ROOT"
bun "$PLUGIN_ROOT/scripts/reimpl-eval.mjs" "$@"
rc=$?
if [ "$rc" -ne 0 ]; then
  case "$rc" in
    3) echo "== run-reimpl-eval INCOMPLETE (exit 3=盲寫腿產物缺) ==" ;;
    *) echo "== run-reimpl-eval FAIL (exit $rc) ==" ;;
  esac
  exit "$rc"
fi
echo "== run-reimpl-eval PASS(23 場景逐腿;數字按組/切片分段見 verdict.json) =="
exit 0
