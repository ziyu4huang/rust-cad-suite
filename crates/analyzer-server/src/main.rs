//! analyzerd — 統一分析器 UDS 服務 face(ANALYZER-SERVICE §5;ADR-0011)。
//!
//! 10.7 core-first:本 bin 不含語義——接收→解碼(analyzer-core::proto)→
//! 調 analyzer_core::analyze→編碼(analyzer_core::report)→回寫。
//! 併發模型=thread-per-connection、連線間並發、單連線內串行(§5.7);
//! 停機=shutdown op 回 ack → unlink → drain(2s 上限)→ exit 0(§5.7);
//! 啟動期對既有 socket 先試連:連通=活實例硬錯 exit 1,連不通=unlink 重綁
//! (§5.8)。CLI 契約 §5.9。

mod conn;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use analyzer_core::proto::frame;

pub const DEFAULT_SOCKET: &str = "/tmp/analyzerd.sock";
/// macOS sun_path 上限 104 含 NUL;留 4 字節餘量(§5.8)。
pub const MAX_SOCK_PATH_BYTES: usize = 100;
pub const DRAIN_WINDOW: Duration = Duration::from_secs(2);
const ACCEPT_POLL: Duration = Duration::from_millis(5);

pub struct ServerState {
    pub shutdown: AtomicBool,
    pub conn_count: AtomicUsize,
    pub start: Instant,
    pub sock_path: String,
    pub max_frame: u32,
}

impl ServerState {
    fn new(sock_path: String, max_frame: u32) -> Arc<Self> {
        Arc::new(ServerState {
            shutdown: AtomicBool::new(false),
            conn_count: AtomicUsize::new(0),
            start: Instant::now(),
            sock_path,
            max_frame,
        })
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("analyzerd: {msg}");
            ExitCode::from(1)
        }
    }
}

/// CLI 解析產物:兩欄皆 Option——None=未顯式指定,在 run() 解析點按
/// §5.8/§5.9 優先級落地(單獨 `--max-frame` 不得硬編碼 DEFAULT_SOCKET
/// 搶走 ANALYZER_SOCK;實錄:繞過 env bug,收平輪回歸測試釘死)。
struct Config {
    sock_path: Option<String>,
    max_frame: Option<u32>,
}

/// socket 路徑解析點:`--socket` > `ANALYZER_SOCK` > 默認(§5.8)。env
/// 查詢獨立注入,單測零進程環境依賴(cargo test 並行安全)。
fn resolve_sock_path(cli: Option<String>, env: Option<String>) -> String {
    cli.or(env).unwrap_or_else(|| DEFAULT_SOCKET.to_string())
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let cfg = parse_args(args)?;
    if let Some(code) = handle_info_flags(args)? {
        return Ok(code);
    }
    let sock_path = resolve_sock_path(cfg.sock_path, std::env::var("ANALYZER_SOCK").ok());
    let max_frame = cfg.max_frame.unwrap_or(frame::MAX_FRAME_DEFAULT);

    let path_bytes = sock_path.as_bytes().len();
    if path_bytes >= MAX_SOCK_PATH_BYTES {
        return Err(format!(
            "socket path too long: {path_bytes} bytes (limit {MAX_SOCK_PATH_BYTES}; \
             macOS sun_path is 104 bytes incl. NUL) — use --socket or ANALYZER_SOCK"
        ));
    }

    let path = Path::new(&sock_path);
    if path.exists() {
        // 先試連(§5.8):連通=已有活實例;連不通=殘留死文件,unlink 重綁。
        match UnixStream::connect(path) {
            Ok(_probe) => {
                return Err(format!(
                    "already serving on {} (another live instance owns the socket); \
                     refusing to bind a second instance",
                    sock_path
                ));
            }
            Err(_) => {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    let listener = match UnixListener::bind(path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("analyzerd: cannot bind {}: {e}", sock_path);
            return Ok(ExitCode::from(2));
        }
    };
    // 0o600 owner-only = 本機訪問邊界(§5.8)。
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    let _ = listener.set_nonblocking(true);

    let st = ServerState::new(sock_path.clone(), max_frame);
    eprintln!("analyzerd: serving on {} (ASP/1, max-frame {})", sock_path, max_frame);

    // accept 輪詢循環(listener non-blocking,5ms 輪):SHUTDOWN 置位後新連線
    // 立即 close 不發幀(§5.6 仲裁);drain 窗口=CONN_COUNT==0 或 2s(§5.7④)。
    let mut drain_deadline: Option<Instant> = None;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                // macOS:accepted fd 繼承 listener 的 O_NONBLOCK(實錄:conn 線程
                // 首讀 EAGAIN os error 35 → 誤判 Eof → 連線被斷);強制回阻塞態。
                let _ = stream.set_nonblocking(false);
                if st.shutdown.load(Ordering::SeqCst) {
                    drop(stream);
                } else {
                    st.conn_count.fetch_add(1, Ordering::SeqCst);
                    let st2 = Arc::clone(&st);
                    let _ = std::thread::Builder::new()
                        .name("analyzerd-conn".to_string())
                        .spawn(move || {
                            conn::serve(stream, Arc::clone(&st2));
                            st2.conn_count.fetch_sub(1, Ordering::SeqCst);
                        });
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => {
                eprintln!("analyzerd: accept failed: {e}");
                return Ok(ExitCode::from(2));
            }
        }
        if st.shutdown.load(Ordering::SeqCst) {
            if drain_deadline.is_none() {
                drain_deadline = Some(Instant::now() + DRAIN_WINDOW);
            }
            if st.conn_count.load(Ordering::SeqCst) == 0 || Instant::now() >= drain_deadline.unwrap()
            {
                break;
            }
        }
        std::thread::sleep(ACCEPT_POLL);
    }
    // 兜底 unlink(冪等:shutdown 連線已 unlink 過;此處防競態殘留)。
    let _ = std::fs::remove_file(&sock_path);
    let _ = std::io::stdout().flush();
    Ok(ExitCode::from(0))
}

/// `--version` / `--help`(即時返回,不綁 socket)。
fn handle_info_flags(args: &[String]) -> Result<Option<ExitCode>, String> {
    if args.iter().any(|a| a == "--version") {
        println!("analyzerd {} (ASP/1)", env!("CARGO_PKG_VERSION"));
        return Ok(Some(ExitCode::from(0)));
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "analyzerd {} (ASP/1) — rust-cad-suite 統一分析器 UDS 服務

用法: analyzerd [--socket PATH] [--max-frame BYTES] [--version] [--help]

  --socket PATH      UDS 路徑(默認 {DEFAULT_SOCKET};環境 ANALYZER_SOCK 次之)
  --max-frame BYTES  幀上限字節數(默認 {};鉗位上限 {} = 1 GiB)
  --version          打印版本行
  --help             本說明

未知旗標=硬錯 exit 1;退出碼:0=正常(含 shutdown)、1=用法/配置錯、2=綁定 IO 錯。",
            env!("CARGO_PKG_VERSION"),
            frame::MAX_FRAME_DEFAULT,
            frame::MAX_FRAME_LIMIT,
        );
        return Ok(Some(ExitCode::from(0)));
    }
    Ok(None)
}

/// CLI 解析(§5.9):未知旗標硬錯;旗標值只記入 Config(Option 欄),
/// 優先級落地在 run() 解析點(resolve_sock_path)。
fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut cfg = Config { sock_path: None, max_frame: None };
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--version" || a == "--help" || a == "-h" {
            i += 1;
            continue;
        }
        let Some(value) = a.strip_prefix("--socket=") else {
            if a == "--socket" {
                i += 1;
                let Some(v) = args.get(i) else {
                    return Err("--socket requires a PATH argument".to_string());
                };
                set_socket(&mut cfg, v);
                i += 1;
                continue;
            }
            let Some(value) = a.strip_prefix("--max-frame=") else {
                if a == "--max-frame" {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        return Err("--max-frame requires a BYTES argument".to_string());
                    };
                    set_max_frame(&mut cfg, v)?;
                    i += 1;
                    continue;
                }
                return Err(format!("unknown flag `{a}` (see --help)"));
            };
            set_max_frame(&mut cfg, value)?;
            i += 1;
            continue;
        };
        set_socket(&mut cfg, value);
        i += 1;
    }
    Ok(cfg)
}

fn set_socket(cfg: &mut Config, v: &str) {
    cfg.sock_path = Some(v.to_string());
}

fn set_max_frame(cfg: &mut Config, v: &str) -> Result<(), String> {
    let n: u64 = v
        .parse()
        .map_err(|_| format!("--max-frame expects an integer, got `{v}`"))?;
    cfg.max_frame = Some(frame::clamp_max_frame(n.min(u32::MAX as u64) as u32));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    /// 解析點優先級(§5.9):CLI > env > 默認,三象限。
    #[test]
    fn resolve_sock_path_priority() {
        // 兩者皆在:CLI 勝。
        assert_eq!(
            resolve_sock_path(Some("/cli.sock".into()), Some("/env.sock".into())),
            "/cli.sock"
        );
        // 無 CLI 有 env:env 勝。
        assert_eq!(
            resolve_sock_path(None, Some("/env.sock".into())),
            "/env.sock"
        );
        // 皆無:默認。
        assert_eq!(
            resolve_sock_path(None, None),
            DEFAULT_SOCKET
        );
    }

    /// 回歸錨(收平輪 ②):單獨 `--max-frame` 不得把 sock_path 釘成
    /// DEFAULT_SOCKET——那會在 run() 解析點前搶走 ANALYZER_SOCK。
    /// 空參與 --max-frame 兩形 sock_path 都必須是 None(未指定)。
    #[test]
    fn parse_args_max_frame_alone_keeps_sock_unset() {
        let c = parse_args(&args(&[])).unwrap();
        assert!(c.sock_path.is_none());
        assert!(c.max_frame.is_none());

        let c = parse_args(&args(&["--max-frame", "1024"])).unwrap();
        assert!(c.sock_path.is_none(), "單獨 --max-frame 禁硬編碼默認路徑");
        assert_eq!(c.max_frame, Some(1024));

        let c = parse_args(&args(&["--max-frame=1024"])).unwrap();
        assert!(c.sock_path.is_none(), "= 形同罪");
        assert_eq!(c.max_frame, Some(1024));
    }

    /// 兩旗標同給:sock/max 互不覆蓋;max-frame 鉗位契約維持。
    #[test]
    fn parse_args_both_flags_independent() {
        let c = parse_args(&args(&["--socket", "/x.sock", "--max-frame", "999999999999999"])).unwrap();
        assert_eq!(c.sock_path.as_deref(), Some("/x.sock"));
        let mf = c.max_frame.unwrap();
        assert!(mf <= frame::MAX_FRAME_LIMIT);
        assert_eq!(mf, frame::clamp_max_frame(u32::MAX));

        let c = parse_args(&args(&["--socket=/y.sock", "--max-frame=4096"])).unwrap();
        assert_eq!(c.sock_path.as_deref(), Some("/y.sock"));
        assert_eq!(c.max_frame, Some(4096));
    }
}

