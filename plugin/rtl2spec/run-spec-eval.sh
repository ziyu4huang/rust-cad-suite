#!/usr/bin/env bash
# run-spec-eval.sh — spec 前端評測床(D-S4;D-PLAN §5.2;直調不進 regress,
# D-S6 收編時配守衛=iverilog 在場,缺場 exit 2 SKIP)。
# 判據:legs.json 全腿——接線斷言(DUT 檔含 `module <spec.module>`)+iverilog
# 編譯+SCENE 塊 diff 對 expected 逐字;mutprobe 腿全紅(判別力數字對
# legs.json 錄入值漂移即紅);證據頁默認落 build/spec-eval/(零牆鐘;
# --record --slice <D-Sn> 才寫已提交頁,保鮮形 D-S6 收編)。
# 守衛:iverilog 缺場 exit 2;腿產物缺 exit 3。寫面=build/spec-eval/+
# --record 時證據頁一檔(10.12 寫面聲明同款);語料唯讀。
# 用法: bash run-spec-eval.sh [--record --slice <D-Sn>]
set -u
PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd)"

echo "== rtl2spec run-spec-eval${*:+ $*} =="

# ① 守衛
if ! command -v iverilog >/dev/null 2>&1; then
  echo "SKIP guard: iverilog not in PATH"
  exit 2
fi
echo "PASS guard: iverilog $(iverilog -V 2>/dev/null | head -1)"

# ② 執行器(守衛細項/寫面在 mjs 內;exit 語義:0=全過,1=域內失敗,2=用法/環境,3=產物缺)
cd "$REPO_ROOT"
bun "$PLUGIN_ROOT/scripts/spec-eval.mjs" "$@"
rc=$?
if [ "$rc" -ne 0 ]; then
  echo "== run-spec-eval FAIL (exit $rc) =="
  exit "$rc"
fi
echo "== run-spec-eval PASS =="
