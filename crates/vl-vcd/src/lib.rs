//! vl-vcd — the single VCD core behind every rverilator facade.
//!
//! One crate, two halves:
//! - [`writer`]: IEEE 1364-2005 §18 value-change dump writer (moved from
//!   vl-sim at the vl-vcd split). Values cross the API as pre-rendered
//!   4-state bit strings (`"0101xz"`), which is bijective with the
//!   simulator's internal `Bits` per width — so output bytes are identical
//!   to the pre-split writer (proven by the e2e `expect.vcd` diffs).
//! - [`reader`]: tolerant VCD reader (GTKWave-informed subset, see
//!   docs/GTKWAVE-VCD-STUDY.md for the adopted/rejected table).
//!
//! Consumers: vl-sim (writer), vl-difftest (reader crosscheck),
//! vl-wasm (both, via wasm-bindgen facade), vl-ffi (both, via C ABI).

pub mod reader;
pub mod writer;

pub use reader::{parse, normalize_width, VcdChange, VcdDiag, VcdFile, VcdValue, VcdVar};
pub use writer::VcdWriter;
