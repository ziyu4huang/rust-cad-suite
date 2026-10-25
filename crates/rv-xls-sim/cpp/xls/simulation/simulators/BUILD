# Copyright 2026 The rust-cad-suite Authors
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# Drop-in BUILD target for an XLS checkout: copy this directory to
# xls/simulation/simulators/ and add
#   "//xls/simulation/simulators:rverilator_simulator"
# to the deps of //xls/simulation:verilog_simulators.

load("@rules_cc//cc:cc_library.bzl", "cc_library")

package(
    default_applicable_licenses = ["//:license"],
    default_visibility = ["//xls:xls_internal"],
    features = [
        "layering_check",
        "parse_headers",
    ],
    licenses = ["notice"],  # Apache 2.0
)

cc_library(
    name = "rverilator_simulator",
    srcs = ["rverilator_simulator.cc"],
    hdrs = ["rverilator_simulator.h"],
    deps = [
        "//xls/common:module_initializer",
        "//xls/common:subprocess",
        "//xls/simulation:verilog_include",
        "//xls/simulation:verilog_simulator",
        "@abseil-cpp//absl/log:check",
        "@abseil-cpp//absl/status",
        "@abseil-cpp//absl/status:statusor",
        "@abseil-cpp//absl/strings",
        "@abseil-cpp//absl/types:span",
    ],
    alwayslink = 1,
)
