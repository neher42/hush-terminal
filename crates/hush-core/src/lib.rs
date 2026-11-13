//! Hush's shell sessions and everything else that isn't UI.

pub mod claude;
pub mod error;
pub mod events;
pub mod executables;
pub mod files;
mod fsutil;
pub mod git;
pub mod history;
pub mod pty;
pub mod settings;
pub mod shell;

pub use error::{Error, Result};
