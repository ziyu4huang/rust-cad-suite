//! Byte-offset based source positions and spans.

/// Identifies a source file registered in a [`SourceMap`](crate::SourceMap).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default)]
pub struct FileId(pub u32);

/// A byte offset into a source file.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default)]
pub struct BytePos(pub u32);

impl BytePos {
    pub fn to_usize(self) -> usize {
        self.0 as usize
    }
}

/// A half-open byte range `[start, end)` inside one source file.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct Span {
    pub file: FileId,
    pub start: BytePos,
    pub end: BytePos,
}

impl Span {
    pub fn new(file: FileId, start: u32, end: u32) -> Self {
        Span {
            file,
            start: BytePos(start),
            end: BytePos(end),
        }
    }

    pub fn point(file: FileId, pos: u32) -> Self {
        Span::new(file, pos, pos)
    }

    /// Union of two spans (assumed same file; if not, returns `self`).
    pub fn merge(self, other: Span) -> Span {
        if self.file != other.file {
            return self;
        }
        Span {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}
