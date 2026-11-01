//! edgetypes(passes/cmds/edgetypes.cc 同構移植;RYOSYS-MVP §11.3-7)。
//!
//! * edge 四元組字串化=**cell type 形**(:94-100 unescape;`<src_type>.
//!   <src_port> <sink_type>.<sink_port>`);multibit 分支移植(:62/:71-72
//!   GetSize(sig)>1 收集+:94-100 消費,`type.port[index]` 帶 bit 下標形;
//!   carryadd 錨全 1-bit 零觸發,multibit 手寫小錨=§11.4 slice-7 彌合)。
//! * pool 去重+`edge_cache.sort()` 字串序正序 log;**無 log_header 不占
//!   編號**(零旗標,argidx 迴圈全註解=break 即出);收集序(hash dict/
//!   pool)完全不出現在輸出面=零序風險(Q7);參數面=cmd_error
//!   (selection 裸字=D-ys11)。

use std::collections::HashMap;

use ry_ir::{Design, Module, SigBit};

use crate::help_desc::HELP_EDGETYPES;
use crate::opt::{cmd_error_pass, module_is_boxed};
use crate::sigmap::SigMap;

/// 命令面(§11.3-7)。
pub fn run_edgetypes(
    design: &mut Design,
    args: &[String],
    log: &mut crate::Log,
) -> Result<(), crate::proc::ProcError> {
    // 零旗標(:46-52 選項迴圈全註解+break);任何參數=extra_args 面
    // →cmd_error(未知旗標=oracle rc=1 一致;selection 裸字=D-ys11)。
    if args.len() > 1 {
        return Err(crate::proc::ProcError(
            cmd_error_pass(log, args, 1, HELP_EDGETYPES).0,
        ));
    }
    let mut edge_cache: Vec<String> = Vec::new();
    for mi in (0..design.modules.len()).rev() {
        if module_is_boxed(&design.modules[mi]) {
            continue;
        }
        let m: &Module = &design.modules[mi];
        let am = {
            let mut am = SigMap::new();
            am.set(m);
            am
        };
        let mut bit_sources: HashMap<SigBit, Vec<(String, String, i64)>> = HashMap::new();
        let mut bit_sinks: HashMap<SigBit, Vec<(String, String, i64)>> = HashMap::new();
        let mut multibit_ports: Vec<(String, String)> = Vec::new();
        for cell in m.cells.iter().rev() {
            for (port, sig) in cell.connections.iter().rev() {
                let cell_type = cell.ctype.clone();
                let mapped = am.map(sig);
                if mapped.bits.len() > 1 {
                    let key = (cell_type.clone(), port.clone());
                    if !multibit_ports.contains(&key) {
                        multibit_ports.push(key);
                    }
                }
                let (is_in, is_out) = crate::cells::port_dir(&cell.ctype, port);
                for (i, b) in mapped.bits.iter().enumerate() {
                    if is_out {
                        let v = bit_sources.entry(b.clone()).or_default();
                        let item = (cell_type.clone(), port.clone(), i as i64);
                        if !v.contains(&item) {
                            v.push(item);
                        }
                    }
                    if is_in {
                        let v = bit_sinks.entry(b.clone()).or_default();
                        let item = (cell_type.clone(), port.clone(), i as i64);
                        if !v.contains(&item) {
                            v.push(item);
                        }
                    }
                }
            }
        }
        let port_str = |types: &(String, String), idx: i64| -> String {
            let (ty, port) = types;
            if multibit_ports.contains(&(ty.clone(), port.clone())) {
                format!(
                    "{}.{}[{}]",
                    crate::jsonout::unescape(ty),
                    crate::jsonout::unescape(port),
                    idx
                )
            } else {
                format!("{}.{}", crate::jsonout::unescape(ty), crate::jsonout::unescape(port))
            }
        };
        for (bit, sources) in &bit_sources {
            let Some(sinks) = bit_sinks.get(bit) else { continue };
            for src in sources {
                for snk in sinks {
                    let s = format!("{} {}", port_str(&(src.0.clone(), src.1.clone()), src.2), port_str(&(snk.0.clone(), snk.1.clone()), snk.2));
                    if !edge_cache.contains(&s) {
                        edge_cache.push(s);
                    }
                }
            }
        }
    }
    edge_cache.sort();
    for s in &edge_cache {
        log.log(&format!("{s}\n"));
    }
    Ok(())
}
