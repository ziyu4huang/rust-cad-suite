//! Simulator core (Phase 1). v0 scope = the M2 vertical slice: 4-state
//! values, expression evaluation, combinational-assign convergence, and
//! `initial`-block execution with `$display`/`$finish`. Timed scheduling
//! (`always`, `#delay` retriggering, NBA regions) lands at M4/M5 per PLAN §5.

pub mod backend;
pub mod eval;
pub mod interp;
pub mod value;
pub mod vpi;

pub use eval::{Val, SimError};
pub use backend::USim;
pub use interp::{RunEnd, Sim};
