//! The folder sidebar: current folder, pinned folders, the tree around the
//! current folder, and the git status of the repository it's in.

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use gpui::{
    anchored, deferred, div, prelude::*, px, rgb, svg, uniform_list, AnyElement, App, ClickEvent,
    Context, EventEmitter, Font, MouseButton, MouseDownEvent, Pixels, Point, ScrollStrategy,
    SharedString, Task, UniformListScrollHandle, Window,
};
use hush_complete::paths::{base_name, display_path, is_inside, parent_path};
use hush_core::files::list_dir;
use hush_core::git::{self, GitStatus};

use crate::colors::Palette;
use crate::icons::Icon;
use crate::tree::{Row, Tree};
use crate::ui::{self, UI_FONT};

pub const WIDTH: f32 = 240.;
const ROW_HEIGHT: f32 = 24.;
/// Characters that fit next to the icons in a top-level tree row and in the git line.
const TREE_CHARS: usize = 24;
const GIT_CHARS: usize = 25;
/// Delay before a single click on a folder acts, so a double-click can cancel it.
const CLICK_DELAY: Duration = Duration::from_millis(220);

/// Something to do with a path, from a click or a context menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathCommand {
    Insert { path: String, is_dir: bool },
    Open(String),
    OpenInNewTab(String),
    Copy(String),
    TogglePin(String),
    Reveal(String),
}

#[derive(Clone)]
pub enum MenuEntry {
    Item {
        label: &'static str,
        command: PathCommand,
    },
    Separator,
}

#[derive(Clone)]
pub struct ContextMenu {
    pub position: Point<Pixels>,
    pub entries: Vec<MenuEntry>,
}

pub enum SidebarEvent {
    Command(PathCommand),
    Menu(ContextMenu),
    Notify(SharedString),
}

pub struct Sidebar {
    tree: Tree,
    rows: Vec<Row>,
    loading: HashSet<String>,
    center_pending: bool,
    pinned: Vec<String>,
    missing: HashSet<String>,
    git: Option<GitStatus>,
    show_git: bool,
    palette: &'static Palette,
    font: Font,
    scroll: UniformListScrollHandle,
    pending_click: Option<Task<()>>,
    git_task: Option<Task<()>>,
    pins_task: Option<Task<()>>,
}

impl EventEmitter<SidebarEvent> for Sidebar {}

impl Sidebar {
    pub fn new(
        home: &str,
        palette: &'static Palette,
        font: Font,
        show_git: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut sidebar = Self {
            tree: Tree::new(home, home),
            rows: Vec::new(),
            loading: HashSet::new(),
            center_pending: true,
            pinned: Vec::new(),
            missing: HashSet::new(),
            git: None,
            show_git,
            palette,
            font,
            scroll: UniformListScrollHandle::new(),
            pending_click: None,
            git_task: None,
            pins_task: None,
        };
        sidebar.update_rows(cx);
        sidebar.refresh_git(cx);
        sidebar
    }

    pub fn set_cwd(&mut self, cwd: &str, cx: &mut Context<Self>) {
        if cwd == self.tree.cwd {
            return;
        }
        self.tree.set_cwd(cwd);
        self.center_pending = true;
        self.update_rows(cx);
        self.refresh_git(cx);
    }

    pub fn set_pinned(&mut self, pinned: &[String], cx: &mut Context<Self>) {
        if pinned != self.pinned {
            self.pinned = pinned.to_vec();
            self.check_pins(cx);
            cx.notify();
        }
    }

    pub fn set_show_git(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_git = show;
        self.refresh_git(cx);
    }

    pub fn set_look(&mut self, palette: &'static Palette, font: Font, cx: &mut Context<Self>) {
        self.palette = palette;
        self.font = font;
        cx.notify();
    }

    /// After a command ran: folders, pins and the repository may have changed.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let paths = self.tree.open_listings();
        let loads = cx.background_executor().spawn(async move {
            paths
                .into_iter()
                .map(|path| {
                    let entries = list_dir(Path::new(&path)).unwrap_or_default();
                    (path, entries)
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let loaded = loads.await;
            let _ = this.update(cx, |sidebar, cx| {
                for (path, entries) in loaded {
                    sidebar.tree.set_listing(path, entries);
                }
                sidebar.update_rows(cx);
            });
        })
        .detach();
        self.check_pins(cx);
        self.refresh_git(cx);
    }

    /// Recomputes the rows and loads folders that are open but not listed yet.
    fn update_rows(&mut self, cx: &mut Context<Self>) {
        let (rows, missing) = self.tree.rows();
        self.rows = rows;
        for path in missing {
            if !self.loading.insert(path.clone()) {
                continue;
            }
            let listing = cx.background_executor().spawn({
                let path = path.clone();
                async move { list_dir(Path::new(&path)).unwrap_or_default() }
            });
            cx.spawn(async move |this, cx| {
                let entries = listing.await;
                let _ = this.update(cx, |sidebar, cx| {
                    sidebar.loading.remove(&path);
                    sidebar.tree.set_listing(path, entries);
                    sidebar.update_rows(cx);
                });
            })
            .detach();
        }
        if self.center_pending && self.loading.is_empty() {
            self.center_pending = false;
            if let Some(index) = self.rows.iter().position(|row| row.path == self.tree.cwd) {
                self.scroll.scroll_to_item(index, ScrollStrategy::Center);
            }
        }
        cx.notify();
    }

    fn refresh_git(&mut self, cx: &mut Context<Self>) {
        if !self.show_git {
            self.git = None;
            self.git_task = None;
            cx.notify();
            return;
        }
        let cwd = self.tree.cwd.clone();
        let status = cx
            .background_executor()
            .spawn(async move { git::status(Path::new(&cwd)) });
        self.git_task = Some(cx.spawn(async move |this, cx| {
            let status = status.await;
            let _ = this.update(cx, |sidebar, cx| {
                sidebar.git = status;
                cx.notify();
            });
        }));
    }

    fn check_pins(&mut self, cx: &mut Context<Self>) {
        let paths = self.pinned.clone();
        let missing = cx.background_executor().spawn(async move {
            paths
                .into_iter()
                .filter(|path| !Path::new(path).is_dir())
                .collect::<HashSet<_>>()
        });
        self.pins_task = Some(cx.spawn(async move |this, cx| {
            let missing = missing.await;
            let _ = this.update(cx, |sidebar, cx| {
                sidebar.missing = missing;
                cx.notify();
            });
        }));
    }

    fn command(&mut self, command: PathCommand, cx: &mut Context<Self>) {
        cx.emit(SidebarEvent::Command(command));
    }

    fn click_row(&mut self, row: &Row, event: &ClickEvent, cx: &mut Context<Self>) {
        let insert = PathCommand::Insert {
            path: row.path.clone(),
            is_dir: row.is_dir,
        };
        if !row.is_dir {
            self.command(insert, cx);
            return;
        }
        if event.click_count() >= 2 {
            self.pending_click = None;
            self.command(PathCommand::Open(row.path.clone()), cx);
            return;
        }
        self.pending_click = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CLICK_DELAY).await;
            let _ = this.update(cx, |sidebar, cx| sidebar.command(insert, cx));
        }));
    }

    fn row_menu(&self, row: &Row, position: Point<Pixels>) -> ContextMenu {
        let path = row.path.clone();
        let mut entries = vec![
            MenuEntry::Item {
                label: "Copy Path",
                command: PathCommand::Copy(path.clone()),
            },
            MenuEntry::Item {
                label: "Insert Path",
                command: PathCommand::Insert {
                    path: path.clone(),
                    is_dir: row.is_dir,
                },
            },
        ];
        if row.is_dir {
            entries.push(MenuEntry::Item {
                label: "Open Here",
                command: PathCommand::Open(path.clone()),
            });
            entries.push(MenuEntry::Item {
                label: if self.pinned.contains(&path) {
                    "Unpin"
                } else {
                    "Pin"
                },
                command: PathCommand::TogglePin(path.clone()),
            });
        }
        entries.push(MenuEntry::Separator);
        entries.push(MenuEntry::Item {
            label: "Reveal in Finder",
            command: PathCommand::Reveal(path),
        });
        ContextMenu { position, entries }
    }

    fn pin_menu(&self, path: &str, position: Point<Pixels>) -> ContextMenu {
        let item = |label, command| MenuEntry::Item { label, command };
        let path = path.to_owned();
        ContextMenu {
            position,
            entries: vec![
                item("Open Here", PathCommand::Open(path.clone())),
                item("Open in New Tab", PathCommand::OpenInNewTab(path.clone())),
                MenuEntry::Separator,
                item("Copy Path", PathCommand::Copy(path.clone())),
                item("Reveal in Finder", PathCommand::Reveal(path.clone())),
                MenuEntry::Separator,
                item("Unpin", PathCommand::TogglePin(path)),
            ],
        }
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let cwd = self.tree.cwd.clone();
        let pinned = self.pinned.contains(&cwd);
        let shown = display_path(&cwd, &self.tree.home);
        div()
            .flex()
            .flex_col()
            .gap(px(7.))
            .px(px(10.))
            .pt(px(12.))
            .pb(px(10.))
            .border_b_1()
            .border_color(rgb(p.line))
            .child(ui::section_label("Current directory", p).pl(px(2.)))
            .child(
                div()
                    .id("cwd")
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .py(px(3.))
                    .pl(px(8.))
                    .pr(px(3.))
                    .border_1()
                    .border_color(rgb(p.line))
                    .rounded(px(7.))
                    .bg(rgb(p.background))
                    .font(self.font.clone())
                    .text_size(px(12.))
                    .text_color(rgb(p.foreground))
                    .child(icon(Icon::Folder, p.muted))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(truncate_start(&shown, 16)),
                    )
                    .tooltip(ui::tooltip(cwd.clone(), p))
                    .child(
                        ui::icon_button("pin-cwd", Icon::Pin, pinned, p)
                            .tooltip(ui::tooltip(
                                if pinned {
                                    "Unpin this folder"
                                } else {
                                    "Pin this folder"
                                },
                                p,
                            ))
                            .on_click(cx.listener({
                                let cwd = cwd.clone();
                                move |sidebar, _, _, cx| {
                                    sidebar.command(PathCommand::TogglePin(cwd.clone()), cx)
                                }
                            })),
                    )
                    .child(
                        ui::icon_button("copy-cwd", Icon::Copy, false, p)
                            .tooltip(ui::tooltip("Copy path  ⌘⇧C", p))
                            .on_click(cx.listener(move |sidebar, _, _, cx| {
                                sidebar.command(PathCommand::Copy(cwd.clone()), cx)
                            })),
                    ),
            )
    }

    fn render_pins(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.pinned.is_empty() {
            return None;
        }
        let p = self.palette;
        let home = &self.tree.home;
        let rows = self.pinned.iter().enumerate().map(|(i, path)| {
            let current = *path == self.tree.cwd;
            let missing = self.missing.contains(path);
            let name = if path == home {
                "~".to_owned()
            } else {
                base_name(path).to_owned()
            };
            let where_ = if path == home || path == "/" {
                String::new()
            } else {
                display_path(parent_path(path), home)
            };
            let color = if current {
                p.current_row_text
            } else if missing {
                p.muted
            } else {
                p.foreground
            };
            let open = path.clone();
            let menu_path = path.clone();
            div()
                .id(("pin", i))
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(26.))
                .pl(px(6.))
                .pr(px(8.))
                .rounded(px(5.))
                .text_color(rgb(color))
                .when(current, |row| {
                    row.bg(rgb(p.current_row))
                        .font_weight(gpui::FontWeight::BOLD)
                })
                .when(!current, |row| row.hover(|s| s.bg(rgb(p.line))))
                .when(missing, |row| row.line_through())
                .tooltip(ui::tooltip(
                    if missing {
                        format!("{path} no longer exists")
                    } else {
                        path.clone()
                    },
                    p,
                ))
                .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                .on_click(cx.listener(move |sidebar, _, _, cx| {
                    if sidebar.missing.contains(&open) {
                        let shown = display_path(&open, &sidebar.tree.home);
                        cx.emit(SidebarEvent::Notify(
                            format!("{shown} no longer exists. Right-click to unpin it.").into(),
                        ));
                    } else {
                        sidebar.command(PathCommand::Open(open.clone()), cx);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |sidebar, event: &MouseDownEvent, _, cx| {
                        let menu = sidebar.pin_menu(&menu_path, event.position);
                        cx.emit(SidebarEvent::Menu(menu));
                    }),
                )
                .child(icon(
                    Icon::Folder,
                    if current { p.current_row_text } else { p.muted },
                ))
                .child(
                    div()
                        .flex_none()
                        .max_w(px(130.))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(name),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_right()
                        .font_family(UI_FONT)
                        .text_size(px(11.))
                        .text_color(rgb(p.muted))
                        .child(truncate_start(&where_, 18)),
                )
        });
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .px(px(6.))
                .pt(px(10.))
                .pb(px(8.))
                .border_b_1()
                .border_color(rgb(p.line))
                .font(self.font.clone())
                .text_size(px(12.5))
                .child(ui::section_label("Pinned", p).pl(px(6.)))
                .children(rows),
        )
    }

    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette;
        let row = self.rows[index].clone();
        let current = row.path == self.tree.cwd;
        let ancestor = !current && row.is_dir && is_inside(&self.tree.cwd, &row.path);
        let color = if current {
            p.current_row_text
        } else if row.is_dimmed() {
            p.muted
        } else if ancestor {
            p.foreground
        } else {
            p.secondary
        };
        let icon_color = if current { p.current_row_text } else { p.muted };
        let chevron = row.is_dir.then(|| {
            let path = row.path.clone();
            div()
                .id(("chevron", index))
                .flex_none()
                .size(px(14.))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(cx.listener(move |sidebar, _, _, cx| {
                    cx.stop_propagation();
                    sidebar.tree.toggle(&path);
                    sidebar.update_rows(cx);
                }))
                .child(
                    svg()
                        .path(Icon::Chevron.path())
                        .size(px(10.))
                        .text_color(rgb(p.muted))
                        .when(row.is_open, |icon| {
                            icon.with_transformation(gpui::Transformation::rotate(gpui::radians(
                                std::f32::consts::FRAC_PI_2,
                            )))
                        }),
                )
        });
        let click_row = row.clone();
        let menu_row = row.clone();
        div()
            .id(("row", index))
            .w_full()
            .h(px(ROW_HEIGHT))
            .flex()
            .items_center()
            .gap(px(5.))
            .pl(px(4. + row.depth as f32 * 14.))
            .pr(px(8.))
            .rounded(px(5.))
            .whitespace_nowrap()
            .text_color(rgb(color))
            .when(current, |el| {
                el.bg(rgb(p.current_row))
                    .font_weight(gpui::FontWeight::BOLD)
            })
            .when(!current, |el| el.hover(|s| s.bg(rgb(p.line))))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .on_click(
                cx.listener(move |sidebar, event, _, cx| sidebar.click_row(&click_row, event, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |sidebar, event: &MouseDownEvent, _, cx| {
                    let menu = sidebar.row_menu(&menu_row, event.position);
                    cx.emit(SidebarEvent::Menu(menu));
                }),
            )
            .child(match chevron {
                Some(chevron) => chevron.into_any_element(),
                None => div().flex_none().size(px(14.)).into_any_element(),
            })
            .child(icon(
                if row.is_dir { Icon::Folder } else { Icon::File },
                icon_color,
            ))
            .child(truncate_end(
                &row.name,
                TREE_CHARS.saturating_sub(row.depth * 2),
            ))
            .into_any_element()
    }

    fn render_git(&self) -> Option<impl IntoElement> {
        let status = self.git.as_ref().filter(|_| self.show_git)?;
        let p = self.palette;
        let name = status
            .branch
            .clone()
            .or_else(|| status.commit.clone())
            .unwrap_or_else(|| "no commits".to_owned());
        // gpui can't ellipsize text in a shrinking flex item, so cut it to what fits.
        let counts = [
            (status.ahead > 0).then(|| format!("↑{}", status.ahead)),
            (status.behind > 0).then(|| format!("↓{}", status.behind)),
            (status.added > 0).then(|| format!("+{}", status.added)),
            (status.removed > 0).then(|| format!("−{}", status.removed)),
        ];
        let used: usize = counts.iter().flatten().map(|c| c.chars().count() + 1).sum();
        let name = truncate_end(&name, GIT_CHARS.saturating_sub(used));
        let count = |text: String, color: u32| div().flex_none().text_color(rgb(color)).child(text);
        Some(
            div()
                .id("git")
                .flex_none()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(10.))
                .min_h(px(34.))
                .px(px(12.))
                .border_t_1()
                .border_color(rgb(p.line))
                .font(self.font.clone())
                .text_size(px(12.))
                .text_color(rgb(p.muted))
                .whitespace_nowrap()
                .tooltip(ui::tooltip(describe(status), p))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            svg()
                                .path(Icon::Branch.path())
                                .size(px(13.))
                                .text_color(rgb(p.muted)),
                        )
                        .child(div().text_color(rgb(p.secondary)).child(name))
                        .when(status.ahead > 0, |el| {
                            el.child(count(format!("↑{}", status.ahead), p.muted))
                        })
                        .when(status.behind > 0, |el| {
                            el.child(count(format!("↓{}", status.behind), p.muted))
                        }),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .gap(px(6.))
                        .when(status.added > 0, |el| {
                            el.child(count(format!("+{}", status.added), p.ansi[2]))
                        })
                        .when(status.removed > 0, |el| {
                            el.child(count(format!("−{}", status.removed), p.ansi[1]))
                        }),
                ),
        )
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        div()
            .flex_none()
            .w(px(WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(p.chrome))
            .border_r_1()
            .border_color(rgb(p.line))
            .child(self.render_header(cx))
            .children(self.render_pins(cx))
            .child(
                uniform_list(
                    "tree",
                    self.rows.len(),
                    cx.processor(|sidebar, range: std::ops::Range<usize>, _, cx| {
                        range.map(|i| sidebar.render_row(i, cx)).collect()
                    }),
                )
                .track_scroll(self.scroll.clone())
                .flex_1()
                .p(px(6.))
                .font(self.font.clone())
                .text_size(px(12.5)),
            )
            .children(self.render_git())
    }
}

/// Draws a context menu at its position, above everything else.
pub fn render_menu(
    menu: &ContextMenu,
    palette: &'static Palette,
    pick: impl Fn(&PathCommand, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let pick = std::rc::Rc::new(pick);
    let items = menu
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| match entry {
            MenuEntry::Separator => div()
                .my(px(4.))
                .h(px(1.))
                .bg(rgb(palette.line))
                .into_any_element(),
            MenuEntry::Item { label, command } => {
                let command = command.clone();
                let pick = pick.clone();
                div()
                    .id(("menu-item", i))
                    .h(px(24.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .rounded(px(5.))
                    .hover(|s| s.bg(rgb(palette.menu_selected)))
                    // Acts on mouse down: any click closes the menu, which would
                    // otherwise remove the item before the click completes.
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                        pick(&command, window, cx);
                    })
                    .child(*label)
                    .into_any_element()
            }
        });
    deferred(
        anchored()
            .position(menu.position)
            .snap_to_window_with_margin(px(8.))
            .child(
                div()
                    .min_w(px(180.))
                    .p(px(4.))
                    .border_1()
                    .border_color(rgb(palette.line))
                    .rounded(px(8.))
                    .bg(rgb(palette.menu))
                    .shadow(ui::shadow(palette))
                    .font_family(UI_FONT)
                    .text_size(px(13.))
                    .text_color(rgb(palette.foreground))
                    .children(items),
            ),
    )
    .with_priority(1)
    .into_any_element()
}

fn icon(icon: Icon, color: u32) -> impl IntoElement {
    svg()
        .path(icon.path())
        .flex_none()
        .size(px(14.))
        .text_color(rgb(color))
}

/// Cuts long names at the end.
fn truncate_end(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let head: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{head}…")
}

/// Keeps the end of a long path, which says the most.
fn truncate_start(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_owned();
    }
    let tail: String = text.chars().skip(count - max + 1).collect();
    format!("…{tail}")
}

fn describe(s: &GitStatus) -> String {
    let plural = |n: u32, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
    let mut parts = vec![match &s.branch {
        Some(branch) => format!("On {branch}"),
        None => format!("Detached at {}", s.commit.as_deref().unwrap_or("no commit")),
    }];
    if let Some(upstream) = &s.upstream {
        let mut sync = Vec::new();
        if s.ahead > 0 {
            sync.push(format!("{} ahead", plural(s.ahead, "commit")));
        }
        if s.behind > 0 {
            sync.push(format!("{} behind", plural(s.behind, "commit")));
        }
        parts.push(if sync.is_empty() {
            format!("up to date with {upstream}")
        } else {
            format!("{} of {upstream}", sync.join(", "))
        });
    }
    parts.push(if s.files > 0 {
        format!(
            "{}, {} lines added, {} removed",
            plural(s.files, "changed file"),
            s.added,
            s.removed
        )
    } else {
        "no changes".to_owned()
    });
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_paths_keep_their_end() {
        assert_eq!(truncate_start("~/a/b", 10), "~/a/b");
        assert_eq!(truncate_start("~/projects/food-planner", 10), "…d-planner");
        assert_eq!(truncate_end("feature/weekly-plan", 10), "feature/w…");
        assert_eq!(truncate_end("main", 10), "main");
    }

    #[test]
    fn describes_the_repository() {
        let status = GitStatus {
            branch: Some("main".into()),
            upstream: Some("origin/main".into()),
            ahead: 1,
            files: 2,
            added: 5,
            removed: 1,
            ..GitStatus::default()
        };
        assert_eq!(
            describe(&status),
            "On main · 1 commit ahead of origin/main · 2 changed files, 5 lines added, 1 removed"
        );
    }
}
