//! native JSON 發射器(wasm parity 對拍的 native 腿;S4 冒煙用)。
//! 用法:emit <sv|rtlil> <analyze|rules|gaps> <name> <file> [max_rules]
//!       emit <sv|rtlil> tbplan <name> <file> [module] [seed] [cycles] [flush]
//! stdout=JSON(rtl-spec 公開 API 直出;與 wasm 面 byte 相等的對拍基準)。

use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // 缺省上限=核心 SSOT rtl_spec::DEFAULT_MAX_RULES(D-r1a/§19 RV2-10:禁複製數字常量)。
    let (lang, mode, name, file, max) = match args.as_slice() {
        // tbplan 8 參形(<sv|rtlil> tbplan <name> <file> [module] [seed] [cycles] [flush])。
        [l, m, n, f, ..] if *m == "tbplan" => (l.as_str(), m.as_str(), n.as_str(), f.as_str(), 0),
        [l, m, n, f] => (
            l.as_str(),
            m.as_str(),
            n.as_str(),
            f.as_str(),
            rtl_spec::DEFAULT_MAX_RULES,
        ),
        [l, m, n, f, mx] => (
            l.as_str(),
            m.as_str(),
            n.as_str(),
            f.as_str(),
            mx.parse().unwrap_or(rtl_spec::DEFAULT_MAX_RULES),
        ),
        _ => {
            eprintln!("usage: emit <sv|rtlil> <analyze|rules|gaps> <name> <file> [max_rules]");
            eprintln!("       emit <sv|rtlil> tbplan <name> <file> [module] [seed] [cycles] [flush]");
            std::process::exit(2);
        }
    };
    let source = match fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {file}: {e}");
            std::process::exit(2);
        }
    };
    // tbplan 模式:module/seed/cycles/flush 缺省=核心默認(空串/0;
    // 空串 module=None,0=核心默認——與 wasm 面同一委託契約)。
    let (module, seed, cycles, flush) = match mode {
        "tbplan" => {
            let g = |k: usize| args.get(k).cloned().unwrap_or_default();
            (
                {
                    let m = g(4);
                    if m.is_empty() { None } else { Some(m) }
                },
                g(5).parse().unwrap_or(0),
                g(6).parse().unwrap_or(0),
                g(7).parse().unwrap_or(0),
            )
        }
        _ => (None, 0, 0, 0),
    };
    let input = match lang {
        "sv" => rtl_spec::SourceInput::Sv(rtl_spec::SvInput {
            name: name.to_string(),
            source,
            ..Default::default()
        }),
        "rtlil" => rtl_spec::SourceInput::Rtlil(rtl_spec::RtlilInput {
            name: name.to_string(),
            source,
        }),
        _ => {
            eprintln!("unknown lang {lang} (sv|rtlil)");
            std::process::exit(2);
        }
    };
    let out = match mode {
        "analyze" => rtl_spec::analysis_json(&rtl_spec::analyze_summary(&input)),
        "rules" => {
            let rb = rtl_spec::extract_rules(&input, max);
            rtl_spec::rulebook_json(&rb)
        }
        "gaps" => rtl_spec::gap_report_json(&rtl_spec::scan_gaps(&input)),
        "tbplan" => rtl_spec::tb_plan_frame(&input, module.as_deref(), seed, cycles, flush),
        _ => {
            eprintln!("unknown mode {mode} (analyze|rules|gaps|tbplan)");
            std::process::exit(2);
        }
    };
    println!("{out}");
}
