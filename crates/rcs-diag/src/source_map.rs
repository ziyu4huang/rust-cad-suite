//! Registry of source files with line/column lookup.

use crate::span::{BytePos, FileId};

pub struct SourceFile {
    pub name: String,
    pub source: String,
    /// Byte offset of the start of each line (line 0 starts at 0).
    line_starts: Vec<u32>,
}

impl SourceFile {
    fn new(name: String, source: String) -> Self {
        let mut line_starts = vec![0u32];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i as u32 + 1);
            }
        }
        SourceFile {
            name,
            source,
            line_starts,
        }
    }

    /// 1-based (line, column). Column counts bytes, which matches typical
    /// editor behavior for ASCII sources; multi-byte characters are counted
    /// as multiple columns (documented limitation).
    fn line_col(&self, pos: BytePos) -> (u32, u32) {
        let pos = pos.to_usize();
        let line_idx = match self.line_starts.binary_search(&(pos as u32)) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let col = pos - self.line_starts[line_idx] as usize;
        (line_idx as u32 + 1, col as u32 + 1)
    }
}

#[derive(Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        SourceMap { files: Vec::new() }
    }

    /// Register a source file (real path or virtual name such as
    /// `<macro:FOO>` for preprocessor expansion sites).
    pub fn add(&mut self, name: impl Into<String>, source: String) -> FileId {
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceFile::new(name.into(), source));
        id
    }

    pub fn file(&self, id: FileId) -> &SourceFile {
        &self.files[id.0 as usize]
    }

    /// True when `id` names a registered file (default spans don't).
    pub fn has(&self, id: FileId) -> bool {
        (id.0 as usize) < self.files.len()
    }

    pub fn file_name(&self, id: FileId) -> &str {
        &self.file(id).name
    }

    pub fn line_col(&self, id: FileId, pos: BytePos) -> (u32, u32) {
        self.file(id).line_col(pos)
    }

    /// Byte offset where a 1-based line starts.
    pub fn line_start_offset(&self, id: FileId, line: u32) -> usize {
        let f = self.file(id);
        let idx = (line - 1) as usize;
        f.line_starts.get(idx).map(|&s| s as usize).unwrap_or(f.source.len())
    }

    /// The text of a 1-based line, without its trailing newline.
    pub fn line_text(&self, id: FileId, line: u32) -> &str {
        let f = self.file(id);
        let idx = (line - 1) as usize;
        if idx >= f.line_starts.len() {
            return "";
        }
        let start = f.line_starts[idx] as usize;
        let end = f
            .line_starts
            .get(idx + 1)
            .map(|&e| e as usize)
            .unwrap_or(f.source.len());
        f.source[start..end].trim_end_matches('\n').trim_end_matches('\r')
    }
}
