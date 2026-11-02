#!/usr/bin/env bash
# 端到端冒煙入口(ANALYZER-SERVICE §6.4;總閘第四條):
# 構建(如需)→ 起 server(臨時 UDS)→ bun test(編解碼向量閘,D7/R1-3)→
# bun smoke.ts 真回環 → 斷言 → 清理 → 退出碼表意(0=全過,1=行為斷言失敗,
# 2=前置環境缺)。
set -euo pipefail
cd "$(dirname "$0")"
REPO="$(cd ../.. && pwd)"
command -v bun >/dev/null 2>&1 || { echo "smoke: bun not found" >&2; exit 2; }
command -v cargo >/dev/null 2>&1 || { echo "smoke: cargo not found" >&2; exit 2; }
cargo build -q --manifest-path "$REPO/Cargo.toml" -p analyzer-server || exit 2
BIN="$REPO/target/debug/analyzerd"
TMP="$(mktemp -d)"; SOCK="$TMP/a.sock"
[ "${#SOCK}" -lt 100 ] || { echo "smoke: UDS path too long: $SOCK" >&2; rm -rf "$TMP"; exit 2; }
"$BIN" --socket "$SOCK" & SRV=$!
cleanup() { kill "$SRV" 2>/dev/null || true; wait "$SRV" 2>/dev/null || true; rm -rf "$TMP"; }
trap cleanup EXIT
for _ in $(seq 1 50); do [ -S "$SOCK" ] && break; sleep 0.1; done
[ -S "$SOCK" ] || { echo "smoke: server socket never appeared" >&2; exit 1; }
bun test            # 編解碼向量閘(test/codec.test.ts;D7 的 TS 半進固定閘,
                    # R1-3:向量斷言不進閘=改壞不翻紅,exit≠0 由 set -e 落 1)
bun smoke.ts "$SOCK"
