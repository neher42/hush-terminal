use std::path::Path;
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    /// Branch name, or `None` for a detached HEAD.
    pub branch: Option<String>,
    /// Short commit id, shown when HEAD is detached.
    pub commit: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Changed, staged and untracked files.
    pub files: u32,
    pub added: u32,
    pub removed: u32,
}

/// Status of the repository containing `cwd`, or `None` outside a repository.
pub fn status(cwd: &Path) -> Option<GitStatus> {
    let porcelain = git(cwd, &["status", "--porcelain=v2", "--branch"])?;
    let mut status = parse_status(&porcelain);
    // Fails in a repository without commits; then there are no line counts yet.
    if let Some(numstat) = git(cwd, &["diff", "--numstat", "HEAD"]) {
        (status.added, status.removed) = parse_numstat(&numstat);
    }
    Some(status)
}

pub fn branches(cwd: &Path) -> Vec<String> {
    git(
        cwd,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )
    .map(|out| {
        out.lines()
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    })
    .unwrap_or_default()
}

fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        // Read-only status checks must not take locks other git processes need.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Parses `git status --porcelain=v2 --branch`.
pub fn parse_status(output: &str) -> GitStatus {
    let mut status = GitStatus::default();
    for line in output.lines() {
        if let Some(header) = line.strip_prefix("# ") {
            let (key, value) = header.split_once(' ').unwrap_or((header, ""));
            match key {
                "branch.oid" if value != "(initial)" => {
                    status.commit = Some(value.chars().take(7).collect())
                }
                "branch.head" if value != "(detached)" => status.branch = Some(value.to_owned()),
                "branch.upstream" => status.upstream = Some(value.to_owned()),
                "branch.ab" => {
                    for part in value.split_whitespace() {
                        if let Some(n) = part.strip_prefix('+') {
                            status.ahead = n.parse().unwrap_or(0);
                        } else if let Some(n) = part.strip_prefix('-') {
                            status.behind = n.parse().unwrap_or(0);
                        }
                    }
                }
                _ => {}
            }
        } else if !line.is_empty() && !line.starts_with('!') {
            status.files += 1;
        }
    }
    status
}

/// Sums `git diff --numstat`; binary files ("-") don't count.
pub fn parse_numstat(output: &str) -> (u32, u32) {
    output.lines().fold((0, 0), |(added, removed), line| {
        let mut parts = line.split('\t');
        let a = parts
            .next()
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(0);
        let r = parts
            .next()
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(0);
        (added + a, removed + r)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branch_upstream_and_counts() {
        let out = "# branch.oid e350d2c878f6465471101c745205c87dd61755d2\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +2 -1\n1 .M N... 100644 100644 100644 a b src/a.ts\n? notes.txt\n";
        let s = parse_status(out);
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert_eq!((s.ahead, s.behind, s.files), (2, 1, 2));
        assert_eq!(s.commit.as_deref(), Some("e350d2c"));
    }

    #[test]
    fn handles_detached_head_and_new_repositories() {
        let detached = parse_status("# branch.oid abcdef1234\n# branch.head (detached)\n");
        assert_eq!(detached.branch, None);
        assert_eq!(detached.commit.as_deref(), Some("abcdef1"));
        let fresh = parse_status("# branch.oid (initial)\n# branch.head main\n");
        assert_eq!(fresh.commit, None);
        assert_eq!(fresh.branch.as_deref(), Some("main"));
    }

    #[test]
    fn sums_line_counts_and_skips_binary_files() {
        assert_eq!(
            parse_numstat("10\t2\ta.ts\n-\t-\timage.png\n3\t0\tb.ts\n"),
            (13, 2)
        );
        assert_eq!(parse_numstat(""), (0, 0));
    }

    #[test]
    fn outside_a_repository_there_is_no_status() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(status(dir.path()), None);
        assert!(branches(dir.path()).is_empty());
    }
}
