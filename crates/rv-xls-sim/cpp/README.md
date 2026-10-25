# XLS side: `RverilatorSimulator`

This directory is the **upstream half** of the XLS integration: a
`VerilogSimulator` subclass (plus its BUILD target) that XLS can compile into
its own simulator manager. It shells out to `rv-xls-sim`, the process adapter in
`..` (the Rust crate), which drives the same unified engine as the
`rverilator` CLI and the wasm playground.

```
cpp/xls/simulation/simulators/rverilator_simulator.{h,cc}
cpp/xls/simulation/simulators/BUILD            # drop-in Bazel target
cpp/harness/                                   # out-of-tree compile + run harness
```

## Upstream shape it implements

Pinned to `google/xls` release `v0.0.0-10898-g8ddae8bf4` (the interface at that
tag, checked with a real header compile — see `scripts/xls-cpp-gate.sh`):

```cpp
absl::StatusOr<std::pair<std::string, std::string>> Run(
    std::string_view text, FileType file_type,
    absl::Span<const MacroDefinition> macros,
    absl::Span<const VerilogInclude> includes) const override;

absl::Status RunSyntaxChecking(...) const override;
bool DoesSupportSystemVerilog() const override { return false; }
bool DoesSupportAssertions() const override { return false; }
```

`VerilogInclude` carries **text**, not a path. Like `IcarusVerilogSimulator`,
the subclass creates a temp dir, writes `top.v` and every include file, passes
`-I <tempdir>` plus the always-on `-D SIMULATION -D ASSERT_ON`, and invokes the
adapter. SystemVerilog is refused with `UnimplementedError` before any process
is spawned.

## Wiring into an XLS checkout (S3)

1. Copy `xls/` over the checkout root (or just the three files into
   `xls/simulation/simulators/`).
2. Add `"//xls/simulation/simulators:rverilator_simulator"` to the `deps` of
   `//xls/simulation:verilog_simulators` (the same place
   `//xls/simulation/simulators:iverilog_simulator` is listed).
3. Make `rv-xls-sim` available to the test environment:
   - either install it on `PATH`, or
   - pass an absolute path to `RverilatorSimulator` /
     `MakeRverilatorSimulator`.
   XLS tests then select it with their simulator flag (the name registered by
   the module initializer is `rverilator`).

Not done here: the `bazel test` run inside XLS. This host has no Bazel, and the
XLS release asset is a **linux-x64 ELF** (`file`: `ELF 64-bit LSB pie executable,
x86-64, interpreter /lib64/ld-linux-x86-64.so.2`); macOS reports "cannot
execute binary file". The workflow runs that binary on `ubuntu-latest`; the
local gate below compiles against the pinned **headers** instead, which is the
strongest verification available without a Linux build of XLS.

## Local verification

```sh
bash scripts/xls-cpp-gate.sh
```

The script:

1. sparse-clones the pinned `google/xls` tag into
   `${TMPDIR:-/tmp}/rv-xls-cpp-gate` (override with `XLS_SRC=...`),
2. generates the XLS `xls/ir/*.proto` headers with `protoc`,
3. compiles the subclass **against the real upstream
   `xls/simulation/verilog_simulator.h`** (syntax-only pass with the module
   initializer enabled),
4. compiles the subclass + harness with `-DRV_XLS_SIM_HARNESS` and links a
   local `xls::InvokeSubprocess` double (`harness/xls_subprocess_double.cc`),
5. runs the harness against the real `rv-xls-sim` binary and the committed
   `corpus/e2e/57_xls_add3` expectations.

Prerequisites (checked by the script): `clang++` with C++20, `protoc`,
`abseil` (`brew install abseil protobuf`), plus a Rust toolchain.

What the harness proves: the subclass compiles against the real interface,
writes in-memory includes, passes macros, maps exit codes to absl statuses, and
reproduces `add3=6 / add3=355` byte-for-byte. What it does not prove: linking
inside XLS's Bazel graph and running XLS's own test suite.
