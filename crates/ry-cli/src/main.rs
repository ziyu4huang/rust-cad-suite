//! ryosys — yosys 的純 Rust 等价物,bin 面(docs/RYOSYS-MVP.md §1.2)。
//!
//! * `-p "<script>"`:唯一腳本入口(oracle 同形);切分契約=script.rs。
//! * `-V`:自標識 `ryosys <version>`,不冒充 yosys 版本串(分歧 D-ys1)。
//! * 兩層 registry:`-H` 普查 306 條=登記層 stub 表 286(registry.rs;
//!   Y0..Y3 硬錯,D-ys2)+實現層 8 命令(read_rtlil/read_json/
//!   write_rtlil/write_json/check/stat/hierarchy/simplemap——M0 五命令
//!   +M1 增 hierarchy/read_json/simplemap);普查外名字=逐字重現
//!   oracle 的 No such command 錯誤。
//! * ERROR 走 stderr、log 走 stdout(log.cc:92 分流契約);錯誤即中止
//!   腳本 exit 1。stdout 全程緩衝、退出前一次落盤。
//! * log 面:spacer(≥2 連續換行補齊)+ 編號 header(`N. <title>`),
//!   與 oracle byte-exact(difftest 歸一化只切 prologue/epilogue)。

mod cells;
mod check;
mod consteval;
mod hierarchy;
mod jsonin;
mod jsonout;
mod opt_expr;
mod proc;
mod registry;
mod sigmap;
mod sigops;
mod simplemap;
mod stat;

use ry_cli::script;

use ry_ir::Design;

/// 錯誤面:訊息印 `ERROR: ` 前綴到 stderr,exit 1。
pub struct HardError(pub String);

/// oracle log 面:newline_count 跟蹤 + 編號 header(spacer 語義移植)。
pub(crate) struct Log {
    out: String,
    newline_count: usize,
    header_stack: Vec<u32>,
    /// log_warning 的訊息(stderr 分流?否——warning 走 stdout 行+彙總計數)。
    pub warnings: Vec<String>,
}

impl Log {
    pub(crate) fn new() -> Log {
        Log { out: String::new(), newline_count: 0, header_stack: vec![0], warnings: Vec::new() }
    }



    pub(crate) fn log(&mut self, s: &str) {
        self.out.push_str(s);
        // yosys newline_count=整體輸出尾部的連續換行數(log.cc spacer 契約)。
        let trailing = self.out.chars().rev().take_while(|c| *c == '\n').count();
        self.newline_count = trailing;
    }

    pub(crate) fn spacer(&mut self) {
        if self.newline_count < 2 {
            self.log("\n");
        }
        if self.newline_count < 2 {
            self.log("\n");
        }
    }

    /// 直寫 out 的 pass 主體(check/stat)之後重同步換行統計。
    pub(crate) fn resync(&mut self) {
        let trailing = self.out.chars().rev().take_while(|c| *c == '\n').count();
        self.newline_count = trailing;
    }

    pub(crate) fn header(&mut self, title: &str) {
        self.spacer();
        if let Some(top) = self.header_stack.last_mut() {
            *top += 1;
        }
        let id: Vec<String> = self.header_stack.iter().map(|c| c.to_string()).collect();
        // title 自帶尾換行(yosys log_header 的格式串原樣)。
        self.log(&format!("{}. {}", id.join("."), title));
    }

    /// log_push/log_pop(hierarchy 的 sub-header 層級,「2.1.」形)。
    pub(crate) fn push(&mut self) {
        self.header_stack.push(0);
    }

    pub(crate) fn pop(&mut self) {
        self.header_stack.pop();
    }
}

struct Ctx {
    log: Log,
    design: Option<Design>,
    warnings: Vec<String>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-V") {
        println!("ryosys {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let script = match args.as_slice() {
        [p, s] if p == "-p" => s.clone(),
        _ => {
            eprintln!("Usage: ryosys [-V] -p <script>");
            std::process::exit(1);
        }
    };
    let mut ctx = Ctx { log: Log::new(), design: None, warnings: Vec::new() };
    let _ = &mut ctx.warnings;
    // driver 橫幅:逐字回顯 -p 串(歸一化保留此行;兩端同串=byte-exact)。
    ctx.log.log(&format!("-- Running command `{script}' --\n"));
    let commands = match script::split_script(&script) {
        Ok(c) => c,
        Err(_) => {
            // 目前唯一 Err 形 = `!` shell 逸出(Y0)。
            flush_and_fail(&ctx.log.out, "shell escape (`!`) is not implemented in ryosys (milestone Y0)");
        }
    };
    for argv in &commands {
        if let Err(HardError(msg)) = exec_command(&mut ctx, argv) {
            flush_and_fail(&ctx.log.out, &msg);
        }
        // Pass::post_execute(register.cc:129):每 pass 末印 debug 抑制計數。
        let n = proc::dbg_take();
        if n > 0 {
            ctx.log.log(&format!("<suppressed ~{n} debug messages>\n"));
        }
    }
    // driver 末尾 print_stats 塊(driver.cc:714-719):**無條件 spacer**
    // (此空行先於 `End of script` 錨,是 golden 內容——values 例 golden
    // 尾部 `backend.\n` 即它),之後僅 total>0 時印警告彙總。
    for w in std::mem::take(&mut ctx.log.warnings) {
        ctx.warnings.push(w);
    }
    ctx.log.spacer();
    if !ctx.warnings.is_empty() {
        let total = ctx.warnings.len();
        let mut unique: Vec<&String> = Vec::new();
        for w in &ctx.warnings {
            if !unique.contains(&w) {
                unique.push(w);
            }
        }
        ctx.log.log(&format!(
            "Warnings: {} unique messages, {total} total\n",
            unique.len()
        ));
    }
    print!("{}", ctx.log.out);
}

fn flush_and_fail(stdout: &str, msg: &str) -> ! {
    print!("{stdout}");
    eprintln!("ERROR: {msg}");
    std::process::exit(1);
}

fn exec_command(ctx: &mut Ctx, argv: &[String]) -> Result<(), HardError> {
    let name = argv[0].as_str();
    match name {
        "read_rtlil" => cmd_read_rtlil(ctx, argv),
        "read_json" => cmd_read_json(ctx, argv),
        "write_rtlil" => cmd_write_rtlil(ctx, argv),
        "write_json" => cmd_write_json(ctx, argv),
        "check" => cmd_check(ctx, argv),
        "stat" => cmd_stat(ctx, argv),
        "hierarchy" => cmd_hierarchy(ctx, argv),
        "simplemap" => cmd_simplemap(ctx, argv),
        "proc" => cmd_proc_family(ctx, argv, proc::run_proc),
        "proc_clean" => cmd_proc_family(ctx, argv, proc::run_proc_clean),
        "proc_rmdead" => cmd_proc_family(ctx, argv, proc::run_proc_rmdead),
        "proc_prune" => cmd_proc_family(ctx, argv, proc::run_proc_prune),
        "proc_init" => cmd_proc_family(ctx, argv, proc::run_proc_init),
        "proc_arst" => cmd_proc_family(ctx, argv, proc::run_proc_arst),
        "proc_rom" => cmd_proc_family(ctx, argv, proc::run_proc_rom),
        "proc_mux" => cmd_proc_family(ctx, argv, proc::run_proc_mux),
        "proc_dlatch" => cmd_proc_family(ctx, argv, proc::run_proc_dlatch),
        "proc_dff" => cmd_proc_family(ctx, argv, proc::run_proc_dff),
        "proc_memwr" => cmd_proc_family(ctx, argv, proc::run_proc_memwr),
        "opt_expr" => cmd_proc_family(ctx, argv, opt_expr::run_opt_expr),
        _ => {
            if let Some((_, ycode)) = registry::REGISTERED.iter().find(|(n, _)| *n == name) {
                Err(HardError(format!(
                    "{name}: not implemented in ryosys (milestone {ycode})"
                )))
            } else {
                Err(HardError(format!(
                    "No such command: {name} (type 'help' for a command overview)"
                )))
            }
        }
    }
}

fn need_design<'a>(ctx: &'a Ctx, cmd: &str) -> Result<&'a Design, HardError> {
    ctx.design.as_ref().ok_or_else(|| {
        HardError(format!(
            "No design object available (run 'read_rtlil' first) while executing command `{cmd}'"
        ))
    })
}

fn cmd_read_rtlil(ctx: &mut Ctx, argv: &[String]) -> Result<(), HardError> {
    ctx.log.header("Executing RTLIL frontend.\n");
    let filename = match argv.len() {
        2 => argv[1].clone(),
        _ => {
            return Err(HardError(
                "read_rtlil: stdin/std multi-file form is not implemented in ryosys (milestone Y0)"
                    .to_string(),
            ))
        }
    };
    let text = std::fs::read_to_string(&filename).map_err(|e| {
        HardError(format!("Can't open input file `{filename}' for reading: {e}"))
    })?;
    let design = ry_ir::parse_rtlil(&text).map_err(|e| HardError(e.to_string()))?;
    ctx.log.log(&format!("Input filename: {filename}\n"));
    ctx.design = Some(design);
    Ok(())
}

fn cmd_read_json(ctx: &mut Ctx, argv: &[String]) -> Result<(), HardError> {
    ctx.log.header("Executing JSON frontend.\n");
    let filename = match argv.len() {
        2 => argv[1].clone(),
        _ => {
            return Err(HardError(
                "read_json: stdin/std multi-file form is not implemented in ryosys (milestone Y0)"
                    .to_string(),
            ))
        }
    };
    let text = std::fs::read_to_string(&filename).map_err(|e| {
        HardError(format!("Can't open input file `{filename}' for reading: {e}"))
    })?;
    let mut body = String::new();
    let design = jsonin::read_json(&text, &mut body).map_err(HardError)?;
    ctx.log.log(&body);
    ctx.design = Some(design);
    ctx.log.resync();
    Ok(())
}

fn cmd_write_rtlil(ctx: &mut Ctx, argv: &[String]) -> Result<(), HardError> {
    let mut readable = false;
    let mut filename: Option<String> = None;
    for a in &argv[1..] {
        if a == "-readable" {
            readable = true;
        } else if a.starts_with('-') {
            return Err(HardError(format!(
                "write_rtlil: option `{a}' is not implemented in ryosys (milestone Y1)"
            )));
        } else {
            filename = Some(a.clone());
        }
    }
    if !readable {
        // D-ys3:默認壓縮形(twines)未實現。
        return Err(HardError(
            "write_rtlil: default (twines) form is not supported in ryosys (milestone Y1); pass -readable"
                .to_string(),
        ));
    }
    let Some(filename) = filename else {
        return Err(HardError(
            "write_rtlil: stdout output is not implemented in ryosys (milestone Y0)".to_string(),
        ));
    };
    let design = need_design(ctx, "write_rtlil")?.clone();
    ctx.log.header("Executing RTLIL backend.\n");
    let mut file = std::fs::File::create(&filename)
        .map_err(|e| HardError(format!("Can't open file `{filename}' for writing: {e}")))?;
    ctx.log.log(&format!("Output filename: {filename}\n"));
    let text = ry_ir::write_rtlil_readable(&design);
    std::io::Write::write_all(&mut file, text.as_bytes())
        .map_err(|e| HardError(format!("Can't write file `{filename}': {e}")))?;
    Ok(())
}

fn cmd_write_json(ctx: &mut Ctx, argv: &[String]) -> Result<(), HardError> {
    let filename = match argv.len() {
        2 => argv[1].clone(),
        _ => {
            return Err(HardError(
                "write_json: stdout output or extra options are not implemented in ryosys (milestone Y0)"
                    .to_string(),
            ))
        }
    };
    let design = need_design(ctx, "write_json")?.clone();
    ctx.log.header("Executing JSON backend.\n");
    // oracle:ofstream 緩衝未落盤即 throw → 0-byte 檔;ryosys 先建空檔,
    // process 硬錯發生在任何位元組寫出之前(json.cc:154 在模組體之前)。
    let mut file = std::fs::File::create(&filename)
        .map_err(|e| HardError(format!("Can't open file `{filename}' for writing: {e}")))?;
    let creator = format!("ryosys {}", env!("CARGO_PKG_VERSION"));
    match jsonout::write_json(&design, &creator) {
        Ok(text) => {
            std::io::Write::write_all(&mut file, text.as_bytes())
                .map_err(|e| HardError(format!("Can't write file `{filename}': {e}")))?;
        }
        Err(msg) => return Err(HardError(msg)),
    }
    Ok(())
}

fn cmd_hierarchy(ctx: &mut Ctx, argv: &[String]) -> Result<(), HardError> {
    let mut auto_top = false;
    let mut top_arg: Option<String> = None;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "-auto-top" => auto_top = true,
            "-top" => {
                i += 1;
                top_arg = argv.get(i).cloned();
            }
            other => {
                return Err(HardError(format!(
                    "hierarchy: option `{other}' is not implemented in ryosys (milestone Y0)"
                )));
            }
        }
        i += 1;
    }
    let Some(mut design) = ctx.design.clone() else {
        return Err(HardError(
            "No design object available (run 'read_rtlil' first) while executing command `hierarchy'"
                .to_string(),
        ));
    };
    ctx.log
        .header("Executing HIERARCHY pass (managing design hierarchy).\n");
    ctx.log.push();
    let res = hierarchy::run_hierarchy(&mut design, auto_top, top_arg.as_deref(), &mut ctx.log);
    ctx.log.pop();
    res.map_err(HardError)?;
    ctx.design = Some(design);
    ctx.log.resync();
    Ok(())
}

fn cmd_simplemap(ctx: &mut Ctx, argv: &[String]) -> Result<(), HardError> {
    if argv.len() > 1 {
        return Err(HardError(format!(
            "simplemap: options are not implemented in ryosys (milestone Y0): {}",
            argv[1]
        )));
    }
    let mut design = need_design(ctx, "simplemap")?.clone();
    ctx.log.header("Executing SIMPLEMAP pass (map simple cells to gate primitives).\n");
    simplemap::run_simplemap(&mut design, &mut ctx.log);
    ctx.design = Some(design);
    ctx.log.resync();
    Ok(())
}

/// proc 子 pass 共用外殼:M2 實現層(§8.3-2;design clone+ProcError 包裝)。
fn cmd_proc_family(
    ctx: &mut Ctx,
    argv: &[String],
    f: fn(&mut Design, &[String], &mut Log) -> Result<(), proc::ProcError>,
) -> Result<(), HardError> {
    let mut design = need_design(ctx, &argv[0].clone())?.clone();
    f(&mut design, argv, &mut ctx.log).map_err(|e| HardError(e.0))?;
    ctx.design = Some(design);
    Ok(())
}

fn cmd_check(ctx: &mut Ctx, argv: &[String]) -> Result<(), HardError> {
    if argv.len() > 1 {
        return Err(HardError(format!(
            "check: options are not implemented in ryosys (milestone Y0): {}",
            argv[1]
        )));
    }
    let design = need_design(ctx, "check")?.clone();
    ctx.log.header("Executing CHECK pass (checking for obvious problems).\n");
    check::run_check(&design, &mut ctx.log.out, &mut ctx.warnings);
    ctx.log.resync();
    Ok(())
}

fn cmd_stat(ctx: &mut Ctx, argv: &[String]) -> Result<(), HardError> {
    if argv.len() > 1 {
        return Err(HardError(format!(
            "stat: options are not implemented in ryosys (milestone Y0): {}",
            argv[1]
        )));
    }
    let design = need_design(ctx, "stat")?.clone();
    ctx.log.header("Printing statistics.\n");
    stat::run_stat(&design, &mut ctx.log.out);
    ctx.log.resync();
    Ok(())
}
