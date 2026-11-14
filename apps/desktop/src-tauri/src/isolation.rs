//! One Git worktree per attempt (ADR-008). An attempt works on a detached checkout of a recorded base
//! revision, so the user's working tree and current branch are never touched. A passing result is kept
//! as a branch (ADR-009); anything else is discarded with its worktree.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug)]
pub enum IsolationError {
    NotARepository(PathBuf),
    /// Git is not installed or not on PATH.
    GitMissing,
    BranchExists(String),
    /// A git command failed; the message is git's own stderr.
    Git {
        args: String,
        message: String,
    },
}

use crate::messages::{self as m, fill};

impl std::fmt::Display for IsolationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IsolationError::NotARepository(p) => f.write_str(&fill(m::NOT_A_REPOSITORY, &[&p.display()])),
            IsolationError::GitMissing => f.write_str(m::GIT_MISSING),
            IsolationError::BranchExists(b) => f.write_str(&fill(m::BRANCH_EXISTS, &[b])),
            IsolationError::Git { args, message } => f.write_str(&fill(m::GIT_FAILED, &[args, message])),
        }
    }
}

impl std::error::Error for IsolationError {}

fn git(dir: &Path, args: &[&str]) -> Result<String, IsolationError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir).args(args).stdin(Stdio::null());
    no_window(&mut command);
    let output = command.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            IsolationError::GitMissing
        } else {
            IsolationError::Git { args: args.join(" "), message: e.to_string() }
        }
    })?;
    if !output.status.success() {
        return Err(IsolationError::Git {
            args: args.join(" "),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(windows)]
fn no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn no_window(_command: &mut Command) {}

/// Whether `path` is inside a Git working tree.
pub fn is_repository(path: &Path) -> bool {
    git(path, &["rev-parse", "--is-inside-work-tree"]).map(|out| out == "true").unwrap_or(false)
}

/// The root of the working tree that contains `path`.
pub fn toplevel(path: &Path) -> Result<PathBuf, IsolationError> {
    require_repository(path)?;
    // Git prints forward slashes on Windows too.
    let root = git(path, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(if cfg!(windows) { root.replace('/', "\\") } else { root }))
}

/// The commit `HEAD` points at.
pub fn head(repo: &Path) -> Result<String, IsolationError> {
    require_repository(repo)?;
    git(repo, &["rev-parse", "HEAD"])
}

/// The current branch name, or `None` on a detached HEAD.
pub fn current_branch(repo: &Path) -> Result<Option<String>, IsolationError> {
    let name = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(if name == "HEAD" { None } else { Some(name) })
}

fn require_repository(repo: &Path) -> Result<(), IsolationError> {
    if is_repository(repo) {
        Ok(())
    } else {
        Err(IsolationError::NotARepository(repo.to_path_buf()))
    }
}

#[derive(Clone, Debug)]
pub struct Worktree {
    pub repo: PathBuf,
    pub path: PathBuf,
    /// The commit the attempt started from.
    pub base: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiffStat {
    pub files: usize,
    pub insertions: usize,
    pub deletions: usize,
}

/// Creates a detached worktree of `repo` at `base` in `dir` (which must not exist yet).
pub fn create(repo: &Path, base: &str, dir: &Path) -> Result<Worktree, IsolationError> {
    require_repository(repo)?;
    let base = git(repo, &["rev-parse", "--verify", &format!("{base}^{{commit}}")])?;
    let dir_text = dir.to_string_lossy().into_owned();
    git(repo, &["worktree", "add", "--detach", &dir_text, &base])?;
    Ok(Worktree { repo: repo.to_path_buf(), path: dir.to_path_buf(), base })
}

/// Everything the attempt changed relative to its base, untracked files included.
pub fn changes(worktree: &Worktree) -> Result<DiffStat, IsolationError> {
    git(&worktree.path, &["add", "--all"])?;
    let numstat = git(&worktree.path, &["diff", "--cached", "--numstat", &worktree.base])?;
    let mut stat = DiffStat::default();
    for line in numstat.lines().filter(|l| !l.trim().is_empty()) {
        let mut parts = line.split('\t');
        // Binary files report "-" for both counts.
        let added = parts.next().and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
        let removed = parts.next().and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
        stat.files += 1;
        stat.insertions += added;
        stat.deletions += removed;
    }
    Ok(stat)
}

/// The paths the attempt added, changed or removed, relative to the repository root.
pub fn changed_files(worktree: &Worktree) -> Result<Vec<String>, IsolationError> {
    git(&worktree.path, &["add", "--all"])?;
    let names = git(&worktree.path, &["diff", "--cached", "--name-only", &worktree.base])?;
    Ok(names.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
}

/// The unified diff of the attempt against its base.
pub fn diff(worktree: &Worktree) -> Result<String, IsolationError> {
    git(&worktree.path, &["add", "--all"])?;
    git(&worktree.path, &["diff", "--cached", &worktree.base])
}

/// The attempt's changes as a patch `git apply` takes, binary files included (ADR-009, Export Patch).
pub fn patch(worktree: &Worktree) -> Result<String, IsolationError> {
    git(&worktree.path, &["add", "--all"])?;
    let mut text = git(&worktree.path, &["diff", "--cached", "--binary", &worktree.base])?;
    // `git` output is trimmed; a patch must end with a newline to apply.
    text.push('\n');
    Ok(text)
}

/// `str/<slug>-<short id>`: lowercase ASCII letters, digits and dashes, at most 48 characters of slug.
pub fn branch_name(title: &str, id: &str) -> String {
    let mut slug = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
        if slug.len() >= 48 {
            break;
        }
    }
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "task" } else { slug };
    let short: String = id.chars().filter(|c| c.is_ascii_alphanumeric()).take(8).collect::<String>().to_lowercase();
    format!("str/{slug}-{short}")
}

/// Commits the attempt's changes and creates `branch` at that commit. The user's checkout is untouched.
/// Returns the new commit.
pub fn keep_as_branch(worktree: &Worktree, branch: &str, message: &str) -> Result<String, IsolationError> {
    if git(&worktree.repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_ok() {
        return Err(IsolationError::BranchExists(branch.to_string()));
    }
    git(&worktree.path, &["add", "--all"])?;
    // The user's own Git identity when they have one; otherwise the app's, so the commit can still be made.
    let has_identity = git(&worktree.path, &["config", "user.email"]).map(|e| !e.is_empty()).unwrap_or(false);
    let mut args: Vec<&str> = Vec::new();
    if !has_identity {
        args.extend(["-c", "user.name=Pygg", "-c", "user.email=pygg@localhost"]);
    }
    args.extend(["commit", "--allow-empty", "--no-verify", "-m", message]);
    git(&worktree.path, &args)?;
    let commit = git(&worktree.path, &["rev-parse", "HEAD"])?;
    git(&worktree.repo, &["branch", branch, &commit])?;
    Ok(commit)
}

/// Makes `dir` a repository whose single commit holds everything in it. Used for benchmark fixtures,
/// never for a user's workspace. Returns the commit.
pub fn init_repository(dir: &Path, message: &str) -> Result<String, IsolationError> {
    git(dir, &["init", "--quiet", "--initial-branch=main"])?;
    git(dir, &["config", "user.name", "Pygg Bench"])?;
    git(dir, &["config", "user.email", "bench@pygg.localhost"])?;
    // Fixtures compare byte-for-byte across platforms.
    git(dir, &["config", "core.autocrlf", "false"])?;
    git(dir, &["add", "--all"])?;
    git(dir, &["commit", "--quiet", "--allow-empty", "-m", message])?;
    git(dir, &["rev-parse", "HEAD"])
}

/// Forgets worktrees whose folder is gone.
pub fn prune(repo: &Path) -> Result<(), IsolationError> {
    git(repo, &["worktree", "prune"]).map(|_| ())
}

/// Removes the worktree and its files. The branch, if one was kept, stays.
pub fn discard(worktree: &Worktree) -> Result<(), IsolationError> {
    let path = worktree.path.to_string_lossy().into_owned();
    git(&worktree.repo, &["worktree", "remove", "--force", &path])?;
    git(&worktree.repo, &["worktree", "prune"])?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;

    /// A fresh repository with one committed file, `app.txt`.
    pub(crate) fn repo_with_commit(dir: &Path) -> PathBuf {
        let repo = dir.join("repo");
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "--quiet", "--initial-branch=main"]).unwrap();
        git(&repo, &["config", "user.name", "Test"]).unwrap();
        git(&repo, &["config", "user.email", "test@example.com"]).unwrap();
        fs::write(repo.join("app.txt"), "one\n").unwrap();
        git(&repo, &["add", "--all"]).unwrap();
        git(&repo, &["commit", "--quiet", "-m", "initial"]).unwrap();
        repo
    }

    #[test]
    fn detects_repositories() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_repository(dir.path()));
        let repo = repo_with_commit(dir.path());
        assert!(is_repository(&repo));
        assert!(matches!(head(dir.path()), Err(IsolationError::NotARepository(_))));
    }

    #[test]
    fn keeps_a_passing_attempt_as_a_branch_without_touching_the_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_with_commit(dir.path());
        let base = head(&repo).unwrap();
        let worktree = create(&repo, "HEAD", &dir.path().join("attempt-1")).unwrap();
        assert_eq!(worktree.base, base);

        fs::write(worktree.path.join("app.txt"), "one\ntwo\n").unwrap();
        fs::write(worktree.path.join("new.txt"), "new\n").unwrap();
        assert_eq!(changes(&worktree).unwrap(), DiffStat { files: 2, insertions: 2, deletions: 0 });

        let branch = branch_name("Fix timeout handling!", "3f2a9c1d-xyz");
        assert_eq!(branch, "str/fix-timeout-handling-3f2a9c1d");
        let commit = keep_as_branch(&worktree, &branch, "Fix timeout handling").unwrap();
        discard(&worktree).unwrap();

        // The user's checkout is unchanged: same branch, same commit, same file.
        assert_eq!(current_branch(&repo).unwrap().as_deref(), Some("main"));
        assert_eq!(head(&repo).unwrap(), base);
        assert_eq!(fs::read_to_string(repo.join("app.txt")).unwrap(), "one\n");
        assert!(!worktree.path.exists());
        // The kept branch holds the change.
        assert_eq!(git(&repo, &["rev-parse", &branch]).unwrap(), commit);
        assert_eq!(git(&repo, &["show", &format!("{branch}:app.txt")]).unwrap(), "one\ntwo");

        // Keeping the same branch twice is refused.
        let again = create(&repo, "HEAD", &dir.path().join("attempt-2")).unwrap();
        assert!(matches!(keep_as_branch(&again, &branch, "again"), Err(IsolationError::BranchExists(_))));
        discard(&again).unwrap();
    }

    #[test]
    fn discarding_a_failed_attempt_leaves_no_trace() {
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_with_commit(dir.path());
        let worktree = create(&repo, "HEAD", &dir.path().join("attempt")).unwrap();
        fs::write(worktree.path.join("app.txt"), "broken\n").unwrap();
        discard(&worktree).unwrap();
        assert_eq!(fs::read_to_string(repo.join("app.txt")).unwrap(), "one\n");
        let worktrees = git(&repo, &["worktree", "list", "--porcelain"]).unwrap();
        assert_eq!(worktrees.matches("worktree ").count(), 1, "only the main checkout remains: {worktrees}");
    }

    #[test]
    fn branch_names_are_safe() {
        assert_eq!(branch_name("  Sửa lỗi --- timeout  ", "ABCDEF123"), "str/s-a-l-i-timeout-abcdef12");
        assert_eq!(branch_name("!!!", "42"), "str/task-42");
        assert!(branch_name(&"x".repeat(200), "1").len() <= "str/".len() + 48 + 1 + 1);
    }
}
