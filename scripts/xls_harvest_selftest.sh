#!/usr/bin/env bash
# Hermetic self-test for the XLS harvest -> adopt chain (no network, no CI).
#
# Builds an artifact from the committed snapshot, writes its manifest, adopts it
# into a scratch tree, and checks: byte-identical copy, idempotent re-adoption,
# normalization of a raw sample, and refusal of a tampered artifact.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/xls-harvest-selftest.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

ADAPTER="$ROOT/target/debug/rv-xls-sim"
[ -x "$ADAPTER" ] || {
    echo "missing $ADAPTER (run cargo build -p rv-xls-sim first)" >&2
    exit 2
}
command -v python3 >/dev/null || {
    echo "missing python3" >&2
    exit 2
}

fail=0
note() { echo "  $*"; }

mkdir -p "$WORK/artifact/generated" "$WORK/artifact/dslx"
cp "$ROOT"/corpus-external/xls/generated/*.v \
   "$ROOT"/corpus-external/xls/generated/*.ir "$WORK/artifact/generated/"
cp "$ROOT"/corpus-external/xls/dslx/*.x "$WORK/artifact/dslx/"
cp "$ROOT"/corpus-external/xls/XLS-LICENSE.txt \
   "$ROOT"/corpus-external/xls/XLS-NOTICE.txt "$WORK/artifact/"

if python3 "$ROOT/scripts/xls_harvest_manifest.py" \
        --dir "$WORK/artifact" --tag selftest --adapter "$ADAPTER" >/dev/null; then
    note "manifest: sha256 + adapter syntax gate"
else
    note "FAIL manifest generation"; fail=1
fi

if python3 "$ROOT/scripts/xls_adopt.py" "$WORK/artifact" \
        --dest "$WORK/dest" >/dev/null; then
    note "adopt: snapshot copied"
else
    note "FAIL adopt"; fail=1
fi

for f in generated/add3.v generated/add3.ir generated/acc_pipe.v \
         generated/acc_pipe.ir dslx/add3.x dslx/acc_pipe.x; do
    if ! cmp -s "$WORK/dest/$f" "$ROOT/corpus-external/xls/$f"; then
        note "FAIL byte mismatch: $f"; fail=1
    fi
done
note "adopt: byte-identical to the committed snapshot"

python3 "$ROOT/scripts/xls_adopt.py" "$WORK/artifact" \
    --dest "$WORK/dest" >/dev/null
if cmp -s "$WORK/dest/generated/add3.v" \
          "$ROOT/corpus-external/xls/generated/add3.v"; then
    note "adopt: idempotent re-run"
else
    note "FAIL idempotent re-run changed bytes"; fail=1
fi

printf 'module m(input clk);\n  always_ff @(posedge clk) begin end\nendmodule\n' \
    > "$WORK/raw.v"
mkdir -p "$WORK/raw-artifact/generated"
cp "$WORK/raw.v" "$WORK/raw-artifact/generated/m.v"
printf 'package m\n' > "$WORK/raw-artifact/generated/m.ir"
# No --adapter: this artifact models a harvest from a pipeline that did not run
# the syntax gate, so adopt must normalize it and the skipped check is allowed.
python3 "$ROOT/scripts/xls_harvest_manifest.py" --dir "$WORK/raw-artifact" \
    --tag selftest --allow-missing-dslx >/dev/null
if python3 "$ROOT/scripts/xls_adopt.py" "$WORK/raw-artifact" \
        --dest "$WORK/raw-dest" >/dev/null 2>&1; then
    if head -1 "$WORK/raw-dest/generated/m.v" | grep -q '^`timescale' \
        && ! grep -q 'always_ff' "$WORK/raw-dest/generated/m.v"; then
        note "adopt: normalization applied to an unnormalized artifact"
    else
        note "FAIL normalization"; fail=1
    fi
else
    note "FAIL raw adopt (syntax-skip artifact should still adopt)"
    fail=1
fi

cp -R "$WORK/artifact" "$WORK/tampered"
printf '// tamper\n' >> "$WORK/tampered/generated/add3.v"
if python3 "$ROOT/scripts/xls_adopt.py" "$WORK/tampered" \
        --dest "$WORK/tampered-dest" >/dev/null 2>&1; then
    note "FAIL tampered artifact was adopted"; fail=1
elif [ -e "$WORK/tampered-dest" ]; then
    note "FAIL tampered adoption left a partial destination"; fail=1
else
    note "adopt: tampered artifact refused before writing"
fi

if [ "$fail" -ne 0 ]; then
    echo "xls harvest self-test: FAIL" >&2
    exit 1
fi
echo "xls harvest self-test: PASS"
