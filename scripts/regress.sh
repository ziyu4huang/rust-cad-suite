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
# Exact-count gate: 2/57 slips silently under a 80% threshold (1023 func-regression lesson).
if ! out=$(./target/debug/vl-difftest --min-pass 100 2>&1); then
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
    if [ "$ln" -lt 77 ]; then
        echo "GATE FAIL: lint parity $ln/87 < committed baseline 77"
        fail=1
    else
        echo "lint parity: $ln/87 (baseline 77)"
    fi
fi

# WASM differential gate: the wasm engine must reproduce the same oracle
# goldens the native gate checks. Rebuilds pkg when core sources are newer.
if command -v wasm-pack >/dev/null && command -v bun >/dev/null; then
    if [ ! -f crates/vl-wasm/pkg/vl_wasm_bg.wasm ] \
       || [ -n "$(find crates/vl-sim/src crates/vl-syntax/src crates/rcs-diag/src crates/vl-wasm/src crates/vl-vcd/src crates/vl-cli/src -name '*.rs' -newer crates/vl-wasm/pkg/vl_wasm_bg.wasm 2>/dev/null | head -1)" ]; then
        echo "== [extra] rebuilding wasm pkg (sources newer than pkg) =="
        wasm-pack build crates/vl-wasm --target web --out-dir pkg >/dev/null 2>&1 \
            || { echo "GATE FAIL: wasm-pack build"; fail=1; }
    fi
    if [ -f crates/vl-wasm/pkg/vl_wasm_bg.wasm ]; then
        for t in run features vcd_parse vcd_svg; do
            bun crates/vl-wasm/wasm-test/$t.mjs >/tmp/wasm-gate-$t.out 2>&1 \
                || { echo "GATE FAIL: wasm $t"; cat /tmp/wasm-gate-$t.out; fail=1; }
            tail -1 /tmp/wasm-gate-$t.out
        done
    fi
fi

# Demo serve smoke: every asset 200 + no-store (repo rule for preview
# servers); browser-rendered visual checks stay a documented manual step.
if command -v bun >/dev/null && [ -f crates/vl-wasm/wasm-demo/index.html ]; then
    bun crates/vl-wasm/wasm-demo/serve.ts >/dev/null 2>&1 &
    DEMO_PID=$!
    sleep 0.6
    smoke=0
    for a in index.html pkg/vl_wasm.js pkg/vl_wasm_bg.wasm corpus/e2e/57_xls_add3/add3.v; do
        code=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:8031/$a")
        [ "$code" != "200" ] && smoke=1
    done
    cc=$(curl -sI http://127.0.0.1:8031/ | grep -ci "cache-control: no-store")
    [ "$cc" -eq 0 ] && smoke=1
    kill $DEMO_PID 2>/dev/null
    if [ "$smoke" -ne 0 ]; then echo "GATE FAIL: demo serve smoke"; fail=1; else echo "demo serve smoke: 200+no-store"; fi
fi

# FFI gate: Bun→native dylib must reproduce the same goldens.
if command -v bun >/dev/null && [ -f target/release/libvl_ffi.dylib ]; then
    bun crates/vl-ffi/bun-example/run.mjs >/tmp/ffi-gate.out 2>&1 \
        || { echo "GATE FAIL: ffi"; cat /tmp/ffi-gate.out; fail=1; }
    echo "ffi golden gate: $(grep -m1 PASS /tmp/ffi-gate.out || echo MISSING)"
    bun crates/vl-ffi/bun-example/spawn.mjs >/tmp/ffi-spawn.out 2>&1 \
        || { echo "GATE FAIL: ffi spawn"; cat /tmp/ffi-spawn.out; fail=1; }
    echo "ffi spawn gate: $(grep -m1 PASS /tmp/ffi-spawn.out || echo MISSING)"
    bun crates/vl-ffi/bun-example/vcd_read.mjs >/tmp/ffi-vcd.out 2>&1 \
        || { echo "GATE FAIL: ffi vcd-read"; cat /tmp/ffi-vcd.out; fail=1; }
    echo "ffi vcd-read gate: $(grep -m1 PASS /tmp/ffi-vcd.out || echo MISSING)"
    bun crates/vl-ffi/bun-example/vcd_svg.mjs >/tmp/ffi-svg.out 2>&1 \
        || { echo "GATE FAIL: ffi vcd-svg"; cat /tmp/ffi-svg.out; fail=1; }
    echo "ffi vcd-svg gate: $(grep -m1 PASS /tmp/ffi-svg.out || echo MISSING)"
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
