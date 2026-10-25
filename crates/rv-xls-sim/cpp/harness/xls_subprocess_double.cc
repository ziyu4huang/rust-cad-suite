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

// Local harness double for `xls::InvokeSubprocess`.
//
// The simulator subclass is written against XLS's real
// `xls/common/subprocess.h`, but compiling XLS's subprocess.cc pulls a chain of
// XLS utility files (file_descriptor, logging, thread). This TU implements the
// one function the subclass calls, with the same signature and the same
// result contract, using posix_spawn + pipes. It exists only for the
// out-of-tree harness in this directory; in-tree builds link XLS's real
// implementation instead.

#include <poll.h>
#include <spawn.h>
#include <sys/wait.h>
#include <unistd.h>

#include <cerrno>
#include <cstring>
#include <filesystem>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/types/span.h"
#include "xls/common/subprocess.h"

extern char** environ;

namespace xls {

absl::StatusOr<SubprocessResult> InvokeSubprocess(
    absl::Span<const std::string> argv,
    std::optional<std::filesystem::path> cwd,
    std::optional<absl::Duration> optional_timeout,
    absl::Span<const EnvironmentVariable> env) {
  (void)cwd;              // The harness never changes directories.
  (void)optional_timeout; // The harness runs bounded commands.
  (void)env;              // The harness inherits the caller environment.

  if (argv.empty()) {
    return absl::InvalidArgumentError("InvokeSubprocess: empty argv");
  }

  int out_fds[2];
  int err_fds[2];
  if (pipe(out_fds) != 0) {
    return absl::InternalError(
        absl::StrCat("pipe: ", std::strerror(errno)));
  }
  if (pipe(err_fds) != 0) {
    close(out_fds[0]);
    close(out_fds[1]);
    return absl::InternalError(
        absl::StrCat("pipe: ", std::strerror(errno)));
  }

  posix_spawn_file_actions_t actions;
  posix_spawn_file_actions_init(&actions);
  posix_spawn_file_actions_adddup2(&actions, out_fds[1], STDOUT_FILENO);
  posix_spawn_file_actions_adddup2(&actions, err_fds[1], STDERR_FILENO);
  posix_spawn_file_actions_addclose(&actions, out_fds[0]);
  posix_spawn_file_actions_addclose(&actions, err_fds[0]);
  posix_spawn_file_actions_addclose(&actions, out_fds[1]);
  posix_spawn_file_actions_addclose(&actions, err_fds[1]);

  std::vector<char*> cargv;
  cargv.reserve(argv.size() + 1);
  for (const std::string& arg : argv) {
    cargv.push_back(const_cast<char*>(arg.c_str()));
  }
  cargv.push_back(nullptr);

  pid_t pid = -1;
  const int spawn_error =
      posix_spawn(&pid, cargv[0], &actions, nullptr, cargv.data(), environ);
  posix_spawn_file_actions_destroy(&actions);
  close(out_fds[1]);
  close(err_fds[1]);
  if (spawn_error != 0) {
    close(out_fds[0]);
    close(err_fds[0]);
    return absl::InternalError(absl::StrCat(
        "posix_spawn ", argv[0], ": ", std::strerror(spawn_error)));
  }

  std::string out;
  std::string err;
  pollfd fds[2] = {{out_fds[0], POLLIN, 0}, {err_fds[0], POLLIN, 0}};
  bool open_fd[2] = {true, true};
  char buffer[8192];
  while (open_fd[0] || open_fd[1]) {
    const int polled = poll(fds, 2, -1);
    if (polled < 0) {
      if (errno == EINTR) {
        continue;
      }
      break;
    }
    for (int index = 0; index < 2; ++index) {
      if (!open_fd[index] ||
          (fds[index].revents & (POLLIN | POLLHUP | POLLERR)) == 0) {
        continue;
      }
      const ssize_t read_bytes = read(fds[index].fd, buffer, sizeof(buffer));
      if (read_bytes > 0) {
        (index == 0 ? out : err).append(buffer, static_cast<size_t>(read_bytes));
      } else if (read_bytes == 0 ||
                 (read_bytes < 0 && errno != EINTR)) {
        open_fd[index] = false;
        close(fds[index].fd);
        fds[index].fd = -1;
      }
    }
  }

  int status = 0;
  while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {
  }

  SubprocessResult result;
  result.stdout_content = std::move(out);
  result.stderr_content = std::move(err);
  result.normal_termination = WIFEXITED(status);
  result.exit_status = WIFEXITED(status) ? WEXITSTATUS(status) : -1;
  result.timeout_expired = false;
  return result;
}

}  // namespace xls
