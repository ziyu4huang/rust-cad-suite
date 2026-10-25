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

#include "xls/simulation/simulators/rverilator_simulator.h"

#include <atomic>
#include <chrono>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <string>
#include <utility>
#include <vector>

#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/types/span.h"
#include "xls/common/subprocess.h"

#if !defined(RV_XLS_SIM_HARNESS)
#include "absl/log/check.h"
#include "xls/common/module_initializer.h"
#endif

namespace xls {
namespace verilog {
namespace {

std::atomic<uint64_t> g_temp_counter{0};

// Owns a temp directory and removes it on scope exit. Movable so it can live
// inside a StatusOr.
class TempDir {
 public:
  TempDir(const TempDir&) = delete;
  TempDir& operator=(const TempDir&) = delete;

  TempDir(TempDir&& other) noexcept : path_(std::move(other.path_)) {
    other.path_.clear();
  }

  TempDir& operator=(TempDir&& other) noexcept {
    if (this != &other) {
      Cleanup();
      path_ = std::move(other.path_);
      other.path_.clear();
    }
    return *this;
  }

  ~TempDir() { Cleanup(); }

  static absl::StatusOr<TempDir> Create() {
    std::error_code error;
    const std::filesystem::path base =
        std::filesystem::temp_directory_path(error);
    if (error) {
      return absl::InternalError(
          absl::StrCat("temp_directory_path: ", error.message()));
    }
    const auto stamp =
        std::chrono::steady_clock::now().time_since_epoch().count();
    for (int attempt = 0; attempt < 100; ++attempt) {
      const uint64_t id =
          g_temp_counter.fetch_add(1, std::memory_order_relaxed);
      std::filesystem::path dir =
          base / absl::StrCat("rv-xls-sim-cpp-", stamp, "-", id);
      if (std::filesystem::create_directory(dir, error)) {
        return TempDir(std::move(dir));
      }
      if (error) {
        return absl::InternalError(absl::StrCat(
            "cannot create temp dir ", dir.string(), ": ", error.message()));
      }
      // Name already existed; try the next id.
    }
    return absl::InternalError("could not create a unique temp dir");
  }

  const std::filesystem::path& path() const { return path_; }

 private:
  explicit TempDir(std::filesystem::path path) : path_(std::move(path)) {}

  void Cleanup() {
    if (path_.empty()) {
      return;
    }
    std::error_code error;
    std::filesystem::remove_all(path_, error);
    path_.clear();
  }

  std::filesystem::path path_;
};

absl::Status WriteFile(const std::filesystem::path& path,
                       std::string_view text) {
  std::error_code error;
  if (!path.parent_path().empty()) {
    std::filesystem::create_directories(path.parent_path(), error);
    if (error) {
      return absl::InternalError(absl::StrCat("cannot create ",
                                              path.parent_path().string(), ": ",
                                              error.message()));
    }
  }
  std::ofstream out(path, std::ios::binary | std::ios::trunc);
  if (!out) {
    return absl::InternalError(
        absl::StrCat("cannot open ", path.string(), " for writing"));
  }
  out.write(text.data(), static_cast<std::streamsize>(text.size()));
  if (!out) {
    return absl::InternalError(
        absl::StrCat("cannot write ", path.string()));
  }
  return absl::OkStatus();
}

}  // namespace

RverilatorSimulator::RverilatorSimulator(std::string adapter_path)
    : adapter_path_(std::move(adapter_path)) {}

absl::StatusOr<std::pair<std::string, std::string>> RverilatorSimulator::Run(
    std::string_view text, FileType file_type,
    absl::Span<const MacroDefinition> macro_definitions,
    absl::Span<const VerilogInclude> includes) const {
  if (file_type != FileType::kVerilog) {
    return absl::UnimplementedError(
        "rverilator targets Verilog-2005; SystemVerilog is out of scope");
  }
  return RunInternal(text, macro_definitions, includes,
                     /*syntax_only=*/false);
}

absl::Status RverilatorSimulator::RunSyntaxChecking(
    std::string_view text, FileType file_type,
    absl::Span<const MacroDefinition> macro_definitions,
    absl::Span<const VerilogInclude> includes) const {
  if (file_type != FileType::kVerilog) {
    return absl::UnimplementedError(
        "rverilator targets Verilog-2005; SystemVerilog is out of scope");
  }
  absl::StatusOr<std::pair<std::string, std::string>> result =
      RunInternal(text, macro_definitions, includes, /*syntax_only=*/true);
  if (!result.ok()) {
    return result.status();
  }
  return absl::OkStatus();
}

absl::StatusOr<std::pair<std::string, std::string>>
RverilatorSimulator::RunInternal(
    std::string_view text, absl::Span<const MacroDefinition> macro_definitions,
    absl::Span<const VerilogInclude> includes, bool syntax_only) const {
  absl::StatusOr<TempDir> temp = TempDir::Create();
  if (!temp.ok()) {
    return temp.status();
  }
  const std::filesystem::path dir = temp->path();
  const std::filesystem::path top_v = dir / GetTopFileName(FileType::kVerilog);

  absl::Status written = WriteFile(top_v, text);
  if (!written.ok()) {
    return written;
  }
  for (const VerilogInclude& include : includes) {
    written =
        WriteFile(dir / include.relative_path, include.verilog_text);
    if (!written.ok()) {
      return written;
    }
  }

  std::vector<std::string> args;
  args.reserve(8 + 2 * macro_definitions.size());
  args.push_back(adapter_path_);
  args.push_back(top_v.string());
  args.push_back("-I");
  args.push_back(dir.string());
  // Same defines the iverilog simulator always supplies.
  args.push_back("-D");
  args.push_back(std::string(VerilogSimulator::kSimulationMacroName));
  args.push_back("-D");
  args.push_back(std::string(VerilogSimulator::kAssertOnMacroName));
  for (const MacroDefinition& macro : macro_definitions) {
    args.push_back("-D");
    if (macro.value.has_value()) {
      args.push_back(absl::StrCat(macro.name, "=", *macro.value));
    } else {
      args.push_back(macro.name);
    }
  }
  if (syntax_only) {
    args.push_back("--syntax-check");
  }

  absl::StatusOr<SubprocessResult> result =
      InvokeSubprocess(absl::MakeConstSpan(args));
  if (!result.ok()) {
    return result.status();
  }
  if (!result->normal_termination) {
    return absl::InternalError("rv-xls-sim terminated abnormally");
  }
  if (result->exit_status != 0) {
    return absl::InternalError(absl::StrCat(
        "rv-xls-sim failed (exit ", result->exit_status, "):\n",
        result->stderr_content));
  }
  return std::make_pair(result->stdout_content, result->stderr_content);
}

std::unique_ptr<VerilogSimulator> MakeRverilatorSimulator(
    std::string adapter_path) {
  return std::make_unique<RverilatorSimulator>(std::move(adapter_path));
}

#if !defined(RV_XLS_SIM_HARNESS)
namespace {
XLS_REGISTER_MODULE_INITIALIZER(rverilator_simulator, {
  CHECK_OK(GetVerilogSimulatorManagerSingleton().RegisterVerilogSimulator(
      "rverilator", []() -> absl::StatusOr<std::unique_ptr<VerilogSimulator>> {
        return MakeRverilatorSimulator();
      }));
});
}  // namespace
#endif

}  // namespace verilog
}  // namespace xls
