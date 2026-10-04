//! tsrslint: a Rust port of tsgolint (oxc-project/tsgolint, MIT) that runs type-aware lint rules on
//! top of tsrs, the Rust port of the TypeScript 7 checker. It speaks tsgolint's headless protocol, so
//! oxlint can use it as a drop-in `tsgolint` binary through `OXLINT_TSGOLINT_PATH`.

pub mod linter;
mod memstats;
pub mod overlayfs;
pub mod protocol;
pub mod rule;
pub mod rules;
mod sched;
pub mod tsconfig;
pub mod utils;
