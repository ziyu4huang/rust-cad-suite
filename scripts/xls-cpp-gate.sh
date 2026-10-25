#!/usr/bin/env bash
# Out-of-tree gate for the XLS `VerilogSimulator` subclass.
#
# Compiles crates/rv-xls-sim/cpp against the REAL google/xls header (pinned
# release), then runs the harness against the real rv-xls-sim binary and the
# committed add3 expectations.
#
# Prerequisites: clang++ (C++20), protoc, abseil, cargo, git, network for the
# one-time pinned clone. Override the clone with XLS_SRC=/path/to/xls.
#
# Usage: bash scripts/xls-cpp-gate.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TAG="${XLS_TAG:-v0.0.0-10898-g8ddae8bf4}"
WORK="${XLS_CPP_WORK:-${TMPDIR:-/tmp}/rv-xls-cpp-gate}"
SRC="${XLS_SRC:-$WORK/xls-src}"
DEPS="$WORK/deps"
PB="$WORK/pb"
OUT="$WORK/rv-xls-cpp-harness"

for tool in clang++ protoc cargo git; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "missing prerequisite: $tool (macOS: brew install protobuf abseil; clang++ ships with Xcode CLT)" >&2
        exit 2
    }
done

ABSL="$(brew --prefix abseil 2>/dev/null || echo /opt/homebrew/opt/abseil)"
PROTOBUF="$(brew --prefix protobuf 2>/dev/null || echo /opt/homebrew/opt/protobuf)"
[ -d "$ABSL/include/absl" ] || {
    echo "abseil headers not found at $ABSL (brew install abseil)" >&2
    exit 2
}
[ -d "$PROTOBUF/include/google/protobuf" ] || {
    echo "protobuf headers not found at $PROTOBUF (brew install protobuf)" >&2
    exit 2
}

mkdir -p "$WORK"

if [ ! -d "$SRC/.git" ]; then
    echo "== clone google/xls @ $TAG =="
    git clone --depth 1 --branch "$TAG" --filter=blob:none --sparse \
        https://github.com/google/xls "$SRC"
fi
git -C "$SRC" sparse-checkout set \
    xls/simulation xls/codegen xls/ir xls/common xls/data_structures >/dev/null
echo "== upstream: $(git -C "$SRC" rev-parse --short HEAD) ($TAG) =="

echo "== dep: gtest_prod.h (FRIEND_TEST shim for XLS headers) =="
mkdir -p "$DEPS/gtest"
if [ ! -f "$DEPS/gtest/gtest_prod.h" ]; then
    curl -fsSL --max-time 30 \
        https://raw.githubusercontent.com/google/googletest/main/googletest/include/gtest/gtest_prod.h \
        -o "$DEPS/gtest/gtest_prod.h" \
        || printf '#define FRIEND_TEST(a, b) friend class a##_##b##_Test\n' \
            > "$DEPS/gtest/gtest_prod.h"
fi

echo "== protoc: xls/ir/*.proto =="
rm -rf "$PB"
mkdir -p "$PB"
(cd "$SRC" && protoc -I. --cpp_out="$PB" xls/ir/*.proto)

echo "== build adapter =="
cargo build -p rv-xls-sim --manifest-path "$ROOT/Cargo.toml" >/dev/null

CPP="$ROOT/crates/rv-xls-sim/cpp"
CXXFLAGS=(
    -std=c++20 -O1 -Wall -Wextra -Wno-unused-parameter
    -I"$CPP"
    -isystem "$SRC"
    -isystem "$PB"
    -isystem "$DEPS"
    -isystem "$ABSL/include"
    -isystem "$PROTOBUF/include"
)

echo "== syntax pass: real header + module initializer =="
clang++ "${CXXFLAGS[@]}" -fsyntax-only \
    "$CPP/xls/simulation/simulators/rverilator_simulator.cc"

echo "== compile harness (local InvokeSubprocess double) =="
clang++ "${CXXFLAGS[@]}" -DRV_XLS_SIM_HARNESS \
    "$CPP/xls/simulation/simulators/rverilator_simulator.cc" \
    "$CPP/harness/harness.cc" \
    "$CPP/harness/xls_subprocess_double.cc" \
    -L"$ABSL/lib" -Wl,-rpath,"$ABSL/lib" \
    -labsl_status -labsl_statusor -labsl_strings -labsl_str_format_internal \
    -o "$OUT"

echo "== run harness =="
"$OUT" "$ROOT/target/debug/rv-xls-sim" "$ROOT"
