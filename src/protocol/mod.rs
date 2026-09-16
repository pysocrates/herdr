//! Shared wire protocol and presentation encoding code.

pub mod endpoint;
pub(crate) mod pane_dock;
pub(crate) mod persistent_area;
pub(crate) mod render_ansi;
pub(crate) mod surface_delta;
pub(crate) mod surface_reuse;
mod wire;

pub use wire::*;
