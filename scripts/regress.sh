#!/usr/bin/env bash
# rust-cad-suite delivery gate (3-tiers+1, tier-1 mechanical).
# Runs the full regression chain; non-zero exit = do not merge/commit.
# Usage: bash scripts/regress.sh [--with-record]
set -u
cd "$(dirname "$0")/.."
fail=0

echo "== [1/4] build (clean-ish) =="
cargo build 2>&1 | grep -E "^(error|warning)" && { echo "GATE FAIL: build warnings/errors"; fail=1; }

echo "== [2/4] unit + integration tests =="
cargo test 2>&1 | grep -qE "FAILED" && { echo "GATE FAIL: cargo test FAILED"; fail=1; }
cargo test 2>&1 | grep -cE "test result: ok" >/dev/null || { echo "GATE FAIL: no green suites"; fail=1; }

echo "== [3/4] E2E differential suite (vs committed oracle expectations) =="
if ! out=$(./target/debug/vl-difftest 2>&1); then
    echo "$out" | grep -E "^FAIL|^=="
    echo "GATE FAIL: e2e below min-pass"
    fail=1
fi
echo "$out" | tail -1

echo "== [4/4] html report regeneration (must be current) =="
./target/debug/vl-difftest --html docs/e2e-report/index.html >/dev/null 2>&1 || { echo "GATE FAIL: report gen"; fail=1; }
git diff --exit-code --stat docs/e2e-report/index.html >/dev/null 2>&1 \
    && echo "report current" \
    || echo "note: report changed vs index (regenerate+commit if intentional)"

# Lint-parity differential (requires local oracle; gate = no regression below
# the committed baseline; baseline bump is a deliberate semantic change).
if [ -x /opt/homebrew/bin/verilator ]; then
    lout=$(bash scripts/lint_diff.sh corpus-external/yosys-simple corpus-external/yosys-simple/LINT-DIFF.tsv 2>/dev/null | head -1)
    ln=$(printf '%s' "$lout" | sed -n 's/.*: \([0-9]*\)\/[0-9]* match.*/\1/p')
    [ -z "$ln" ] && ln=0
    if [ "$ln" -lt 66 ]; then
        echo "GATE FAIL: lint parity $ln/87 < committed baseline 66"
        fail=1
    else
        echo "lint parity: $ln/87 (baseline 66)"
    fi
fi

# WASM differential gate: the wasm engine must reproduce the same oracle
# goldens the native gate checks. Rebuilds pkg when core sources are newer.
if command -v wasm-pack >/dev/null && command -v bun >/dev/null; then
    if [ ! -f crates/vl-wasm/pkg/vl_wasm_bg.wasm ] \
       || [ -n "$(find crates/vl-sim/src crates/vl-syntax/src crates/rcs-diag/src crates/vl-wasm/src -name '*.rs' -newer crates/vl-wasm/pkg/vl_wasm_bg.wasm 2>/dev/null | head -1)" ]; then
        echo "== [extra] rebuilding wasm pkg (sources newer than pkg) =="
        wasm-pack build crates/vl-wasm --target web --out-dir pkg >/dev/null 2>&1 \
            || { echo "GATE FAIL: wasm-pack build"; fail=1; }
    fi
    if [ -f crates/vl-wasm/pkg/vl_wasm_bg.wasm ]; then
        bun crates/vl-wasm/wasm-test/run.mjs >/tmp/wasm-gate.out 2>&1 \
            || { echo "GATE FAIL: wasm differential"; cat /tmp/wasm-gate.out; fail=1; }
        tail -1 /tmp/wasm-gate.out
    fi
fi

if [ "${1:-}" = "--with-record" ]; then
    echo "== extra: re-record from oracle (requires verilator; review diff!) =="
    ./target/debug/vl-difftest --record 2>&1 | tail -1
    git diff --stat corpus/e2e | tail -3
fi

if [ "$fail" -ne 0 ]; then
    echo "== REGRESSION GATE: FAIL =="
    exit 1
fi
echo "== REGRESSION GATE: PASS =="
