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
    found
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
