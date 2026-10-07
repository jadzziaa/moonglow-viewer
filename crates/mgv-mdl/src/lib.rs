//! Model source tools: models as ASCII (the native decompiler), what an
//! ASCII model's text holds (its outline) and what is wrong with it
//! (diagnostics), the keywords the game knows, compiled models written from
//! a model (the native compiler's first stage), and the external compile
//! and decompile back ends.

pub mod binary;
pub mod keywords;
pub mod lint;
pub mod outline;
pub mod tools;
pub mod write;

use mg_mdl::{MdlError, Model};
use thiserror::Error;

pub use lint::{Diagnostic, Severity};
pub use outline::{Outline, outline};
pub use write::{Options, file_order, to_ascii, to_ascii_in_order};

#[derive(Debug, Error)]
pub enum DecompileError {
    #[error("not a compiled model (it is already ASCII)")]
    NotBinary,
    #[error(transparent)]
    Read(#[from] MdlError),
}

/// Decompiles a binary model to ASCII in process.
pub fn decompile(data: &[u8]) -> Result<String, DecompileError> {
    if !mg_mdl::is_binary(data) {
        return Err(DecompileError::NotBinary);
    }
    let model = Model::read(data)?;
    // In the order of its part numbers, so compiling it again keeps them.
    let order = match binary::PartNumbers::read(data) {
        Some(parts) => write::file_order(&model, &parts.numbers),
        None => (0..model.nodes.len()).collect(),
    };
    Ok(write::to_ascii_in_order(&model, &Options::default(), &order))
}
