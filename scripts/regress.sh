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
