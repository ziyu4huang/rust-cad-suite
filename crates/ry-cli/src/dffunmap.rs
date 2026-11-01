//! dffunmap(passes/techmap/dffunmap.cc 同構移植;RYOSYS-MVP §11.3-4)。
//!
//! * log_header(:48 逐字)→selected_modules×selected_cells 值拷貝(rev)
//!   →FfData has_clk 分派→unmap_ce/unmap_srst/unmap_ce_srst→emit;
//!   靜默面=僅 header+error 面。
//! * 已知旗標 `-ce-only`/`-srst-only`(含雙投互斥形)=Y2 硬錯(§11.1-8;
//!   oracle 真實旗標語義=行為面歧);未知旗標/selection 裸字=cmd_error
//!   全形(D-ys11);fine cell($_ 族)=Y2 硬錯(語料零觸發邊界)。
//! * unmap NEW_ID 錨=fsm-pbo 錨 golden 親證(unmap_ce=:592/:594、
//!   unmap_srst=:612/:614,Mux helper wire=rtlil.cc:3337)。

use ry_ir::Design;

use crate::async2sync::{unmap_ce, unmap_srst};
use crate::help_desc::HELP_DFFUNMAP;
use crate::opt::{cmd_error_pass, module_is_boxed};
use crate::opt_dff::{ff_from_cell, FfInitVals};
use crate::sigmap::SigMap;

/// oracle 進 FF 分派面=is_builtin_ff(async2sync 同表)。
const BUILTIN_FF: [&str; 17] = [
    "$dff", "$dffe", "$adff", "$adffe", "$sdff", "$sdffe", "$sdffce", "$dffsr", "$dffsre",
    "$aldff", "$aldffe", "$dlatch", "$adlatch", "$dlatchsr", "$sr", "$ff", "$anyinit",
];

/// 命令面(§11.3-4)。
pub fn run_dffunmap(
    design: &mut Design,
    args: &[String],
    log: &mut crate::Log,
) -> Result<(), crate::proc::ProcError> {
    log.header("Executing DFFUNMAP pass (unmap clock enable and synchronous reset from FFs).\n");
    // argidx 迴圈(:54-63):僅 -ce-only/-srst-only;已知旗標=Y2 硬錯
    // (雙投同投=Y2 內,首旗標即錯;互斥 log_cmd_error 面=oracle 真實
    // 行為=行為面歧);其餘 token=extra_args 面→cmd_error。
    for a in args.get(1..).unwrap_or(&[]) {
        if matches!(a.as_str(), "-ce-only" | "-srst-only") {
            return Err(crate::proc::ProcError(format!(
                "dffunmap: option `{a}' is not implemented in ryosys (milestone Y2)"
            )));
        }
        break;
    }
    if args.len() > 1 {
        return Err(crate::proc::ProcError(
            cmd_error_pass(log, args, 1, HELP_DFFUNMAP).0,
        ));
    }
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        let mut m = std::mem::take(&mut design.modules[mi]);
        let mut autoidx = design.autoidx;
        let am = {
            let mut am = SigMap::new();
            am.set(&m);
            am
        };
        let mut iv = FfInitVals::set(&am, &m);
        for ci in (0..m.cells.len()).rev() {
            let cell = m.cells[ci].clone();
            if !BUILTIN_FF.contains(&cell.ctype.as_str()) {
                continue;
            }
            if cell.ctype.starts_with("$_") {
                design.autoidx = autoidx;
                design.modules[mi] = m;
                return Err(crate::proc::ProcError(String::from(
                    "dffunmap: fine-cell ($_DFF_ family) form is not implemented in ryosys (milestone Y2)",
                )));
            }
            let mut ff = ff_from_cell(&cell, &iv, &am);
            if !ff.has_clk {
                continue;
            }
            // 無旗標形=unmap_ce_srst(:70-76):unmap_ce();unmap_srst();
            // (有 ce/srst 才不 continue;branch 內 continue 已回)。
            if !ff.has_ce && !ff.has_srst {
                continue;
            }
            unmap_ce(&mut ff, &mut m, &mut autoidx);
            unmap_srst(&mut ff, &mut m, &mut autoidx);
            ff.emit(&mut m, &mut iv, &am);
        }
        design.autoidx = autoidx;
        design.modules[mi] = m;
    }
    Ok(())
}
