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

#ifndef XLS_SIMULATION_SIMULATORS_RVERILATOR_SIMULATOR_H_
#define XLS_SIMULATION_SIMULATORS_RVERILATOR_SIMULATOR_H_

#include <memory>
#include <string>
#include <string_view>
#include <utility>

#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"
#include "xls/simulation/verilog_simulator.h"

namespace xls {
namespace verilog {

// `VerilogSimulator` implementation that shells out to the `rv-xls-sim`
// process adapter (rust-cad-suite/crates/rv-xls-sim). The adapter drives the
// same unified engine as the rverilator CLI and the wasm playground, so this
// class is the XLS-side face of that core.
//
// Shape mirrors `IcarusVerilogSimulator`: temp dir + `top.v` + includes written
// to disk, `-D SIMULATION -D ASSERT_ON`, subprocess capture of stdout/stderr.
// The only difference is the binary: `rv-xls-sim` replaces `iverilog`/`vvp`
// with one process (the two-step compile/run contract lives inside it).
class RverilatorSimulator : public VerilogSimulator {
 public:
  explicit RverilatorSimulator(std::string adapter_path = "rv-xls-sim");

  absl::StatusOr<std::pair<std::string, std::string>> Run(
      std::string_view text, FileType file_type,
      absl::Span<const MacroDefinition> macro_definitions,
      absl::Span<const VerilogInclude> includes) const override;

  absl::Status RunSyntaxChecking(
      std::string_view text, FileType file_type,
      absl::Span<const MacroDefinition> macro_definitions,
      absl::Span<const VerilogInclude> includes) const override;

  bool DoesSupportSystemVerilog() const override { return false; }

  // Same answer as the iverilog simulator: XLS's assertion lowering emits
  // SystemVerilog `assert` forms this engine does not target.
  bool DoesSupportAssertions() const override { return false; }

 private:
  absl::StatusOr<std::pair<std::string, std::string>> RunInternal(
      std::string_view text, absl::Span<const MacroDefinition> macro_definitions,
      absl::Span<const VerilogInclude> includes, bool syntax_only) const;

  std::string adapter_path_;
};

// Convenience factory used by the module initializer (and by harnesses).
std::unique_ptr<VerilogSimulator> MakeRverilatorSimulator(
    std::string adapter_path = "rv-xls-sim");

}  // namespace verilog
}  // namespace xls

#endif  // XLS_SIMULATION_SIMULATORS_RVERILATOR_SIMULATOR_H_
