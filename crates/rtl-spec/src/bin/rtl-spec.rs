//! rtl-spec bin — 薄 main(vl-cli 同款;邏輯全在 rtl_spec::cli,D-S2)。

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(rtl_spec::cli::run(&args));
}
