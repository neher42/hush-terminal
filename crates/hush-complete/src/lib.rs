//! Tab completion, command history and path helpers for Hush's prompt.
//!
//! Text positions are byte offsets into UTF-8 strings and always fall on char boundaries.

mod catalog;
mod complete;
mod context;
mod history;
pub mod paths;
mod sources;
pub mod specs;
mod suggestions;

#[cfg(test)]
mod fake;

pub use catalog::CommandCatalog;
pub use complete::{common_prefix, complete, CompletionItem, CompletionKind};
pub use context::{completion_context, CompletionContext};
pub use history::{HistoryBrowser, HistoryStore, SEARCH_LIMIT};
pub use sources::{CompletionSources, DirEntry, Place, SystemSources};
pub use suggestions::{cd_target, fits_here, FolderCheck};
