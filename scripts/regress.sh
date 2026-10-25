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
    if [ "$ln" -lt 83 ]; then
        echo "GATE FAIL: lint parity $ln/87 < committed baseline 83"
        fail=1
    else
        echo "lint parity: $ln/87 (baseline 83)"
    fi
fi

# riverilog: CLI contract goldens (byte-exact, zero oracle dependency)
# + C4 runner gates: exact-count baselines on both arms (e2e via
# riverilog->rivvp + normalization; iv via case.json enforced/pending),
# UNLEDGERED/DRIFT red-light greps == 0. Flipping a case.json
# enforced<->pending or editing a riv-pending.txt ledger moves the exact
# counts and must land in the same commit as the capability change
# (AGENTS 10.5 riv ledger row).
if [ -f crates/riv-cli/Cargo.toml ]; then
    ct=$(cargo test -p riv-cli 2>&1 | grep -E "^test result" | tail -1)
    echo "$ct" | grep -q "test result: ok" || { echo "GATE FAIL: riv-cli goldens: $ct"; fail=1; }
    e2e_out=$(cargo run -q -p riv-difftest -- --e2e --corpus corpus/e2e 2>&1)
    iv_out=$(cargo run -q -p riv-difftest -- --corpus corpus/iv-e2e 2>&1)
    e2e_sum=$(echo "$e2e_out" | grep -E "^SUMMARY e2e=" || true)
    iv_sum=$(echo "$iv_out" | grep -E "^SUMMARY .*iv=" || true)
    [ -n "$e2e_sum" ] || { echo "GATE FAIL: riv e2e SUMMARY missing"; fail=1; }
    [ -n "$iv_sum" ] || { echo "GATE FAIL: riv iv SUMMARY missing"; fail=1; }
    echo "$e2e_sum" | grep -q "SUMMARY e2e=58/59 iv=0/0" || { echo "GATE FAIL: riv e2e baseline (want exact 58/59): $e2e_sum"; fail=1; }
    echo "$iv_sum" | grep -q "SUMMARY e2e=0/0 iv=21/22" || { echo "GATE FAIL: riv iv baseline (want exact 21/22): $iv_sum"; fail=1; }
    riv_all="$e2e_out
$iv_out"
    riv_unledgered=$(echo "$riv_all" | grep -c "UNLEDGERED" || true)
    [ "$riv_unledgered" -eq 0 ] || { echo "GATE FAIL: riv UNLEDGERED x$riv_unledgered (byte-diff without a riv-pending.txt ledger)"; echo "$riv_all" | grep "UNLEDGERED"; fail=1; }
    riv_drift=$(echo "$riv_all" | grep -c "DRIFT" || true)
    [ "$riv_drift" -eq 0 ] || { echo "GATE FAIL: riv DRIFT x$riv_drift (ledger stale — flip case.json/ledger in the same commit)"; echo "$riv_all" | grep "DRIFT"; fail=1; }
    # Three-way triage (R4 arc step 0, RIVERILOG-PLAN §8.1): the iv leg is
    # committed (expected.iv.*), so this needs no local oracle. bug>0 = riv
    # disagrees with BOTH agreeing oracles — blocks any baseline flip.
    # tri-div-* lines feed the rivD register (they do not fail the gate).
    tri_out=$(cargo run -q -p riv-difftest -- --tri --corpus corpus/e2e 2>&1)
    tri_sum=$(echo "$tri_out" | grep -E "^TRI " || true)
    [ -n "$tri_sum" ] || { echo "GATE FAIL: riv tri TRI summary missing"; fail=1; }
    tri_bug=$(echo "$tri_sum" | sed -E 's/.*bug=([0-9]+).*/\1/')
    [ "$tri_bug" = "0" ] || { echo "GATE FAIL: riv tri bug=x$tri_bug (riv differs from both agreeing oracles)"; echo "$tri_out" | grep "tri-bug"; fail=1; }
    echo "$tri_sum"
    echo "$e2e_sum"
    echo "$iv_sum"
    # riverilog VCD cross-compare (RIVERILOG-PLAN §8.1/rivD9 + R5 22):
    # every case with an iverilog-recorded oracle-ref.iv.vcd (51 base,
    # 22 dump controls) — sample-point compare through the single VCD core.
    repo_root=$(pwd)
    for vcase in corpus/e2e/51_vcd_counter corpus/iv-e2e/22_dump_control; do
        [ -f "$vcase/oracle-ref.iv.vcd" ] || continue
        vcd_tmp=$(mktemp -d)
        gflag=""
        case "$vcase" in
            corpus/iv-e2e/*) gflag="-g2005" ;;
        esac
        cp "$vcase"/tb.v "$vcd_tmp/"
        (cd "$vcd_tmp" && "$repo_root/target/debug/riverilog" $gflag -o out.riv tb.v >/dev/null 2>&1 && "$repo_root/target/debug/rivvp" out.riv >/dev/null 2>&1)
        if [ -f "$vcd_tmp/dump.vcd" ]; then
            vcd_rep=$(cargo run -q -p vl-difftest -- --vcd-xcheck "$vcd_tmp/dump.vcd" "$vcase/oracle-ref.iv.vcd" 2>&1)
            if [ $? -ne 0 ]; then
                echo "GATE FAIL: riv VCD crosscheck ($vcase vs iverilog ref)"
                echo "$vcd_rep"
                fail=1
            else
                echo "$vcd_rep" | tail -1
            fi
        else
            echo "GATE FAIL: riv VCD crosscheck — $vcase produced no dump.vcd"
            fail=1
        fi
        rm -rf "$vcd_tmp"
    done
fi

# WASM differential gate: the wasm engine must reproduce the same oracle
# goldens the native gate checks. Rebuilds pkg when core sources are newer.
if command -v wasm-pack >/dev/null && command -v bun >/dev/null; then
    if [ ! -f crates/vl-wasm/pkg/vl_wasm_bg.wasm ] \
       || [ -n "$(find crates/vl-sim/src crates/vl-syntax/src crates/rcs-diag/src crates/vl-wasm/src crates/vl-vcd/src crates/vl-cli/src crates/vl-spec/src crates/spec-i2c/src crates/spec-i2c/rtl \( -name '*.rs' -o -name '*.v' \) -newer crates/vl-wasm/pkg/vl_wasm_bg.wasm 2>/dev/null | head -1)" ]; then
        echo "== [extra] rebuilding wasm pkg (sources newer than pkg) =="
        wasm-pack build crates/vl-wasm --target web --out-dir pkg >/dev/null 2>&1 \
            || { echo "GATE FAIL: wasm-pack build"; fail=1; }
    fi
    if [ -f crates/vl-wasm/pkg/vl_wasm_bg.wasm ]; then
        for t in run features vcd_parse vcd_svg spec; do
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
