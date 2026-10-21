#!/usr/bin/env bash
# Produce an ORACLE reference VCD for an e2e case. Verilator natively honors
# $dumpfile/$dumpvars when built with --trace (its --binary flow then writes
# the VCD the TB asked for). Usage: bash scripts/oracle_vcd.sh <case_dir> <out.vcd>
set -eu
case_dir="$(cd "$1" && pwd)"
out="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
work="$(mktemp -d)"
( cd "$work" && /opt/homebrew/bin/verilator --binary --timing --trace -Wno-fatal \
    --Mdir obj -o sim "$case_dir"/*.v >build.log 2>&1 \
  && ./obj/sim >run.log 2>&1 ) || { echo "oracle build/run failed:"; tail -5 "$work/build.log" "$work/run.log"; exit 1; }
[ -f "$work/dump.vcd" ] || { echo "no dump.vcd produced (TB must call \$dumpfile/\$dumpvars)"; exit 1; }
cp "$work/dump.vcd" "$out"
echo "oracle VCD → $out ($(grep -c '^#' "$out") time markers)"
rm -rf "$work"
