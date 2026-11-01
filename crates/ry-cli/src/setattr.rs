//! setattr(passes/cmds/setattr.cc 同構移植;RYOSYS-MVP §11.3-5)。
//!
//! * argidx 迴圈(:83-97):`-set name value`(值語法=雙引號剝引號字串/
//!   Verilog 常量整數;失敗=`ERROR: Can't decode value '<v>'!` rc=1,
//!   setunset_t ctor 面=log_cmd_error)/`-unset name`/`-mod` 布爾,可重複;
//!   其餘 token=extra_args 面→cmd_error(selection 裸字=D-ys11)。
//! * 執行體靜默(**零 log_header 不占編號**,sa1.log 親證);-mod 面=
//!   module->attributes(is_selected_whole 全選=真);成員面=selected_members
//!   (wires+cells+memories;do_setunset 逐物件冪等,遍歷序不影響終態);
//!   遍歷=modules rev(dict 逆插入序)。
//! * 值 tag_bare_integer_const(rtlil.cc:619):純數字串=32-bit 整數形,
//!   printer autoint 面=`attribute \src_sync 1` 形(Q5 golden 親證 12 處)。

use ry_ir::{Bit, Const, Design};

use crate::help_desc::HELP_SETATTR;
use crate::opt::cmd_error_pass;

/// setunset_t(SigSpec::parse;const2ast 子集=十進整數+sized b/o/d/h;
/// 失敗=decode 錯誤)。
fn parse_value(v: &str) -> Result<Const, ()> {
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        return Ok(Const::Str(v[1..v.len() - 1].to_string()));
    }
    // 純十進整數(可帶符號)=32-bit 二補數。
    let (sign, digits) = match v.strip_prefix('-').or_else(|| v.strip_prefix('+')) {
        Some(d) => (v.starts_with('-'), d),
        None => (false, v),
    };
    if !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) {
        let mut n: i64 = 0;
        for c in digits.bytes() {
            n = n
                .checked_mul(10)
                .and_then(|x| x.checked_add((c - b'0') as i64))
                .ok_or(())?;
        }
        let n = if sign { -n } else { n };
        if !(-((i32::MAX) as i64)..=((i32::MAX) as i64)).contains(&n) {
            return Err(());
        }
        return Ok(Const::from_i64_i32_width(n));
    }
    // sized 形 `<w>'[s]<b|d><digits>`(const2ast 子集;_ 分隔符吃掉;
    // o/h 形=語料外→decode 錯誤誠實面,精簡化邊界)。
    let Some(qpos) = v.find('\'') else { return Err(()) };
    let width: i64 = v[..qpos].parse().map_err(|_| ())?;
    if !(0..=1_000_000).contains(&width) {
        return Err(());
    }
    let rest = &v[qpos + 1..];
    let (rest, signed) = match rest.strip_prefix('s') {
        Some(r) => (r, true),
        None => (rest, false),
    };
    let mut chars = rest.chars();
    let base = chars.next().ok_or(())?;
    let digits_str: String = chars.filter(|c| *c != '_').collect();
    let mut bits: Vec<Bit> = match base {
        'b' => digits_str
            .chars()
            .map(|c| match c.to_ascii_lowercase() {
                '0' => Some(Bit::S0),
                '1' => Some(Bit::S1),
                'x' => Some(Bit::Sx),
                'z' => Some(Bit::Sz),
                'm' => Some(Bit::Sm),
                '-' => Some(Bit::Sa),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(())?,
        'd' => {
            let n: i64 = if digits_str.is_empty() {
                return Err(());
            } else {
                digits_str.parse().map_err(|_| ())?
            };
            (0..width).map(|i| if (n >> i) & 1 == 1 { Bit::S1 } else { Bit::S0 }).collect()
        }
        _ => return Err(()),
    };
    bits.reverse();
    if bits.len() as i64 > width {
        bits.truncate(width as usize);
    } else {
        let ext = bits.last().copied().map(|b| if b == Bit::S1 { Bit::S0 } else { b }).unwrap_or(Bit::Sx);
        bits.resize(width.max(0) as usize, ext);
    }
    Ok(Const::Bits { width, bits, signed, is_unsized: false })
}

fn escape_id(name: &str) -> String {
    if name.starts_with('\\') || name.starts_with('$') {
        name.to_string()
    } else {
        format!("\\{name}")
    }
}

/// do_setunset(setattr.cc:49-55)。
fn do_setunset(attrs: &mut ry_ir::InsertMap<Const>, list: &[(String, Option<Const>)]) {
    for (name, val) in list {
        match val {
            None => {
                attrs.swap_pop_remove(name);
            }
            Some(v) => attrs.set(name.clone(), v.clone()),
        }
    }
}

/// 命令面(§11.3-5)。
pub fn run_setattr(
    design: &mut Design,
    args: &[String],
    log: &mut crate::Log,
) -> Result<(), crate::proc::ProcError> {
    let _ = log; // 執行體靜默;cmd_error 面才寫 stdout
    let mut list: Vec<(String, Option<Const>)> = Vec::new();
    let mut flag_mod = false;
    let mut argidx = 1usize;
    while argidx < args.len() {
        let arg = args[argidx].as_str();
        if arg == "-set" && argidx + 2 < args.len() {
            let key = escape_id(&args[argidx + 1]);
            let val = parse_value(&args[argidx + 2]).map_err(|_| {
                crate::proc::ProcError(format!(
                    "Can't decode value '{}'!",
                    args[argidx + 2]
                ))
            })?;
            list.push((key, Some(val)));
            argidx += 3;
            continue;
        }
        if arg == "-unset" && argidx + 1 < args.len() {
            list.push((escape_id(&args[argidx + 1]), None));
            argidx += 2;
            continue;
        }
        if arg == "-mod" {
            flag_mod = true;
            argidx += 1;
            continue;
        }
        break;
    }
    if argidx < args.len() {
        return Err(crate::proc::ProcError(
            cmd_error_pass(log, args, argidx, HELP_SETATTR).0,
        ));
    }
    for mi in (0..design.modules.len()).rev() {
        if flag_mod {
            // is_selected_whole(全選=真;our 無 selection 面)。
            do_setunset(&mut design.modules[mi].attributes, &list);
            continue;
        }
        let mut m = std::mem::take(&mut design.modules[mi]);
        for w in m.wires.iter_mut().rev() {
            do_setunset(&mut w.attributes, &list);
        }
        for mem in m.memories.iter_mut().rev() {
            do_setunset(&mut mem.attributes, &list);
        }
        for c in m.cells.iter_mut().rev() {
            do_setunset(&mut c.attributes, &list);
        }
        design.modules[mi] = m;
    }
    Ok(())
}
