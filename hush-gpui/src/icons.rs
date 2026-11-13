use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

/// Line icons, drawn on a 16x16 grid (same set as the web version).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Copy,
    Command,
    Rerun,
    Send,
    Folder,
    File,
    Branch,
    Prompt,
    History,
    Flag,
}

impl Icon {
    const ALL: [Icon; 10] = [
        Icon::Copy,
        Icon::Command,
        Icon::Rerun,
        Icon::Send,
        Icon::Folder,
        Icon::File,
        Icon::Branch,
        Icon::Prompt,
        Icon::History,
        Icon::Flag,
    ];

    pub fn path(self) -> &'static str {
        match self {
            Icon::Copy => "icons/copy.svg",
            Icon::Command => "icons/command.svg",
            Icon::Rerun => "icons/rerun.svg",
            Icon::Send => "icons/send.svg",
            Icon::Folder => "icons/folder.svg",
            Icon::File => "icons/file.svg",
            Icon::Branch => "icons/branch.svg",
            Icon::Prompt => "icons/prompt.svg",
            Icon::History => "icons/history.svg",
            Icon::Flag => "icons/flag.svg",
        }
    }

    fn shapes(self) -> &'static str {
        match self {
            Icon::Copy => {
                r#"<rect x="5.5" y="5.5" width="8" height="8" rx="1.6"/><path d="M10.5 3.5v-.3a.9.9 0 0 0-.9-.9H3.2a.9.9 0 0 0-.9.9v6.4c0 .5.4.9.9.9h.3"/>"#
            }
            Icon::Command => r#"<path d="M3 4.5 6.5 8 3 11.5M8.5 12H13"/>"#,
            Icon::Rerun => r#"<path d="M13 8a5 5 0 1 1-1.5-3.6"/><path d="M13 2.5v3h-3"/>"#,
            Icon::Send => r#"<path d="M8 13V3.5M4 7.5 8 3.5l4 4"/>"#,
            Icon::Folder => {
                r#"<path d="M2 4.6c0-.6.5-1.1 1.1-1.1h2.8l1.5 1.6h5.5c.6 0 1.1.5 1.1 1.1v5.7c0 .6-.5 1.1-1.1 1.1H3.1c-.6 0-1.1-.5-1.1-1.1z"/>"#
            }
            Icon::File => r#"<path d="M4 2h5l3 3v9H4z"/><path d="M9 2v3h3"/>"#,
            Icon::Branch => {
                r#"<circle cx="5" cy="3.8" r="1.5"/><circle cx="5" cy="12.2" r="1.5"/><circle cx="11" cy="5.5" r="1.5"/><path d="M5 5.3v5.4M11 7c0 2.6-6 1.4-6 3.6"/>"#
            }
            Icon::Prompt => r#"<path d="M4.5 4.5 8 8l-3.5 3.5"/>"#,
            Icon::History => r#"<circle cx="8" cy="8" r="5.5"/><path d="M8 5v3.2l2 1.3"/>"#,
            Icon::Flag => r#"<path d="M4.5 8h7"/>"#,
        }
    }

    fn svg(self) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="black" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#,
            self.shapes()
        )
    }
}

/// Serves the icons to gpui's `svg()` element, which tints them with the text color.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(Icon::ALL
            .into_iter()
            .find(|icon| icon.path() == path)
            .map(|icon| Cow::Owned(icon.svg().into_bytes())))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Icon::ALL
            .into_iter()
            .map(Icon::path)
            .filter(|p| p.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}
