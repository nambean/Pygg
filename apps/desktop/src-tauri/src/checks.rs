//! Deterministic checks (ADR-007): finding the ones a workspace already has, and running one in an
//! attempt's worktree. Detected checks are suggestions until the user approves them for the workspace.

use crate::process::{self, Exit, Spec};
use crate::redact::redact;
use serde::Serialize;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DetectedCheck {
    pub command: String,
    /// The file it was found in, e.g. `package.json`.
    pub source: String,
}

/// package.json scripts worth running as checks, in the order they are suggested.
const SCRIPT_CHECKS: &[&str] = &["typecheck", "lint", "test", "build"];

pub fn detect(workspace: &Path) -> Vec<DetectedCheck> {
    let mut found = Vec::new();
    if let Ok(text) = std::fs::read_to_string(workspace.join("package.json")) {
        if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) {
            let (runner, install) = if workspace.join("pnpm-lock.yaml").exists() {
                ("pnpm", "pnpm install --frozen-lockfile")
            } else if workspace.join("yarn.lock").exists() {
                ("yarn", "yarn install --frozen-lockfile")
            } else if workspace.join("package-lock.json").exists() {
                ("npm run", "npm ci")
            } else {
                ("npm run", "npm install")
            };
            if let Some(scripts) = manifest.get("scripts").and_then(|s| s.as_object()) {
                let first = found.len();
                for name in SCRIPT_CHECKS {
                    let Some(body) = scripts.get(*name).and_then(|b| b.as_str()) else { continue };
                    // npm's placeholder test script always fails; it is not a check.
                    if body.contains("no test specified") {
                        continue;
                    }
                    found.push(DetectedCheck { command: format!("{runner} {name}"), source: "package.json".into() });
                }
                // An attempt's worktree is a fresh checkout without node_modules, so the scripts need the
                // dependencies installed there first. Checks run in order and stop at the first failure.
                if found.len() > first {
                    found.insert(first, DetectedCheck { command: install.into(), source: "package.json".into() });
                }
            }
        }
    }
    if workspace.join("Cargo.toml").exists() {
        found.push(DetectedCheck { command: "cargo check".into(), source: "Cargo.toml".into() });
        found.push(DetectedCheck { command: "cargo test".into(), source: "Cargo.toml".into() });
    }
    detect_more(workspace, &mut found);
    found
}

/// What a repository with thin tests still has (ROADMAP focus item 3): its type checker, Node's own test runner over
/// test files no script runs, and each language's own build, parse and test commands. Each is a suggestion with what
/// it covers (`covers`); the user approves it or not.
fn detect_more(workspace: &Path, found: &mut Vec<DetectedCheck>) {
    let has = |name: &str| workspace.join(name).exists();
    if let Some(manifest) = std::fs::read_to_string(workspace.join("package.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    {
        let scripts = manifest.get("scripts").and_then(|s| s.as_object());
        let script = |name: &str| scripts.and_then(|s| s.get(name)).and_then(|b| b.as_str());
        let depends_on = |name: &str| {
            ["dependencies", "devDependencies"].iter().any(|k| manifest.get(*k).and_then(|d| d.get(name)).is_some())
        };
        let (install, exec) = if has("pnpm-lock.yaml") {
            ("pnpm install --frozen-lockfile", "pnpm exec")
        } else if has("yarn.lock") {
            ("yarn install --frozen-lockfile", "yarn")
        } else if has("package-lock.json") {
            ("npm ci", "npx --no-install")
        } else {
            ("npm install", "npx --no-install")
        };
        let real_test = script("test").is_some_and(|b| !b.contains("no test specified"));
        let mut wants_install = false;
        if has("tsconfig.json") && script("typecheck").is_none() && depends_on("typescript") {
            push_new(found, format!("{exec} tsc --noEmit"), "tsconfig.json");
            wants_install = true;
        }
        if !real_test && has_js_tests(workspace) {
            push_new(found, "node --test".into(), "test files");
        }
        if wants_install && !found.iter().any(|c| c.command == install) {
            let at = found.iter().position(|c| c.command.ends_with("tsc --noEmit")).unwrap_or(found.len());
            found.insert(at, DetectedCheck { command: install.into(), source: "package.json".into() });
        }
    }
    let python = if cfg!(windows) { "python" } else { "python3" };
    if has_files(workspace, &["py"], 2) {
        push_new(found, format!("{python} -m compileall -q ."), "Python files");
        if has_python_tests(workspace) {
            let pytest =
                ["pyproject.toml", "setup.cfg", "tox.ini", "requirements.txt", "requirements-dev.txt", "pytest.ini"]
                    .iter()
                    .any(|f| std::fs::read_to_string(workspace.join(f)).is_ok_and(|t| t.contains("pytest")));
            let command =
                if pytest { format!("{python} -m pytest -q") } else { format!("{python} -m unittest discover -q") };
            push_new(found, command, "Python tests");
        }
    }
    if has("go.mod") {
        push_new(found, "go vet ./...".into(), "go.mod");
        push_new(found, "go test ./...".into(), "go.mod");
    }
    if std::fs::read_to_string(workspace.join("Makefile")).is_ok_and(|t| t.lines().any(|l| l.starts_with("test:"))) {
        push_new(found, "make test".into(), "Makefile");
    }
    if has_files(workspace, &["sln", "csproj", "fsproj"], 1) {
        push_new(found, "dotnet build".into(), ".NET project");
        push_new(found, "dotnet test".into(), ".NET project");
    }
    if has("pom.xml") {
        push_new(found, "mvn -q -B test".into(), "pom.xml");
    } else if has("gradlew") || has("gradlew.bat") {
        push_new(found, if cfg!(windows) { "gradlew.bat test" } else { "./gradlew test" }.into(), "Gradle wrapper");
    } else if has("build.gradle") || has("build.gradle.kts") {
        push_new(found, "gradle test".into(), "build.gradle");
    }
}

/// Adds a suggestion unless the same command is already there.
fn push_new(found: &mut Vec<DetectedCheck>, command: String, source: &str) {
    if !found.iter().any(|c| c.command == command) {
        found.push(DetectedCheck { command, source: source.into() });
    }
}

/// Folders a repository's own code is not in.
const NOT_CODE: &[&str] = &["node_modules", ".git", "target", "dist", "build", ".venv", "venv", "vendor"];

/// Whether a file with one of `extensions` is in the folder or `depth - 1` levels below it.
fn has_files(dir: &Path, extensions: &[&str], depth: u32) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    entries.flatten().any(|e| {
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            depth > 1 && !NOT_CODE.contains(&name.as_str()) && has_files(&path, extensions, depth - 1)
        } else {
            path.extension().is_some_and(|x| extensions.iter().any(|e| x.eq_ignore_ascii_case(e)))
        }
    })
}

/// Test files `node --test` runs by itself: `*.test.js` (and `.mjs`, `.cjs`) or files in a `test` folder.
fn has_js_tests(dir: &Path) -> bool {
    let named = |p: &Path| {
        let n = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        [".test.js", ".test.mjs", ".test.cjs", "-test.js", "_test.js"].iter().any(|s| n.ends_with(s))
    };
    let walk = |d: &Path| std::fs::read_dir(d).into_iter().flatten().flatten().any(|e| named(&e.path()));
    walk(dir)
        || ["test", "tests", "src", "lib"].iter().any(|f| walk(&dir.join(f)))
        || has_files(&dir.join("test"), &["js", "mjs", "cjs"], 1)
}

/// Python tests `pytest` or `unittest` find: `test_*.py` or `*_test.py` up to three folders down (a package's own
/// `tests` folder included).
fn has_python_tests(dir: &Path) -> bool {
    fn find(dir: &Path, depth: u32) -> bool {
        let Ok(entries) = std::fs::read_dir(dir) else { return false };
        entries.flatten().any(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if e.path().is_dir() {
                depth > 1 && !NOT_CODE.contains(&name.as_str()) && find(&e.path(), depth - 1)
            } else {
                name.ends_with(".py") && (name.starts_with("test_") || name.ends_with("_test.py"))
            }
        })
    }
    find(dir, 3)
}

/// What passing a check means, and what it leaves unchecked (ROADMAP focus item 3), from its command: shown beside
/// every check, suggested or approved, so a pass is never read as "correct".
pub fn covers(command: &str) -> &'static str {
    let c = command.trim().to_ascii_lowercase();
    let ends = |s: &str| c.ends_with(s);
    if c.contains(" install") || c == "npm ci" {
        "Installs the dependencies in the step's copy, so the checks after it can run; it checks nothing itself."
    } else if c.contains("compileall") || c.starts_with("node --check") {
        "Smoke check: every file parses. Nothing runs, so behaviour is unchecked."
    } else if c.contains("tsc --noemit") || ends(" typecheck") {
        "Types compile. Nothing runs, so behaviour is unchecked."
    } else if ends(" lint") {
        "Lint rules pass. Nothing runs, so behaviour is unchecked."
    } else if ends(" build") || c == "cargo check" || c.starts_with("go vet") {
        "The code builds. Nothing runs, so behaviour is unchecked."
    } else if ends(" test")
        || ends("test ./...")
        || c.contains("pytest")
        || c.contains("unittest")
        || c == "node --test"
    {
        "Runs the project's tests: it checks what they test. Code they do not reach is unchecked."
    } else {
        "Checks what this command checks; Pygg does not know what it covers."
    }
}

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub command: String,
    pub passed: bool,
    /// `exited 1`, `timed out`, `canceled`, `could not start: …`.
    pub detail: String,
    /// The end of the combined output, redacted.
    pub output_tail: String,
    pub duration_ms: u64,
}

const TAIL_CHARS: usize = 4000;

/// Runs one check command in `dir`. Passing means exit code 0, nothing else.
pub async fn run(
    command: &str, dir: &Path, timeout: Duration, secrets: &[&str], cancel: impl std::future::Future<Output = ()>,
) -> CheckResult {
    let spec = Spec::shell(command, dir, timeout);
    let outcome = process::run(&spec, cancel).await;
    use crate::messages::{self as m, fill};
    let detail = match &outcome.exit {
        Exit::Code(code) => fill(m::EXITED, &[code]),
        Exit::Abnormal => m::ABNORMAL.into(),
        Exit::TimedOut => m::TIMED_OUT.into(),
        Exit::Canceled => m::CANCELED.into(),
        Exit::FailedToStart(e) => fill(m::COULD_NOT_START, &[e]),
    };
    let combined = format!("{}{}", outcome.stdout, outcome.stderr);
    let tail: String = {
        let chars: Vec<char> = combined.chars().collect();
        chars[chars.len().saturating_sub(TAIL_CHARS)..].iter().collect()
    };
    CheckResult {
        command: command.to_string(),
        passed: outcome.succeeded(),
        detail,
        output_tail: redact(&tail, secrets),
        duration_ms: outcome.duration.as_millis() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_package_scripts_and_cargo() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{
            "test":"echo \"Error: no test specified\" && exit 1",
            "typecheck":"tsc --noEmit", "build":"vite build", "dev":"vite"}}"#,
        )
        .unwrap();
        fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        let commands: Vec<String> = detect(dir.path()).into_iter().map(|c| c.command).collect();
        assert_eq!(
            commands,
            ["pnpm install --frozen-lockfile", "pnpm typecheck", "pnpm build", "cargo check", "cargo test"]
        );
    }

    #[test]
    fn a_thin_repository_gets_its_languages_own_checks() {
        let dir = tempfile::tempdir().unwrap();
        // A TypeScript package whose test script is npm's placeholder, with test files no script runs.
        fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"test":"echo \"Error: no test specified\" && exit 1"},"devDependencies":{"typescript":"5"}}"#,
        )
        .unwrap();
        fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();
        fs::write(dir.path().join("package-lock.json"), "{}").unwrap();
        fs::write(dir.path().join("sum.test.js"), "").unwrap();
        // Python with a tests folder and pytest in its requirements, Go, and a Makefile test target.
        fs::create_dir(dir.path().join("tests")).unwrap();
        fs::write(dir.path().join("tests").join("test_app.py"), "").unwrap();
        fs::write(dir.path().join("requirements.txt"), "pytest\n").unwrap();
        fs::write(dir.path().join("go.mod"), "module x").unwrap();
        fs::write(dir.path().join("Makefile"), "test:\n\tgo test ./...\n").unwrap();
        let python = if cfg!(windows) { "python" } else { "python3" };
        let commands: Vec<String> = detect(dir.path()).into_iter().map(|c| c.command).collect();
        assert_eq!(
            commands,
            [
                "npm ci".to_string(),
                "npx --no-install tsc --noEmit".into(),
                "node --test".into(),
                format!("{python} -m compileall -q ."),
                format!("{python} -m pytest -q"),
                "go vet ./...".into(),
                "go test ./...".into(),
                "make test".into(),
            ]
        );
        // Every suggestion says what it covers; none is called a proof of correctness.
        for c in &commands {
            assert!(!covers(c).starts_with("Checks what this command checks"), "{c}");
        }
        assert!(covers("python -m compileall -q .").starts_with("Smoke check"));
        assert!(covers("./run-my-checks.sh").contains("does not know"));
    }

    /// ROADMAP focus item 3's proof: what Pygg suggests for each folder in `PYGG_DETECT_DIRS` (separated by `;`), one
    /// JSON line per folder with each suggestion and what it covers. Reads files only; runs nothing.
    ///
    ///     PYGG_DETECT_DIRS="a;b" cargo test --lib detect_proof -- --ignored --nocapture
    #[test]
    #[ignore]
    fn detect_proof() {
        for dir in std::env::var("PYGG_DETECT_DIRS").unwrap_or_default().split(';').filter(|d| !d.is_empty()) {
            let found: Vec<serde_json::Value> = detect(Path::new(dir))
                .into_iter()
                .map(|c| serde_json::json!({"command": c.command, "source": c.source, "covers": covers(&c.command)}))
                .collect();
            println!("PROOF {}", serde_json::json!({"dir": dir, "checks": found}));
        }
    }

    #[test]
    fn nothing_detected_in_an_empty_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect(dir.path()).is_empty());
    }

    #[test]
    fn runs_a_check_and_redacts_its_output() {
        let dir = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let ok = rt.block_on(run(
            "echo key=sk-0123456789abcdefghijkl",
            dir.path(),
            Duration::from_secs(20),
            &[],
            std::future::pending(),
        ));
        assert!(ok.passed, "{ok:?}");
        assert!(ok.output_tail.contains("[redacted]") && !ok.output_tail.contains("sk-0123"));
        let failed = rt.block_on(run("exit 2", dir.path(), Duration::from_secs(20), &[], std::future::pending()));
        assert!(!failed.passed);
        assert_eq!(failed.detail, "exited 2");
    }
}
