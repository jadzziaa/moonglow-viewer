//! The external compile and decompile back ends (nwnmdlcomp and the game's
//! own compiler). The native compiler, the writers, the diagnostics and the
//! outline are the toolset's (`mg_mdl::compile`, `binary_write`,
//! `ascii_write`, `lint`, `outline`), where they moved from here.

pub mod tools;
