//! Model source tools: models as ASCII (the native decompiler), what an
//! ASCII model's text holds (its outline) and what is wrong with it
//! (diagnostics), the keywords the game knows, and the external compile and
//! decompile back ends.

pub mod keywords;
pub mod lint;
pub mod outline;
pub mod tools;
pub mod write;

use mg_mdl::{MdlError, Model};
use thiserror::Error;

pub use lint::{Diagnostic, Severity};
pub use outline::{Outline, outline};
pub use write::{Options, to_ascii};

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
    Ok(to_ascii(&model, &Options::default()))
}
