//! In-tree deterministic fuzz smoke (REFLECTION §7 / AGENTS §4): mutate every
//! corpus .v file with a fixed-seed PRNG and require that preprocess+parse
//! neither panics nor hangs. A hang is detected by a per-mutant watchdog
//! thread; the offending mutant is written to `target/fuzz-repro-<seed>.v`.
//! Runs in a few hundred ms — cheap enough for every `cargo test`.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const ALPHABET: &[&str] = &[
    "module", "endmodule", "begin", "end", "always", "initial", "if", "else", "case",
    "endcase", "for", "function", "endfunction", "task", "endtask", "generate",
    "endgenerate", "wire", "reg", "integer", "assign", "@", "#", "(", ")", "[", "]",
    "{", "}", ";", ",", ".", ":", "?", "=", "<=", "==", "===", "!", "~", "&", "|",
    "^", "+", "-", "*", "/", "%", "<<", ">>", "<<<", ">>>", "4'b1x0z", "8'hff",
    "'sd5", "1.5", "$display", "$finish", "\"str\"", "`define", "`ifdef", "`endif",
    "`FOO", "``", "`\"", "posedge", "negedge", "or", "x", "z", "\n", " ", "//c\n",
    "/*", "*/", "\\esc ", "\u{00e9}",
];

fn mutate(src: &str, rng: &mut XorShift) -> String {
    let mut bytes: Vec<u8> = src.as_bytes().to_vec();
    let n_ops = 1 + rng.below(4);
    for _ in 0..n_ops {
        if bytes.is_empty() {
            bytes.extend_from_slice(b"module m; endmodule");
        }
        match rng.below(6) {
            0 => {
                let i = rng.below(bytes.len());
                bytes[i] = ALPHABET[rng.below(ALPHABET.len())].as_bytes()[0];
            }
            1 => {
                let i = rng.below(bytes.len());
                bytes.remove(i);
            }
            2 => {
                let i = rng.below(bytes.len() + 1);
                let tok = ALPHABET[rng.below(ALPHABET.len())];
                for (k, b) in tok.bytes().enumerate() {
                    bytes.insert(i + k, b);
                }
            }
            3 => {
                let i = rng.below(bytes.len() + 1);
                bytes.truncate(i);
            }
            4 => {
                // duplicate a random line
                let s = String::from_utf8_lossy(&bytes).to_string();
                let lines: Vec<&str> = s.lines().collect();
                if !lines.is_empty() {
                    let i = rng.below(lines.len());
                    let mut out = String::new();
                    for (k, l) in lines.iter().enumerate() {
                        out.push_str(l);
                        out.push('\n');
                        if k == i {
                            out.push_str(l);
                            out.push('\n');
                        }
                    }
                    bytes = out.into_bytes();
                }
            }
            _ => {
                // swap two random byte ranges
                if bytes.len() > 4 {
                    let a = rng.below(bytes.len());
                    let b = rng.below(bytes.len());
                    bytes.swap(a, b);
                }
            }
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

fn corpus_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let mut out = Vec::new();
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().map(|x| x == "v" || x == "sv").unwrap_or(false) {
                    out.push(p);
                }
            }
        }
    }
    walk(&root, &mut out);
    out.sort();
    out
}

fn frontend_once(name: String, src: String) {
    let mut sm = rcs_diag::SourceMap::new();
    let mut diags = rcs_diag::DiagSink::new();
    let opts = vl_syntax::pp::PpOptions::default();
    let out = vl_syntax::pp::preprocess_source(&name, &src, &mut sm, &mut diags, &opts);
    let _ = vl_syntax::parser::parse_source_file(out.tokens, &mut diags);
    // Rendering must also be total (default spans etc.).
    let mut rendered = String::new();
    rcs_diag::render_diags(&mut rendered, &sm, &diags.diags);
}

/// Runs one mutant under a watchdog; returns Err(reason) on panic/hang.
fn run_guarded(name: String, src: String, timeout: Duration) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let src2 = src.clone();
    std::thread::spawn(move || {
        let r = std::panic::catch_unwind(move || frontend_once(name, src2));
        let _ = tx.send(r.is_ok());
    });
    match rx.recv_timeout(timeout) {
        Ok(true) => Ok(()),
        Ok(false) => Err("panic".into()),
        Err(_) => Err("hang (watchdog)".into()),
    }
}

#[test]
fn fuzz_frontend_no_panic_no_hang() {
    let files = corpus_files();
    assert!(files.len() >= 20, "corpus too small: {}", files.len());
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let mut failures = Vec::new();
    let iterations = 400;
    for i in 0..iterations {
        let f = &files[rng.below(files.len())];
        let base = std::fs::read_to_string(f).unwrap();
        let seed = rng.next();
        let mut local = XorShift(seed);
        let mutant = mutate(&base, &mut local);
        if let Err(why) = run_guarded(format!("fuzz{i}.v"), mutant.clone(), Duration::from_secs(5)) {
            let repro = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../target/fuzz-repro-{seed:x}.v"));
            let _ = std::fs::write(&repro, &mutant);
            failures.push(format!("{why} seed={seed:x} base={} repro={}", f.display(), repro.display()));
            if failures.len() >= 5 {
                break;
            }
        }
    }
    assert!(failures.is_empty(), "fuzz failures:\n{}", failures.join("\n"));
}

#[test]
fn fuzz_lexer_random_bytes() {
    let mut rng = XorShift(0xDEAD_BEEF_CAFE_F00D);
    for i in 0..200 {
        let len = rng.below(256);
        let bytes: Vec<u8> = (0..len).map(|_| (rng.next() & 0xff) as u8).collect();
        let src = String::from_utf8_lossy(&bytes).to_string();
        let r = run_guarded(format!("rand{i}.v"), src.clone(), Duration::from_secs(5));
        if let Err(why) = r {
            let repro = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../target/fuzz-lex-{i}.txt"));
            let _ = std::fs::write(&repro, &src);
            panic!("random-bytes lexing failed: {why:?} (iter {i}, input saved to {})", repro.display());
        }
    }
}
