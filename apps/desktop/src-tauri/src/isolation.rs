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

pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<String, IsolationError> {
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

/// Everything on `branch` since `base` (the commit it started from), or since it left the checked-out branch when no
/// base is known (`HEAD...branch`), at most `limit` bytes, with a note when cut. Reads only; nothing is checked out.
pub fn branch_diff(repo: &Path, branch: &str, base: Option<&str>, limit: usize) -> Result<String, IsolationError> {
    require_repository(repo)?;
    let range = match base {
        Some(base) => format!("{base}..refs/heads/{branch}"),
        None => format!("HEAD...refs/heads/{branch}"),
    };
    let mut text = git(repo, &["diff", &range])?;
    if text.len() > limit {
        let mut cut = limit;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("\n… [diff cut here; see the whole of it with git diff] …\n");
    }
    Ok(text)
}

/// The attempt's changes as a patch `git apply` takes, binary files included (ADR-009, Export Patch).
pub fn patch(worktree: &Worktree) -> Result<String, IsolationError> {
    git(&worktree.path, &["add", "--all"])?;
    let mut text = git(&worktree.path, &["diff", "--cached", "--binary", &worktree.base])?;
    // `git` output is trimmed; a patch must end with a newline to apply.
    text.push('\n');
    Ok(text)
}

/// A lowercase letter with its accents taken off (Vietnamese and the common Latin ones), so a branch name keeps the
/// words: "sửa lỗi" gives "sua-loi", not "s-a-l-i". Anything else is returned as it is.
fn fold(c: char) -> std::vec::IntoIter<char> {
    const GROUPS: &[(&str, &str)] = &[
        ("àáạảãâầấậẩẫăằắặẳẵäåā", "a"),
        ("èéẹẻẽêềếệểễëē", "e"),
        ("ìíịỉĩïî", "i"),
        ("òóọỏõôồốộổỗơờớợởỡöøō", "o"),
        ("ùúụủũưừứựửữüûū", "u"),
        ("ỳýỵỷỹÿ", "y"),
        ("đ", "d"),
        ("ç", "c"),
        ("ñ", "n"),
        ("ß", "ss"),
    ];
    for (from, to) in GROUPS {
        if from.contains(c) {
            return to.chars().collect::<Vec<_>>().into_iter();
        }
    }
    vec![c].into_iter()
}

/// `str/<slug>-<short id>`: lowercase ASCII letters, digits and dashes, at most 48 characters of slug.
pub fn branch_name(title: &str, id: &str) -> String {
    let mut slug = String::new();
    for c in title.chars().flat_map(char::to_lowercase).flat_map(fold) {
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
    if branch_exists(&worktree.repo, branch) {
        return Err(IsolationError::BranchExists(branch.to_string()));
    }
    let commit = commit_result(worktree, message)?;
    git(&worktree.repo, &["branch", branch, &commit])?;
    Ok(commit)
}

fn branch_exists(repo: &Path, branch: &str) -> bool {
    git(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_ok()
}

/// The user's own Git identity when they have one; otherwise the app's, so a commit can still be made.
fn identity(dir: &Path) -> Vec<&'static str> {
    let has_identity = git(dir, &["config", "user.email"]).map(|e| !e.is_empty()).unwrap_or(false);
    if has_identity {
        Vec::new()
    } else {
        vec!["-c", "user.name=Pygg", "-c", "user.email=pygg@localhost"]
    }
}

/// Commits everything the attempt changed, untracked files included, on its detached HEAD. Returns the commit.
pub fn commit_result(worktree: &Worktree, message: &str) -> Result<String, IsolationError> {
    git(&worktree.path, &["add", "--all"])?;
    let mut args = identity(&worktree.path);
    args.extend(["commit", "--allow-empty", "--no-verify", "-m", message]);
    git(&worktree.path, &args)?;
    git(&worktree.path, &["rev-parse", "HEAD"])
}

/// Creates `branch` at `commit`; an existing branch of that name is an error.
pub fn create_branch(repo: &Path, branch: &str, commit: &str) -> Result<(), IsolationError> {
    if branch_exists(repo, branch) {
        return Err(IsolationError::BranchExists(branch.to_string()));
    }
    git(repo, &["branch", branch, commit]).map(|_| ())
}

/// `branch`'s commit, creating the branch at the repository's HEAD when it does not exist yet.
pub fn ensure_branch(repo: &Path, branch: &str) -> Result<String, IsolationError> {
    require_repository(repo)?;
    if !branch_exists(repo, branch) {
        git(repo, &["branch", branch, "HEAD"])?;
    }
    git(repo, &["rev-parse", &format!("refs/heads/{branch}")])
}

/// Whether `branch` is checked out in any worktree of the repository, the user's own included.
fn checked_out(repo: &Path, branch: &str) -> Result<bool, IsolationError> {
    let list = git(repo, &["worktree", "list", "--porcelain"])?;
    let line = format!("branch refs/heads/{branch}");
    Ok(list.lines().any(|l| l.trim() == line))
}

/// Why a kept result could not be added to a brainstorm's branch.
#[derive(Clone, Debug, PartialEq)]
pub enum NotAdded {
    /// The branch is checked out somewhere; moving it would change files under someone's feet.
    CheckedOut,
    /// The branch moved since the attempt started, and the result does not apply cleanly on its tip.
    Conflict,
}

/// Adds `commit` (made on `base`) to `branch` without touching any checkout: as is when the branch is still at
/// `base`, otherwise picked onto the branch's tip in a temporary worktree. Returns the branch's new commit, or why
/// it was left alone.
pub fn add_to_branch(
    repo: &Path, branch: &str, commit: &str, base: &str,
) -> Result<Result<String, NotAdded>, IsolationError> {
    if checked_out(repo, branch)? {
        return Ok(Err(NotAdded::CheckedOut));
    }
    let reference = format!("refs/heads/{branch}");
    let tip = git(repo, &["rev-parse", &reference])?;
    if tip == base {
        git(repo, &["update-ref", &reference, commit, &tip])?;
        return Ok(Ok(commit.to_string()));
    }
    let dir = std::env::temp_dir().join(format!("pygg-pick-{}", uuid::Uuid::new_v4().simple()));
    let dir_text = dir.to_string_lossy().into_owned();
    git(repo, &["worktree", "add", "--detach", &dir_text, &tip])?;
    let mut args = identity(&dir);
    args.extend(["cherry-pick", "--allow-empty", "--keep-redundant-commits", commit]);
    let picked = match git(&dir, &args) {
        Ok(_) => Some(git(&dir, &["rev-parse", "HEAD"])?),
        Err(_) => {
            let _ = git(&dir, &["cherry-pick", "--abort"]);
            None
        }
    };
    let _ = git(repo, &["worktree", "remove", "--force", &dir_text]);
    let _ = git(repo, &["worktree", "prune"]);
    match picked {
        Some(new) => {
            git(repo, &["update-ref", &reference, &new, &tip])?;
            Ok(Ok(new))
        }
        None => Ok(Err(NotAdded::Conflict)),
    }
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
    fn a_result_is_added_to_a_branch_on_top_or_picked_onto_its_moved_tip() {
        let repo = std::env::temp_dir().join(format!("pygg-add-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("a.txt"), "one\n").unwrap();
        init_repository(&repo, "start").unwrap();
        let start = head(&repo).unwrap();
        let tip = ensure_branch(&repo, "str/brain-x").unwrap();
        assert_eq!(tip, start);
        assert_eq!(ensure_branch(&repo, "str/brain-x").unwrap(), start, "an existing branch is kept as it is");
        let attempt = |name: &str, file: &str, text: &str, base: &str| {
            let w =
                create(&repo, base, &std::env::temp_dir().join(format!("pygg-add-{name}-{}", uuid::Uuid::new_v4())))
                    .unwrap();
            std::fs::write(w.path.join(file), text).unwrap();
            let commit = commit_result(&w, name).unwrap();
            (w, commit)
        };
        // On top: the branch is still at the attempt's base.
        let (w1, c1) = attempt("first", "b.txt", "two\n", &start);
        assert_eq!(add_to_branch(&repo, "str/brain-x", &c1, &w1.base).unwrap(), Ok(c1.clone()));
        // Moved: an attempt that started before c1 is picked onto the new tip, keeping both files.
        let (w2, c2) = attempt("second", "c.txt", "three\n", &start);
        let picked = add_to_branch(&repo, "str/brain-x", &c2, &w2.base).unwrap().unwrap();
        assert_ne!(picked, c2);
        let files = git(&repo, &["ls-tree", "--name-only", "refs/heads/str/brain-x"]).unwrap();
        assert_eq!(files.lines().collect::<Vec<_>>(), ["a.txt", "b.txt", "c.txt"]);
        // A conflict leaves the branch where it was.
        let (w3, c3) = attempt("third", "b.txt", "other\n", &start);
        assert_eq!(add_to_branch(&repo, "str/brain-x", &c3, &w3.base).unwrap(), Err(NotAdded::Conflict));
        assert_eq!(git(&repo, &["rev-parse", "refs/heads/str/brain-x"]).unwrap(), picked);
        // A branch someone has checked out is never moved.
        git(&repo, &["checkout", "-q", "str/brain-x"]).unwrap();
        let (w4, c4) = attempt("fourth", "d.txt", "four\n", &picked);
        assert_eq!(add_to_branch(&repo, "str/brain-x", &c4, &w4.base).unwrap(), Err(NotAdded::CheckedOut));
        for w in [w1, w2, w3, w4] {
            discard(&w).unwrap();
        }
    }

    #[test]
    fn a_branchs_diff_is_against_where_it_left_the_checkout() {
        let repo = std::env::temp_dir().join(format!("pygg-bdiff-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("a.txt"), "one\n").unwrap();
        init_repository(&repo, "start").unwrap();
        ensure_branch(&repo, "str/brain-y").unwrap();
        let w =
            create(&repo, "str/brain-y", &std::env::temp_dir().join(format!("pygg-bdiff-w-{}", uuid::Uuid::new_v4())))
                .unwrap();
        std::fs::write(w.path.join("b.txt"), "two\n").unwrap();
        let c = commit_result(&w, "card").unwrap();
        add_to_branch(&repo, "str/brain-y", &c, &w.base).unwrap().unwrap();
        discard(&w).unwrap();
        let d = branch_diff(&repo, "str/brain-y", None, 10_000).unwrap();
        assert!(d.contains("+two") && !d.contains("a.txt"), "{d}");
        assert!(branch_diff(&repo, "str/brain-y", None, 20).unwrap().ends_with("git diff] …\n"));
        // From the plan's own start, whatever is checked out now.
        let start = head(&repo).unwrap();
        git(&repo, &["checkout", "-q", "--detach", &c]).unwrap();
        assert!(branch_diff(&repo, "str/brain-y", None, 10_000).unwrap().is_empty(), "against the checkout: nothing");
        assert!(
            branch_diff(&repo, "str/brain-y", Some(&start), 10_000).unwrap().contains("+two"),
            "against the plan's start"
        );
    }

    #[test]
    fn branch_names_are_safe() {
        assert_eq!(branch_name("  Sửa lỗi --- timeout  ", "ABCDEF123"), "str/sua-loi-timeout-abcdef12");
        assert_eq!(branch_name("brain đọc kiến trúc dự án", "b_931be223ee"), "str/brain-doc-kien-truc-du-an-b931be22");
        assert_eq!(branch_name("!!!", "42"), "str/task-42");
        assert!(branch_name(&"x".repeat(200), "1").len() <= "str/".len() + 48 + 1 + 1);
    }
}
