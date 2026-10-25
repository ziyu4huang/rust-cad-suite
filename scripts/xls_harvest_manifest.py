#!/usr/bin/env python3
"""Write the adoptable manifest for an XLS harvest artifact.

The xls-harvest GitHub workflow drops `generated/` (Verilog + IR), `dslx/`
(our DSLX samples), the Apache-2.0 licence/notice, and this script produces
`manifest.json`: per-case provenance (tag, sha256) plus an adapter syntax check
(the `rv-xls-sim` gate). `scripts/xls_adopt.py` consumes exactly this file.

Usage:
  python3 scripts/xls_harvest_manifest.py --dir artifact --tag v0.0.0-... \
      [--adapter target/debug/rv-xls-sim]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import subprocess
import sys

PIPELINE = (
    "xls-harvest v1: ir_converter_main -> codegen_main; "
    "always_ff -> always; `timescale 1ns/1ps injected"
)


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dir", required=True, type=pathlib.Path,
                        help="artifact root containing generated/ and dslx/")
    parser.add_argument("--tag", required=True,
                        help="google/xls release tag the Verilog came from")
    parser.add_argument("--adapter", type=pathlib.Path, default=None,
                        help="rv-xls-sim binary; runs --syntax-check per case")
    parser.add_argument("--allow-missing-dslx", action="store_true",
                        help="do not fail when a DSLX source is absent")
    args = parser.parse_args()

    root: pathlib.Path = args.dir
    generated = root / "generated"
    if not generated.is_dir():
        print(f"missing {generated}", file=sys.stderr)
        return 2
    adapter = args.adapter
    if adapter is not None and not adapter.exists():
        print(f"adapter not found: {adapter}", file=sys.stderr)
        return 2

    cases = []
    ok = True
    for verilog in sorted(generated.glob("*.v")):
        ident = verilog.stem
        entry: dict = {
            "id": ident,
            "verilog": f"generated/{verilog.name}",
            "sha256": {"verilog": sha256(verilog)},
        }
        ir = generated / f"{ident}.ir"
        if ir.is_file():
            entry["ir"] = f"generated/{ir.name}"
            entry["sha256"]["ir"] = sha256(ir)
        else:
            ok = False
            print(f"{ident}: missing generated/{ident}.ir", file=sys.stderr)

        dslx = root / "dslx" / f"{ident}.x"
        if dslx.is_file():
            entry["dslx"] = f"dslx/{dslx.name}"
            entry["sha256"]["dslx"] = sha256(dslx)
        elif not args.allow_missing_dslx:
            ok = False
            print(f"{ident}: missing dslx/{ident}.x", file=sys.stderr)

        if adapter is not None:
            check = subprocess.run(
                [str(adapter), "--syntax-check", str(verilog)],
                capture_output=True, text=True, timeout=120,
            )
            entry["syntax_check"] = {
                "status": "ok" if check.returncode == 0 else "error",
                "adapter": "rv-xls-sim",
            }
            if check.returncode != 0:
                ok = False
                entry["syntax_check"]["stderr"] = check.stderr[-2000:]
                print(f"{ident}: adapter syntax check failed\n{check.stderr}",
                      file=sys.stderr)
        else:
            entry["syntax_check"] = {"status": "skipped"}

        cases.append(entry)

    if not cases:
        print("no generated Verilog cases found", file=sys.stderr)
        return 1

    manifest = {
        "schema": 1,
        "xls": {"repository": "google/xls", "tag": args.tag},
        "pipeline": PIPELINE,
        "cases": cases,
    }
    out = root / "manifest.json"
    out.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"manifest: {len(cases)} case(s) -> {out}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
