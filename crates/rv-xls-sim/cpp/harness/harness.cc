// Copyright 2026 The rust-cad-suite Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Out-of-tree harness for `RverilatorSimulator`.
//
// Compiles the subclass against the *real* upstream
// `xls/simulation/verilog_simulator.h` (pinned by scripts/xls-cpp-gate.sh) and
// links a local stand-in for `xls::InvokeSubprocess`, then exercises the same
// contract the Rust crate pins: XLS corpus byte-equality, macros, in-memory
// includes, syntax checking, SystemVerilog refusal, and error propagation.
//
// Usage: rv-xls-cpp-harness <path-to-rv-xls-sim> <repo-root>

#include <filesystem>
#include <fstream>
#include <iostream>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"
#include "xls/simulation/simulators/rverilator_simulator.h"

namespace {

int g_failures = 0;

void Check(bool ok, std::string_view name, std::string_view detail = "") {
  if (ok) {
    std::cout << "  PASS  " << name << "\n";
    return;
  }
  ++g_failures;
  std::cout << "  FAIL  " << name;
  if (!detail.empty()) {
    std::cout << " :: " << detail;
  }
  std::cout << "\n";
}

std::string ReadFile(const std::filesystem::path& path) {
  std::ifstream in(path, std::ios::binary);
  if (!in) {
    std::cerr << "cannot read " << path << "\n";
    return "";
  }
  return std::string(std::istreambuf_iterator<char>(in),
                     std::istreambuf_iterator<char>());
}

std::string PairSummary(
    const absl::StatusOr<std::pair<std::string, std::string>>& result) {
  if (!result.ok()) {
    return std::string(result.status().message());
  }
  return result->first;
}

}  // namespace

int main(int argc, char** argv) {
  if (argc < 3) {
    std::cerr << "usage: rv-xls-cpp-harness <rv-xls-sim> <repo-root>\n";
    return 2;
  }
  const std::string adapter = argv[1];
  const std::filesystem::path repo = argv[2];

  xls::verilog::RverilatorSimulator simulator(adapter);
  using xls::verilog::FileType;
  using xls::verilog::VerilogInclude;
  using xls::verilog::VerilogSimulator;
  const absl::Span<const VerilogSimulator::MacroDefinition> no_macros;
  const absl::Span<const VerilogInclude> no_includes;

  // 1. The harvested XLS case, as one text, must reproduce the oracle stdout.
  const std::filesystem::path case_dir =
      repo / "corpus/e2e/57_xls_add3";
  const std::string text =
      ReadFile(case_dir / "add3.v") + "\n" + ReadFile(case_dir / "tb.v");
  const std::string expected = ReadFile(case_dir / "expected.stdout");
  const auto run = simulator.Run(text, FileType::kVerilog, no_macros, no_includes);
  Check(run.ok(), "Run(add3) succeeds", PairSummary(run));
  if (run.ok()) {
    Check(run->first == expected, "Run(add3) stdout is byte-exact",
          run->first);
    Check(run->second.empty(), "Run(add3) stderr is empty", run->second);
  }

  // 2. Syntax checking is the RunSyntaxChecking face.
  const auto syntax =
      simulator.RunSyntaxChecking(text, FileType::kVerilog, no_macros, no_includes);
  Check(syntax.ok(), "RunSyntaxChecking(add3) succeeds",
        std::string(syntax.message()));

  // 3. A broken design surfaces as a non-OK status carrying the diagnostics.
  const auto broken = simulator.Run("module tb(\n", FileType::kVerilog,
                                    no_macros, no_includes);
  Check(!broken.ok(), "Run(broken) fails");
  if (!broken.ok()) {
    Check(broken.status().code() == absl::StatusCode::kInternal &&
              broken.status().message().find("rv-xls-sim failed") !=
                  std::string::npos,
          "Run(broken) message carries the adapter diagnostics",
          std::string(broken.status().message()));
  }

  // 4. SystemVerilog is refused exactly like the iverilog implementation.
  const auto sv = simulator.Run("module tb; endmodule\n",
                                FileType::kSystemVerilog, no_macros,
                                no_includes);
  Check(!sv.ok() && sv.status().code() == absl::StatusCode::kUnimplemented,
        "Run(.sv) is Unimplemented");

  // 5. Macro definitions reach the preprocessor (not just accepted).
  const std::string macro_text =
      "`ifdef BIG\n"
      "module tb; initial begin $display(\"big\"); $finish; end endmodule\n"
      "`else\n"
      "module tb; initial begin $display(\"small\"); $finish; end endmodule\n"
      "`endif\n";
  const std::vector<VerilogSimulator::MacroDefinition> macros = {
      {"BIG", std::nullopt}};
  const auto macro =
      simulator.Run(macro_text, FileType::kVerilog, macros, no_includes);
  Check(macro.ok() && macro->first == "big\n",
        "Run(macros) applies -D BIG", PairSummary(macro));

  // 6. VerilogInclude carries text, not a path: the subclass must write it.
  const std::vector<VerilogInclude> includes = {
      {std::filesystem::path("defs.vh"), "`define VAL 7\n"}};
  const auto include =
      simulator.Run("`include \"defs.vh\"\n"
                    "module tb; initial begin "
                    "$display(\"val=%0d\", `VAL); $finish; end endmodule\n",
                    FileType::kVerilog, no_macros, includes);
  Check(include.ok() && include->first == "val=7\n",
        "Run(includes) writes VerilogInclude text", PairSummary(include));

  // 7. Syntax-check failure is an error status (no half-OK).
  const auto bad_syntax = simulator.RunSyntaxChecking(
      "module tb(\n", FileType::kVerilog, no_macros, no_includes);
  Check(!bad_syntax.ok(), "RunSyntaxChecking(broken) fails");

  if (g_failures == 0) {
    std::cout << "\nALL CHECKS PASS\n";
    return 0;
  }
  std::cout << "\n" << g_failures << " CHECK(S) FAILED\n";
  return 1;
}
