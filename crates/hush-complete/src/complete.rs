use crate::context::CompletionContext;
use crate::paths::{escape_shell_word, resolve_path};
use crate::sources::{CompletionSources, DirEntry, Place};
use crate::specs::{self, Suggestion, BRANCH_SUBCOMMANDS, FOLDER_COMMANDS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionKind {
    Command,
    Subcommand,
    Folder,
    File,
    Branch,
    Flag,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    /// Replaces the raw word (from `word_start` to the cursor), already shell-escaped.
    pub insert: String,
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<&'static str>,
    pub append_space: bool,
    /// Byte length of the start of `label` that matches what was typed.
    pub matched: usize,
}

const MAX_ITEMS: usize = 200;

/// Candidates for the word at the cursor. `commands` are the names known in command position.
pub fn complete(
    ctx: &CompletionContext,
    place: Place<'_>,
    commands: &[String],
    sources: &dyn CompletionSources,
) -> Vec<CompletionItem> {
    let word = ctx.word.as_str();
    let Some(command) = ctx.args.first() else {
        if word.contains('/') {
            return complete_path(ctx, place, sources, false);
        }
        return by_name(commands, String::as_str, word)
            .into_iter()
            .map(|name| item(name, CompletionKind::Command, word))
            .collect();
    };
    let sub = ctx.args.get(1).map(String::as_str);
    let subcommands = specs::subcommands(command);

    if word.starts_with('-') {
        let key = match sub {
            Some(sub) if subcommands.is_some() => format!("{command} {sub}"),
            _ => command.clone(),
        };
        return from_suggestions(
            specs::flags(&key).unwrap_or_default(),
            word,
            CompletionKind::Flag,
        );
    }
    if let (1, Some(subcommands)) = (ctx.args.len(), subcommands) {
        return from_suggestions(subcommands, word, CompletionKind::Subcommand);
    }
    if let Some(sub) = sub {
        let after_create_flag = matches!(ctx.args.last().map(String::as_str), Some("-b" | "-c"));
        if command == "git" && BRANCH_SUBCOMMANDS.contains(&sub) && !after_create_flag {
            let branches = sources.git_branches(place.cwd);
            let items: Vec<_> = by_name(&branches, String::as_str, word)
                .into_iter()
                .map(|name| item(name, CompletionKind::Branch, word))
                .collect();
            // `git checkout` also takes files.
            if !items.is_empty() || sub != "checkout" {
                return items;
            }
        }
    }
    complete_path(
        ctx,
        place,
        sources,
        FOLDER_COMMANDS.contains(&command.as_str()),
    )
}

fn complete_path(
    ctx: &CompletionContext,
    place: Place<'_>,
    sources: &dyn CompletionSources,
    folders_only: bool,
) -> Vec<CompletionItem> {
    let word = ctx.word.as_str();
    let (typed_dir, prefix) = match word.rfind('/') {
        Some(slash) => word.split_at(slash + 1),
        None => ("", word),
    };
    // Keep the folder part exactly as typed (with its escapes); only the name is rewritten.
    let raw_dir = ctx
        .raw_word
        .rfind('/')
        .map_or("", |slash| &ctx.raw_word[..=slash]);

    let dir = if typed_dir.is_empty() {
        place.cwd.to_owned()
    } else {
        resolve_path(typed_dir, place.cwd, place.home)
    };
    let Ok(entries) = sources.list_dir(&dir) else {
        return Vec::new();
    };
    let show_hidden = prefix.starts_with('.');
    let candidates: Vec<&DirEntry> = entries
        .iter()
        .filter(|e| (show_hidden || !e.name.starts_with('.')) && (!folders_only || e.is_dir))
        .collect();

    by_name(&candidates, |e| &e.name, prefix)
        .into_iter()
        .map(|entry| {
            let slash = if entry.is_dir { "/" } else { "" };
            CompletionItem {
                insert: format!("{raw_dir}{}{slash}", escape_shell_word(&entry.name)),
                label: format!("{}{slash}", entry.name),
                kind: if entry.is_dir {
                    CompletionKind::Folder
                } else {
                    CompletionKind::File
                },
                detail: None,
                append_space: !entry.is_dir,
                matched: matched_len(&entry.name, prefix),
            }
        })
        .collect()
}

/// Items whose name starts with `prefix`; falls back to ignoring case, as macOS file names do.
fn by_name<'a, T>(items: &'a [T], name: impl Fn(&'a T) -> &'a str, prefix: &str) -> Vec<&'a T> {
    let exact: Vec<&T> = items
        .iter()
        .filter(|it| name(it).starts_with(prefix))
        .take(MAX_ITEMS)
        .collect();
    if !exact.is_empty() {
        return exact;
    }
    let prefix = prefix.to_lowercase();
    items
        .iter()
        .filter(|it| name(it).to_lowercase().starts_with(&prefix))
        .take(MAX_ITEMS)
        .collect()
}

/// How much of `name` the typed `prefix` covers, on a char boundary even for case-insensitive matches.
fn matched_len(name: &str, prefix: &str) -> usize {
    if name.starts_with(prefix) {
        return prefix.len();
    }
    name.char_indices()
        .nth(prefix.chars().count())
        .map_or(name.len(), |(i, _)| i)
}

fn from_suggestions(
    list: &'static [Suggestion],
    word: &str,
    kind: CompletionKind,
) -> Vec<CompletionItem> {
    by_name(list, |s| s.name, word)
        .into_iter()
        .map(|suggestion| CompletionItem {
            detail: Some(suggestion.description),
            ..item(suggestion.name, kind, word)
        })
        .collect()
}

fn item(name: &str, kind: CompletionKind, typed: &str) -> CompletionItem {
    CompletionItem {
        insert: escape_shell_word(name),
        label: name.to_owned(),
        kind,
        detail: None,
        append_space: true,
        matched: matched_len(name, typed),
    }
}

/// The longest start all inserts share, never ending inside a multibyte char.
pub fn common_prefix(items: &[CompletionItem]) -> &str {
    let Some(first) = items.first() else {
        return "";
    };
    let mut prefix = first.insert.as_str();
    for item in &items[1..] {
        let len = prefix
            .chars()
            .zip(item.insert.chars())
            .take_while(|(a, b)| a == b)
            .map(|(a, _)| a.len_utf8())
            .sum();
        prefix = &prefix[..len];
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::completion_context;
    use crate::fake::{dir, file, FakeSources};

    const PLACE: Place<'static> = Place {
        cwd: "/home/me",
        home: "/home/me",
    };

    fn sources() -> FakeSources {
        FakeSources {
            branches: vec!["main".into(), "fix-tests".into()],
            ..Default::default()
        }
        .with_dir(
            "/home/me",
            vec![
                dir("workspace"),
                dir("Work Notes"),
                file(".zshrc"),
                file("todo.txt"),
            ],
        )
        .with_dir(
            "/home/me/workspace",
            vec![dir("food-planner"), file("fonts.zip")],
        )
    }

    fn run_with(text: &str, sources: &FakeSources) -> Vec<CompletionItem> {
        let commands: Vec<String> = ["git", "grep", "gzip", "ls"].map(String::from).into();
        complete(
            &completion_context(text, text.len()),
            PLACE,
            &commands,
            sources,
        )
    }

    fn run(text: &str) -> Vec<CompletionItem> {
        run_with(text, &sources())
    }

    fn labels(items: &[CompletionItem]) -> Vec<&str> {
        items.iter().map(|i| i.label.as_str()).collect()
    }

    fn inserts(items: &[CompletionItem]) -> Vec<&str> {
        items.iter().map(|i| i.insert.as_str()).collect()
    }

    #[test]
    fn completes_commands_in_command_position() {
        assert_eq!(labels(&run("g")), ["git", "grep", "gzip"]);
    }

    #[test]
    fn completes_subcommands_flags_and_branches() {
        assert_eq!(labels(&run("git ch")), ["checkout", "cherry-pick"]);
        assert_eq!(labels(&run("git commit --am")), ["--amend"]);
        assert_eq!(inserts(&run("git checkout f")), ["fix-tests"]);
    }

    #[test]
    fn completes_paths_with_escapes_and_folder_slashes() {
        let items = run("cat W");
        let pairs: Vec<_> = items
            .iter()
            .map(|i| (i.insert.as_str(), i.append_space))
            .collect();
        assert_eq!(pairs, [("Work\\ Notes/", false)]);
        assert_eq!(
            inserts(&run("ls ~/workspace/fo")),
            ["~/workspace/food-planner/", "~/workspace/fonts.zip"]
        );
    }

    #[test]
    fn only_offers_folders_to_cd_and_hides_dotfiles_unless_asked() {
        assert_eq!(labels(&run("cd ")), ["workspace/", "Work Notes/"]);
        assert_eq!(labels(&run("cat .z")), [".zshrc"]);
    }

    #[test]
    fn falls_back_to_case_insensitive_matches() {
        assert_eq!(labels(&run("cd work")), ["workspace/"]);
        assert_eq!(labels(&run("cd WORK")), ["workspace/", "Work Notes/"]);
    }

    #[test]
    fn finds_the_shared_prefix() {
        assert_eq!(common_prefix(&run("ls workspace/fo")), "workspace/fo");
        assert_eq!(common_prefix(&run("git ch")), "che");
    }

    #[test]
    fn keeps_multibyte_names_whole() {
        let sources = sources().with_dir(
            "/home/me/mix",
            vec![dir("Été"), file("née.txt"), file("nüsse.txt")],
        );
        // "é" and "ü" share their first UTF-8 byte; the prefix must stop before it.
        let items = run_with("cat mix/n", &sources);
        assert_eq!(inserts(&items), ["mix/née.txt", "mix/nüsse.txt"]);
        assert_eq!(common_prefix(&items), "mix/n");

        let items = run_with("cd mix/ét", &sources);
        assert_eq!(labels(&items), ["Été/"]);
        assert_eq!(items[0].matched, "Ét".len());
    }
}
