//! Elaboration error surface (design §5-5): every unsupported construct is
//! a hard error with a milestone pointer (AGENTS §3), classified into
//! exit-2 (compile class) and exit-1 (the iverilog `sorry` shape, used for
//! net delays per design §5-5).

pub struct ElabError {
    pub message: String,
    pub exit: i32,
}

impl std::fmt::Display for ElabError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub type ElabResult<T> = Result<T, ElabError>;

/// `{file}:{line}: {msg}` — the iverilog diagnostic line shape.
pub fn err_at(file: &str, line: u32, msg: impl Into<String>) -> ElabError {
    ElabError {
        message: format!("{file}:{line}: {}", msg.into()),
        exit: 2,
    }
}

