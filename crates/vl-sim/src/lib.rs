//! Simulator core (Phase 1). v0 scope = the M2 vertical slice: 4-state
//! values, expression evaluation, combinational-assign convergence, and
//! `initial`-block execution with `$display`/`$finish`. Timed scheduling
//! (`always`, `#delay` retriggering, NBA regions) lands at M4/M5 per PLAN §5.

pub mod eval;
pub mod interp;
pub mod value;

pub use eval::{Val, SimError};
pub use interp::Sim;
