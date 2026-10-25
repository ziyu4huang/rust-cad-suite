#!/usr/bin/env python3
"""Adopt an XLS harvest artifact into `corpus-external/xls`.

The artifact is exactly what `.github/workflows/xls-harvest.yml` uploads:
`manifest.json`, `generated/<id>.{v,ir}`, `dslx/<id>.x`, and the Apache-2.0
licence/notice. This script verifies every sha256, then copies the files into
the snapshot, applying the two documented normalizations only when the artifact
is not already normalized (idempotent adoption).

It deliberately does NOT create the e2e case: the testbench is human-written
(race-safe sampling) and the expectation must come from the Verilator oracle.
The script prints those two next steps.

Usage:
  python3 scripts/xls_adopt.py /tmp/xls-artifact [--dest corpus-external/xls] [--dry-run]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import shutil
import sys

TIMESCALE = "`timescale 1ns/1ps\n"


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def normalize_verilog(text: str) -> tuple[str, list[str]]:
    changes: list[str] = []
    if "always_ff @" in text:
        text = text.replace("always_ff @", "always @")
        changes.append("always_ff -> always")
    if not re.search(r"^\s*`timescale\b", text, re.MULTILINE):
        text = TIMESCALE + text
        changes.append("timescale injected")
    return text, changes


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=pathlib.Path)
    parser.add_argument("--dest", type=pathlib.Path,
                        default=pathlib.Path("corpus-external/xls"))
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    artifact: pathlib.Path = args.artifact
    manifest_path = artifact / "manifest.json"
    if not manifest_path.is_file():
        print(f"missing {manifest_path}", file=sys.stderr)
        return 2
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schema") != 1:
        print(f"unsupported manifest schema: {manifest.get('schema')}",
              file=sys.stderr)
        return 2

    dest: pathlib.Path = args.dest
    failures = 0
    planned = []
    # Verify everything before writing anything: a tampered artifact must not
    # leave a half-updated snapshot behind.
    for case in manifest.get("cases", []):
        ident = case.get("id", "?")
        syntax = case.get("syntax_check", {}).get("status", "skipped")
        if syntax == "error":
            print(f"{ident}: adapter syntax check failed in the artifact", file=sys.stderr)
            failures += 1
            continue
        files = {}
        for key in ("verilog", "ir", "dslx"):
            rel = case.get(key)
            if rel is None:
                continue
            path = artifact / rel
            if not path.is_file():
                print(f"{ident}: missing {path}", file=sys.stderr)
                failures += 1
                continue
            expected = case.get("sha256", {}).get(key)
            actual = sha256(path)
            if expected and actual != expected:
                print(f"{ident}: sha256 mismatch for {rel}\n"
                      f"  expected {expected}\n  actual   {actual}",
                      file=sys.stderr)
                failures += 1
                continue
            files[key] = path
        if "verilog" in files:
            planned.append((ident, files))

    if failures:
        print(f"\n{failures} file(s) failed verification; nothing adopted.",
              file=sys.stderr)
        return 1
    if not planned:
        print("no cases adopted", file=sys.stderr)
        return 1

    adopted = []
    for ident, files in planned:
        text, changes = normalize_verilog(
            files["verilog"].read_text(encoding="utf-8"))
        target_v = dest / "generated" / f"{ident}.v"
        target_ir = dest / "generated" / f"{ident}.ir"
        target_x = dest / "dslx" / f"{ident}.x"

        actions = [f"generated/{ident}.v"]
        if "ir" in files:
            actions.append(f"generated/{ident}.ir")
        if "dslx" in files:
            actions.append(f"dslx/{ident}.x")

        if args.dry_run:
            print(f"{ident}: would adopt {', '.join(actions)}"
                  + (f" ({'; '.join(changes)})" if changes else ""))
            adopted.append(ident)
            continue

        target_v.parent.mkdir(parents=True, exist_ok=True)
        target_x.parent.mkdir(parents=True, exist_ok=True)
        target_v.write_text(text, encoding="utf-8")
        if "ir" in files:
            shutil.copyfile(files["ir"], target_ir)
        if "dslx" in files:
            shutil.copyfile(files["dslx"], target_x)
        adopted.append(ident)
        print(f"{ident}: adopted {', '.join(actions)}"
              + (f" ({'; '.join(changes)})" if changes else ""))

    for name in ("XLS-LICENSE.txt", "XLS-NOTICE.txt"):
        source = artifact / name
        if source.is_file() and not args.dry_run:
            dest.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, dest / name)

    if not args.dry_run:
        dest.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(manifest_path, dest / "harvest-manifest.json")

    tag = manifest.get("xls", {}).get("tag", "?")
    verb = "would adopt" if args.dry_run else "adopted"
    print(f"\n{verb} {len(adopted)} case(s) from google/xls {tag} into {dest}")
    print("Next (human, one case at a time):")
    for ident in adopted:
        print(f"  1. add corpus/e2e/<NN>_xls_{ident}/tb.v (race-safe: "
              f"@(posedge clk); #1; <read>)")
    print("  2. record expectations from the oracle without churning the rest:")
    print("       mkdir -p /tmp/xls-record && ln -sfn \"$PWD/corpus/e2e/<case>\" /tmp/xls-record/<case>")
    print("       ./target/debug/vl-difftest --record --corpus /tmp/xls-record")
    print("       ./target/debug/vl-difftest --corpus corpus/e2e --min-pass 100")
    print("  3. the DSH panel picks the case up from the XLS manifest once its "
          "e2e expected.stdout exists (no client edit)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
