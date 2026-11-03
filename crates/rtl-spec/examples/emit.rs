//! native JSON 發射器(wasm parity 對拍的 native 腿;S4 冒煙用)。
//! 用法:emit <sv|rtlil> <analyze|rules|gaps> <name> <file> [max_rules]
//! stdout=JSON(rtl-spec 公開 API 直出;與 wasm 面 byte 相等的對拍基準)。

use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (lang, mode, name, file, max) = match args.as_slice() {
        [l, m, n, f] => (l.as_str(), m.as_str(), n.as_str(), f.as_str(), 128usize),
        [l, m, n, f, mx] => (
            l.as_str(),
            m.as_str(),
            n.as_str(),
            f.as_str(),
            mx.parse().unwrap_or(128),
        ),
        _ => {
            eprintln!("usage: emit <sv|rtlil> <analyze|rules|gaps> <name> <file> [max_rules]");
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
        _ => {
            eprintln!("unknown mode {mode} (analyze|rules|gaps)");
            std::process::exit(2);
        }
    };
    println!("{out}");
}
