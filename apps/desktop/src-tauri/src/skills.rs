//! Skills & Prompts (ADR-039): a library of skills in the open Agent Skills format (a folder holding `SKILL.md`: YAML
//! front matter with `name` and `description`, then the instructions), installed into the coding agents' own skill
//! folders, and a measure of the context each agent sends with every request before any work is done.
//!
//! - **Where skills go.** Claude Code reads `~/.claude/skills` (or `$CLAUDE_CONFIG_DIR/skills`); Codex reads
//!   `~/.agents/skills`; OpenCode reads both and `~/.config/opencode/skills`. Pygg installs into the first two, and into
//!   a project's own `.claude/skills` and `.agents/skills` for a project folder the user added (`skill_projects`).
//! - **What an agent loads.** Each agent keeps only a skill's name and description in context and reads the
//!   instructions when the skill is used. A skill marked `disable-model-invocation: true` (a "prompt": run only when the
//!   user types `/name`) is left out of Claude Code's and OpenCode's list, so it costs nothing until used. Codex has its
//!   own switch: `agents/openai.yaml` with `policy.allow_implicit_invocation: false` keeps a skill out of the model's
//!   context until it is called as `$name`.
//! - **What Pygg writes.** A `SKILL.md` whose front matter carries `metadata.pygg-hash`, a hash of the rest of the file,
//!   and for a prompt in the shared folder Codex reads, Codex's `agents/openai.yaml` as well. Pygg never overwrites a
//!   skill folder it did not write, and never removes a file changed since it wrote it.
//! - **What Pygg reads.** The agents' global skill, command and prompt folders and their global instruction files
//!   (`CLAUDE.md`, `AGENTS.md`), and the same at the root of each project the user added. Never a project it was not
//!   given, and never the folders below a project's root.
//!
//! Nothing here downloads anything or runs a script: a skill's folder gets its `SKILL.md` and, for a prompt Codex can
//! see, `agents/openai.yaml`.

use crate::db::now_ms;
use crate::gateway::estimate_text_tokens;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, String>;

/// The agents' folders, following their own variables.
#[derive(Clone, Debug)]
pub struct Homes {
    /// Claude Code's folder: `$CLAUDE_CONFIG_DIR` or `~/.claude`.
    pub claude: PathBuf,
    /// The shared Agent Skills folder Codex and OpenCode read: `~/.agents`.
    pub agents: PathBuf,
    /// Codex's folder: `$CODEX_HOME` or `~/.codex`.
    pub codex: PathBuf,
    /// OpenCode's folder: `$XDG_CONFIG_HOME/opencode` or `~/.config/opencode`.
    pub opencode: PathBuf,
}

impl Homes {
    pub fn from_env() -> Homes {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        let home = var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap_or_default();
        Homes {
            claude: var("CLAUDE_CONFIG_DIR").unwrap_or_else(|| home.join(".claude")),
            agents: home.join(".agents"),
            codex: var("CODEX_HOME").unwrap_or_else(|| home.join(".codex")),
            opencode: var("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")).join("opencode"),
        }
    }

    /// The skills folder Pygg installs into for a target.
    pub fn skills_dir(&self, target: Target) -> PathBuf {
        match target {
            Target::ClaudeCode => self.claude.join("skills"),
            Target::Agents => self.agents.join("skills"),
        }
    }
}

/// Where a skill can be installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    /// `~/.claude/skills`: Claude Code (OpenCode reads it too).
    ClaudeCode,
    /// `~/.agents/skills`: Codex and OpenCode.
    Agents,
}

impl Target {
    pub const ALL: [Target; 2] = [Target::ClaudeCode, Target::Agents];

    pub fn parse(s: &str) -> Result<Target> {
        match s {
            "claude-code" | "claude" => Ok(Target::ClaudeCode),
            "agents" | "codex" | "opencode" => Ok(Target::Agents),
            other => Err(format!("unknown target {other}: use claude-code or agents (Codex and OpenCode)")),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Target::ClaudeCode => "claude-code",
            Target::Agents => "agents",
        }
    }
}

/// A starter skill written for Pygg (MIT, like the rest of it).
struct Starter {
    id: &'static str,
    description: &'static str,
    manual: bool,
    body: &'static str,
}

/// Pygg's starter skills. Each is short, so the instructions cost little when used, and all are prompts (run with
/// `/name`), so none adds to an agent's context until it is used.
const STARTERS: &[Starter] = &[
    Starter {
        id: "brain-v2",
        description: "What pygg/brain asks the planner (ADR-041): clarifying questions and a board of small cards, one \
                      role and one done criterion each, as JSON. Pygg sends it for you; run it by hand to see or test it.",
        manual: true,
        body: "You are the planner behind pygg/brain. Turn the requirement into a board of small cards for a Kanban \
               run one card at a time, each card done by one coding-agent attempt and judged by the repository's \
               checks.\n\n\
               Reply with JSON only, no prose and no code fence, in exactly this shape:\n\
               {\"questions\": [{\"id\": \"q1\", \"text\": \"...\"}],\n\
               \x20\"cards\": [{\"id\": \"c1\", \"title\": \"...\", \"role\": \"plan|code|test|review\", \
               \"done\": \"...\", \"dependsOn\": [\"q1\", \"c1\"]}]}\n\n\
               Rules:\n\
               - Questions: only those whose answer would change the cards, at most seven. None is fine.\n\
               - Cards: small enough for one attempt and one set of checks; at most forty. Each has one role and one \
               done criterion a check can test (a command that passes, a page that renders, a file that exists).\n\
               - Each card is judged by the repository's checks right after its attempt, and the checks must pass: \
               every card leaves the project working and its tests passing. Put a test in the same card as the code it \
               tests, or in a later card; never a test card before the code it tests.\n\
               - dependsOn names the questions a card needs answered and the cards it needs finished. No cycles.\n\
               - Roles: plan (a written design the other cards cite), code, test, review. Never name a model, a \
               price, a time or a story point: Pygg picks the model for each role by its own rules.\n\
               - Say nothing about files you have not seen; the requirement is all you have.\n\
               - When you are given your previous proposal and the user's answers, return the whole updated board, \
               and leave out every question the user has answered.\n",
    },
    Starter {
        id: "brief-answers",
        description: "Answer with as few output tokens as the task allows for the rest of the session: no restating, \
                      no recap, changed code as diffs.",
        manual: true,
        body: "For the rest of this session, keep answers short. Output tokens cost the most.\n\n\
               - Do not restate the question or the plan you were given.\n\
               - Show changed code as a unified diff or the edited lines only, never a whole unchanged file.\n\
               - No closing summary of what you did unless asked; one line saying what changed is enough.\n\
               - Ask one question only when you cannot go on without the answer.\n\
               - Keep explanations to the reason a reader could not see from the code.\n",
    },
    Starter {
        id: "fix-failing-test",
        description: "Find and fix the cause of a failing test with the smallest change, without editing the test.",
        manual: true,
        body: "Fix the failing test named in $ARGUMENTS (or the first failing one if none is named).\n\n\
               1. Run only that test and read the failure.\n\
               2. Find the cause in the code under test. Read only the files the failure points to.\n\
               3. Make the smallest change that fixes the cause. Do not edit, skip or weaken the test.\n\
               4. Run the test again, then the tests of the module you changed.\n\
               5. Reply with the cause in one sentence and the diff.\n",
    },
    Starter {
        id: "review-diff",
        description: "Review the current changes for correctness bugs only, with file and line for each finding.",
        manual: true,
        body: "Review the uncommitted changes (or the branch's changes against main if there are none).\n\n\
               - Look for defects: wrong logic, missed cases, errors swallowed, races, security holes, broken callers.\n\
               - Skip style, naming and formatting.\n\
               - For each finding: `path:line`, what goes wrong, and the input that makes it go wrong.\n\
               - Rank by severity. Say \"no findings\" when there are none; do not invent any.\n",
    },
    Starter {
        id: "write-tests",
        description: "Write focused tests for the named code: behaviour and edge cases, matching the project's style.",
        manual: true,
        body: "Write tests for $ARGUMENTS.\n\n\
               1. Read the code and one existing test file next to it, and follow its style and helpers.\n\
               2. Cover the main behaviour, each error path, and the edges (empty, one, many, limits).\n\
               3. One behaviour per test, named for what it checks.\n\
               4. Run the new tests. If one fails because the code is wrong, say so instead of changing the test to pass.\n",
    },
    Starter {
        id: "plan-first",
        description: "Plan a change before editing: the files, the steps and the risks, then wait for approval.",
        manual: true,
        body: "Before changing anything for $ARGUMENTS:\n\n\
               1. Read what you need to understand the change, no more.\n\
               2. Reply with a plan: the files to change and why, the steps in order, how you will check it, and the \
               risks.\n\
               3. Stop and wait for approval. Do not edit files until the plan is approved.\n",
    },
    Starter {
        id: "explain-error",
        description: "Explain an error message or stack trace: the cause, where it starts, and the fix.",
        manual: true,
        body: "Explain the error in $ARGUMENTS (or the last error in this session).\n\n\
               - The cause in plain words, in one or two sentences.\n\
               - Where it starts: the first frame or line in this project's code.\n\
               - The fix, as a diff if it is small. If the cause is unclear, the one check that would settle it.\n",
    },
    Starter {
        id: "commit-message",
        description: "Write a commit message for the staged changes in the project's own commit style.",
        manual: true,
        body: "Write a commit message for the staged changes.\n\n\
               1. Read `git diff --staged` and the last ten commit subjects (`git log --oneline -10`) for the style.\n\
               2. A subject under 72 characters that says what changes for a user or a caller.\n\
               3. A short body only if the reason is not obvious from the diff.\n\
               4. Print the message; do not commit.\n",
    },
    Starter {
        id: "handoff-note",
        description: "Write a short handoff note so a fresh session or another model can continue this work.",
        manual: true,
        body: "Write a handoff note for this session, under 300 words, so a new session can continue without the \
               history (a long history is re-sent with every request).\n\n\
               - Goal: what the user asked for, in their words where it matters.\n\
               - Done: what is finished and checked.\n\
               - State: files changed, branch, anything uncommitted.\n\
               - Next: the remaining steps in order, and the command that checks each.\n\
               - Decisions and constraints a newcomer would get wrong.\n\n\
               Print the note only.\n",
    },
];

/// A skill in the library, with where it is installed.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub description: String,
    pub body: String,
    /// Run only when the user types `/id` (`disable-model-invocation: true`).
    pub manual: bool,
    /// `pygg` for a starter, `mine` for the user's own.
    pub origin: &'static str,
    pub updated_at: Option<i64>,
    pub installs: Vec<Install>,
    /// What the skill adds to every request where it is listed: its name and description, or nothing for a prompt.
    pub listed_tokens: u64,
    /// What its instructions cost when it is used.
    pub body_tokens: u64,
}

/// A skill's state in one target folder.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    pub target: Target,
    /// `installed` (Pygg's copy, as in the library), `outdated` (Pygg's copy of an earlier version), `changed` (Pygg's
    /// copy, edited since), `other` (a skill of that name that Pygg did not write) or `absent`.
    pub state: &'static str,
    pub path: String,
    /// The project folder, for an install into a project's own skills folder; none for the agents' global folders.
    pub project: Option<String>,
}

/// A skill as the user saves it.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEdit {
    pub id: String,
    pub description: String,
    pub body: String,
    pub manual: bool,
}

/// What a `SKILL.md` (or a command or prompt file) says about itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parsed {
    pub name: Option<String>,
    pub description: Option<String>,
    pub manual: bool,
    pub pygg_hash: Option<String>,
    pub body: String,
}

/// A name the Agent Skills format accepts: lowercase letters, digits and single hyphens, at most 64 characters.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .split('-')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

fn check(edit: &SkillEdit) -> Result<()> {
    if !valid_id(&edit.id) {
        return Err("A skill's name is lowercase letters, digits and single hyphens, at most 64 characters, such \
                    as review-diff."
            .into());
    }
    let description = edit.description.trim();
    if description.is_empty() {
        return Err("Describe what the skill does and when to use it: the agent reads only this to choose it.".into());
    }
    if description.chars().count() > 1024 {
        return Err("Keep the description under 1,024 characters (the Agent Skills limit).".into());
    }
    if edit.body.trim().is_empty() {
        return Err("Write the skill's instructions.".into());
    }
    if edit.body.chars().count() > 100_000 {
        return Err("The instructions are over 100,000 characters; split them into smaller skills.".into());
    }
    Ok(())
}

fn hash_of(text: &str) -> String {
    let unhashed: String = text
        .replace("\r\n", "\n")
        .lines()
        .filter(|l| !l.trim_start().starts_with("pygg-hash:"))
        .map(|l| format!("{l}\n"))
        .collect();
    let digest = Sha256::digest(unhashed.as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// A description as one double-quoted YAML line.
fn yaml_quoted(s: &str) -> String {
    let one_line = s.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("\"{}\"", one_line.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The `agents/openai.yaml` Pygg writes beside a prompt in the shared folder, so that Codex leaves it out of the model's
/// context until it is called by name (Codex's `SkillPolicy`; it ignores `disable-model-invocation`). Pygg owns the file
/// only while it reads exactly this.
const CODEX_POLICY: &str =
    "# Written by Pygg: Codex lists this skill only when it is called by name.\npolicy:\n  allow_implicit_invocation: false\n";

/// Codex's settings file in a skill's folder.
fn codex_policy_path(skill_dir: &Path) -> PathBuf {
    skill_dir.join("agents").join("openai.yaml")
}

/// The `agents/openai.yaml` Pygg's copy of a skill has in a target, if any.
fn expected_policy(target: Target, manual: bool) -> Option<&'static str> {
    (manual && target == Target::Agents).then_some(CODEX_POLICY)
}

/// Whether an `agents/openai.yaml` sets `policy.allow_implicit_invocation: false`, read line by line as Pygg reads
/// front matter.
fn implicit_invocation_off(yaml: &str) -> bool {
    let mut in_policy = false;
    for line in yaml.replace("\r\n", "\n").lines() {
        let content = line.split(" #").next().unwrap_or("").trim_end();
        if content.trim().is_empty() || content.trim_start().starts_with('#') {
            continue;
        }
        if !content.starts_with(' ') && !content.starts_with('\t') {
            in_policy = content.trim() == "policy:";
            continue;
        }
        if let Some(v) = content.trim().strip_prefix("allow_implicit_invocation:") {
            if in_policy && unquote(v).eq_ignore_ascii_case("false") {
                return true;
            }
        }
    }
    false
}

/// The `SKILL.md` Pygg writes for a skill.
pub fn render(id: &str, description: &str, body: &str, manual: bool) -> String {
    let mut text = format!("---\nname: {id}\ndescription: {}\n", yaml_quoted(description));
    if manual {
        text.push_str("disable-model-invocation: true\n");
    }
    text.push_str("metadata:\n  pygg-hash: \n---\n\n");
    text.push_str(body.trim_end());
    text.push('\n');
    let hash = hash_of(&text);
    text.replacen("  pygg-hash: \n", &format!("  pygg-hash: {hash}\n"), 1)
}

fn unquote(value: &str) -> String {
    let v = value.trim();
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        let inner = &v[1..v.len() - 1];
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => {}
                }
            } else {
                out.push(c);
            }
        }
        out
    } else if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
        v[1..v.len() - 1].replace("''", "'")
    } else {
        v.to_string()
    }
}

/// Reads the front matter Pygg needs (`name`, `description`, `disable-model-invocation`, `metadata.pygg-hash`) and the
/// body. Plain, quoted and block (`|`, `>`) values; anything else in the front matter is left alone.
pub fn parse(text: &str) -> Parsed {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text).replace("\r\n", "\n");
    let mut parsed = Parsed::default();
    let Some(rest) = text.strip_prefix("---\n") else {
        parsed.body = text.to_string();
        return parsed;
    };
    let (front, body) = match rest.find("\n---\n") {
        Some(i) => (&rest[..i], &rest[i + 5..]),
        None => match rest.strip_suffix("\n---") {
            Some(front) => (front, ""),
            None => {
                parsed.body = text.to_string();
                return parsed;
            }
        },
    };
    parsed.body = body.trim_start_matches('\n').to_string();
    let lines: Vec<&str> = front.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(v) = line.trim().strip_prefix("pygg-hash:") {
                parsed.pygg_hash = Some(v.trim().to_string()).filter(|v| !v.is_empty());
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim();
        let value = if value.starts_with('|') || value.starts_with('>') {
            let mut block = Vec::new();
            while i < lines.len() && (lines[i].starts_with(' ') || lines[i].starts_with('\t') || lines[i].is_empty()) {
                block.push(lines[i].trim());
                i += 1;
            }
            if value.starts_with('>') {
                block.join(" ").trim().to_string()
            } else {
                block.join("\n").trim().to_string()
            }
        } else {
            unquote(value)
        };
        match key.trim() {
            "name" => parsed.name = Some(value),
            "description" => parsed.description = Some(value),
            "disable-model-invocation" => parsed.manual = value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }
    parsed
}

/// Whether Pygg wrote this file and it is unchanged since.
fn ours_unchanged(text: &str) -> Option<bool> {
    let hash = parse(text).pygg_hash?;
    Some(hash == hash_of(text))
}

fn listed_tokens(id: &str, description: &str, manual: bool) -> u64 {
    if manual {
        0
    } else {
        estimate_text_tokens(&format!("{id}: {description}"))
    }
}

/// Codex's settings file in a skill's folder: absent, Pygg's (as written), or someone else's or edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Policy {
    Absent,
    Pygg,
    Other,
}

fn policy_state(skill_dir: &Path) -> Policy {
    match std::fs::read_to_string(codex_policy_path(skill_dir)) {
        Err(_) => Policy::Absent,
        Ok(text) if text.replace("\r\n", "\n") == CODEX_POLICY => Policy::Pygg,
        Ok(_) => Policy::Other,
    }
}

/// The skills folder Pygg installs into: an agent's global folder, or a project's own.
fn skills_dir(homes: &Homes, project: Option<&Path>, target: Target) -> PathBuf {
    match (project, target) {
        (None, target) => homes.skills_dir(target),
        (Some(root), Target::ClaudeCode) => root.join(".claude").join("skills"),
        (Some(root), Target::Agents) => root.join(".agents").join("skills"),
    }
}

fn install_state(
    homes: &Homes, project: Option<&Path>, target: Target, id: &str, expected: &str, manual: bool,
) -> Install {
    let dir = skills_dir(homes, project, target).join(id);
    let path = dir.join("SKILL.md");
    let state = match std::fs::read_to_string(&path) {
        Err(_) if !dir.exists() => "absent",
        Err(_) => "other",
        Ok(text) => match (ours_unchanged(&text), policy_state(&dir), expected_policy(target, manual)) {
            (None, ..) => "other",
            (Some(false), ..) => "changed",
            // Codex's file, edited or someone else's, where Pygg would write its own: left alone.
            (Some(true), Policy::Other, Some(_)) => "changed",
            (Some(true), policy, wanted)
                if text.replace("\r\n", "\n") == expected && (policy == Policy::Pygg) == wanted.is_some() =>
            {
                "installed"
            }
            (Some(true), ..) => "outdated",
        },
    };
    Install { target, state, path: path.display().to_string(), project: project.map(|p| p.display().to_string()) }
}

#[allow(clippy::too_many_arguments)]
fn skill(
    homes: &Homes, projects: &[PathBuf], id: &str, description: &str, body: &str, manual: bool, origin: &'static str,
    updated: Option<i64>,
) -> Skill {
    let expected = render(id, description, body, manual);
    let places = std::iter::once(None).chain(projects.iter().map(|p| Some(p.as_path())));
    Skill {
        id: id.to_string(),
        description: description.to_string(),
        body: body.to_string(),
        manual,
        origin,
        updated_at: updated,
        installs: places
            .flat_map(|project| Target::ALL.into_iter().map(move |target| (project, target)))
            .map(|(project, target)| install_state(homes, project, target, id, &expected, manual))
            .collect(),
        listed_tokens: listed_tokens(id, description, manual),
        body_tokens: estimate_text_tokens(body),
    }
}

fn starter(id: &str) -> Option<&'static Starter> {
    STARTERS.iter().find(|s| s.id == id)
}

/// A starter's instructions, for the gateway to send as its own prompt (`brain-v2`, ADR-041).
pub fn starter_body(id: &str) -> Option<&'static str> {
    starter(id).map(|s| s.body)
}

fn mine(conn: &Connection, id: &str) -> Result<Option<(String, String, bool, i64)>> {
    conn.query_row("SELECT description, body, manual, updated_at FROM skills WHERE id = ?1", [id], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0, r.get(3)?))
    })
    .optional()
    .map_err(|e| e.to_string())
}

/// The library: Pygg's starters, then the user's own skills by name.
pub fn library(conn: &Connection, homes: &Homes) -> Result<Vec<Skill>> {
    let projects = project_paths(conn)?;
    let mut out: Vec<Skill> =
        STARTERS.iter().map(|s| skill(homes, &projects, s.id, s.description, s.body, s.manual, "pygg", None)).collect();
    let mut stmt = conn
        .prepare("SELECT id, description, body, manual, updated_at FROM skills ORDER BY id")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get(4)?))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (id, description, body, manual, updated): (String, String, String, i64, i64) =
            row.map_err(|e| e.to_string())?;
        out.push(skill(homes, &projects, &id, &description, &body, manual != 0, "mine", Some(updated)));
    }
    Ok(out)
}

/// One skill of the library.
pub fn get(conn: &Connection, homes: &Homes, id: &str) -> Result<Skill> {
    let projects = project_paths(conn)?;
    if let Some(s) = starter(id) {
        return Ok(skill(homes, &projects, s.id, s.description, s.body, s.manual, "pygg", None));
    }
    let (description, body, manual, updated) = mine(conn, id)?.ok_or_else(|| format!("There is no skill {id}."))?;
    Ok(skill(homes, &projects, id, &description, &body, manual, "mine", Some(updated)))
}

/// Saves one of the user's skills, new or edited. A starter's name is Pygg's: duplicate it under another name to change
/// it. An installed copy is not rewritten here; the library shows it as outdated until it is installed again.
pub fn save(conn: &Connection, homes: &Homes, edit: &SkillEdit) -> Result<Skill> {
    check(edit)?;
    if starter(&edit.id).is_some() {
        return Err(format!("{} is one of Pygg's starter skills; save your version under another name.", edit.id));
    }
    let now = now_ms();
    conn.execute(
        "INSERT INTO skills (id, description, body, manual, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5) \
         ON CONFLICT(id) DO UPDATE SET description = ?2, body = ?3, manual = ?4, updated_at = ?5",
        params![edit.id, edit.description.trim(), edit.body.trim_end(), edit.manual as i64, now],
    )
    .map_err(|e| e.to_string())?;
    get(conn, homes, &edit.id)
}

/// Deletes one of the user's skills. Its installed copies must be removed first, so none is left behind unmanaged.
pub fn delete(conn: &Connection, homes: &Homes, id: &str) -> Result<()> {
    if starter(id).is_some() {
        return Err(format!("{id} is one of Pygg's starter skills and cannot be deleted."));
    }
    let skill = get(conn, homes, id)?;
    if let Some(i) = skill.installs.iter().find(|i| matches!(i.state, "installed" | "outdated" | "changed")) {
        return Err(format!("{id} is installed in {}; remove it there first.", i.path));
    }
    conn.execute("DELETE FROM skills WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

fn write_atomically(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    }
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".pygg-tmp");
    let tmp = path.with_file_name(tmp_name);
    std::fs::write(&tmp, text).map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Could not replace {}: {e}", path.display())
    })
}

/// Writes the skill's `SKILL.md` into the target's skills folder, globally or in a project the user added, or brings
/// Pygg's earlier copy up to date. A folder of that name that Pygg did not write, or a copy edited since, is left alone.
pub fn install(conn: &Connection, homes: &Homes, id: &str, target: Target, project: Option<&str>) -> Result<Skill> {
    let root = listed_project(conn, project, true)?;
    let skill = get(conn, homes, id)?;
    let install = skill
        .installs
        .iter()
        .find(|i| i.target == target && i.project.as_deref() == project)
        .expect("one state per target and place");
    let folder = skills_dir(homes, root.as_deref(), target);
    match install.state {
        "other" => {
            return Err(format!(
                "{} already holds a skill named {id} that Pygg did not write; it was left as it is.",
                folder.display()
            ))
        }
        "changed" => {
            return Err(format!(
                "{} was edited after Pygg installed it; it was left as it is. Remove or rename it to install again.",
                install.path
            ))
        }
        "installed" => return Ok(skill),
        _ => {}
    }
    let dir = folder.join(id);
    // Codex's file first: a prompt is never left visible to Codex because the second write failed.
    match (expected_policy(target, skill.manual), policy_state(&dir)) {
        (Some(text), _) => write_atomically(&codex_policy_path(&dir), text)?,
        (None, Policy::Pygg) => remove_policy(&dir)?,
        (None, _) => {}
    }
    let text = render(&skill.id, &skill.description, &skill.body, skill.manual);
    write_atomically(Path::new(&install.path), &text)?;
    get(conn, homes, id)
}

/// Removes Pygg's `agents/openai.yaml`, and the `agents` folder when nothing else is in it.
fn remove_policy(skill_dir: &Path) -> Result<()> {
    let path = codex_policy_path(skill_dir);
    std::fs::remove_file(&path).map_err(|e| format!("Could not remove {}: {e}", path.display()))?;
    let _ = std::fs::remove_dir(skill_dir.join("agents"));
    Ok(())
}

/// Removes Pygg's copy from the target, if it is unchanged; its folder goes too when nothing else is in it.
pub fn uninstall(conn: &Connection, homes: &Homes, id: &str, target: Target, project: Option<&str>) -> Result<Skill> {
    if !valid_id(id) {
        return Err(format!("There is no skill {id}."));
    }
    let root = listed_project(conn, project, false)?;
    let dir = skills_dir(homes, root.as_deref(), target).join(id);
    let path = dir.join("SKILL.md");
    let Ok(text) = std::fs::read_to_string(&path) else { return get(conn, homes, id) };
    match ours_unchanged(&text) {
        None => return Err(format!("{} was not written by Pygg; it was left as it is.", path.display())),
        Some(false) => {
            return Err(format!("{} was edited after Pygg installed it; it was left as it is.", path.display()))
        }
        Some(true) => {}
    }
    let policy = policy_state(&dir);
    if policy == Policy::Other && expected_policy(target, parse(&text).manual).is_some() {
        return Err(format!(
            "{} was edited after Pygg installed it; it was left as it is.",
            codex_policy_path(&dir).display()
        ));
    }
    if policy == Policy::Pygg {
        remove_policy(&dir)?;
    }
    std::fs::remove_file(&path).map_err(|e| format!("Could not remove {}: {e}", path.display()))?;
    let _ = std::fs::remove_dir(&dir);
    get(conn, homes, id)
}

/// One thing an agent may send with every request.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextItem {
    /// `instructions` (a global CLAUDE.md or AGENTS.md, sent whole), `skill`, `command` (Claude Code) or `prompt`
    /// (Codex, sent only when typed).
    pub kind: &'static str,
    pub name: String,
    pub path: String,
    /// Estimated tokens added to every request: the whole file for instructions, the name and description for a
    /// listed skill or command, nothing for a prompt or a skill run only by name.
    pub tokens: u64,
    /// Estimated tokens of the whole file, what it costs when used.
    pub full_tokens: u64,
    pub manual: bool,
    /// Written by Pygg and unchanged since.
    pub pygg: bool,
    /// A skill whose name is in the library.
    pub in_library: bool,
}

/// What one agent sends with every request, from its global files.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContext {
    pub agent: &'static str,
    pub label: &'static str,
    pub always_on_tokens: u64,
    pub items: Vec<ContextItem>,
}

fn instructions(path: PathBuf) -> Option<ContextItem> {
    let text = std::fs::read_to_string(&path).ok()?;
    let tokens = estimate_text_tokens(&text);
    Some(ContextItem {
        kind: "instructions",
        name: path.file_name()?.to_string_lossy().into_owned(),
        path: path.display().to_string(),
        tokens,
        full_tokens: tokens,
        manual: false,
        pygg: false,
        in_library: false,
    })
}

/// How an agent decides that a skill is run only by name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RunByName {
    /// `disable-model-invocation: true` in `SKILL.md` (Claude Code, OpenCode).
    FrontMatter,
    /// `policy.allow_implicit_invocation: false` in `agents/openai.yaml` (Codex).
    CodexPolicy,
}

/// The skills in a folder (`<name>/SKILL.md`), each costing its name and description unless the agent leaves it out.
fn skills_in(dir: &Path, rule: RunByName, library: &[String]) -> Vec<ContextItem> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path().join("SKILL.md");
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let parsed = parse(&text);
        let name = entry.file_name().to_string_lossy().into_owned();
        let manual = match rule {
            RunByName::FrontMatter => parsed.manual,
            RunByName::CodexPolicy => std::fs::read_to_string(codex_policy_path(&entry.path()))
                .is_ok_and(|yaml| implicit_invocation_off(&yaml)),
        };
        let description = parsed.description.clone().unwrap_or_default();
        out.push(ContextItem {
            kind: "skill",
            tokens: if description.is_empty() && rule == RunByName::FrontMatter {
                0
            } else {
                listed_tokens(&name, &description, manual)
            },
            full_tokens: estimate_text_tokens(&text),
            manual,
            pygg: ours_unchanged(&text) == Some(true),
            in_library: library.contains(&name),
            path: path.display().to_string(),
            name,
        });
    }
    out
}

/// Markdown files in a folder: Claude Code's commands (listed with their description) or Codex's prompts (sent only
/// when typed).
fn markdown_in(dir: &Path, kind: &'static str) -> Vec<ContextItem> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let parsed = parse(&text);
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let description = parsed
            .description
            .clone()
            .unwrap_or_else(|| parsed.body.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string());
        let tokens = if kind == "command" { listed_tokens(&name, &description, parsed.manual) } else { 0 };
        out.push(ContextItem {
            kind,
            name,
            path: path.display().to_string(),
            tokens,
            full_tokens: estimate_text_tokens(&text),
            manual: parsed.manual || kind == "prompt",
            pygg: false,
            in_library: false,
        });
    }
    out
}

fn agent(agent: &'static str, label: &'static str, mut items: Vec<ContextItem>) -> AgentContext {
    items.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.name.cmp(&b.name)));
    AgentContext { agent, label, always_on_tokens: items.iter().map(|i| i.tokens).sum(), items }
}

/// The skills OpenCode reads from several folders, a name found twice listed once.
fn skills_once(dirs: impl IntoIterator<Item = PathBuf>, library: &[String]) -> Vec<ContextItem> {
    let mut out: Vec<ContextItem> = Vec::new();
    for dir in dirs {
        for item in skills_in(&dir, RunByName::FrontMatter, library) {
            if !out.iter().any(|i| i.name == item.name) {
                out.push(item);
            }
        }
    }
    out
}

/// The first of these instruction files that exists, as the agent reads only one of them.
fn first_instructions(paths: impl IntoIterator<Item = PathBuf>) -> Option<ContextItem> {
    paths.into_iter().find_map(instructions)
}

/// What each agent sends with every request from its global files, largest first. Estimates lean high, as the
/// gateway's do (three ASCII characters a token).
pub fn context(conn: &Connection, homes: &Homes) -> Result<Vec<AgentContext>> {
    let library: Vec<String> = library(conn, homes)?.into_iter().map(|s| s.id).collect();
    let claude_skills = homes.claude.join("skills");
    let agents_skills = homes.agents.join("skills");
    let mut claude: Vec<ContextItem> = instructions(homes.claude.join("CLAUDE.md")).into_iter().collect();
    claude.extend(skills_in(&claude_skills, RunByName::FrontMatter, &library));
    claude.extend(markdown_in(&homes.claude.join("commands"), "command"));
    let mut codex: Vec<ContextItem> =
        first_instructions(["AGENTS.override.md", "AGENTS.md"].map(|f| homes.codex.join(f))).into_iter().collect();
    codex.extend(skills_in(&agents_skills, RunByName::CodexPolicy, &library));
    codex.extend(markdown_in(&homes.codex.join("prompts"), "prompt"));
    // OpenCode falls back to Claude Code's CLAUDE.md when it has no AGENTS.md of its own.
    let mut opencode: Vec<ContextItem> =
        first_instructions([homes.opencode.join("AGENTS.md"), homes.claude.join("CLAUDE.md")]).into_iter().collect();
    opencode.extend(skills_once([homes.opencode.join("skills"), claude_skills, agents_skills], &library));
    Ok(vec![
        agent("claude-code", "Claude Code", claude),
        agent("codex", "Codex", codex),
        agent("opencode", "OpenCode", opencode),
    ])
}

/// What each agent adds to every request in a project, on top of its global files: the instruction files and skill
/// folders at the project's root. Files in folders below the root, read only when the agent works there, are not
/// counted.
fn project_context(root: &Path, library: &[String]) -> Vec<AgentContext> {
    let mut claude: Vec<ContextItem> = ["CLAUDE.md", ".claude/CLAUDE.md", "CLAUDE.local.md"]
        .iter()
        .filter_map(|f| instructions(root.join(f)))
        .collect();
    claude.extend(skills_in(&root.join(".claude").join("skills"), RunByName::FrontMatter, library));
    claude.extend(markdown_in(&root.join(".claude").join("commands"), "command"));
    let mut codex: Vec<ContextItem> =
        first_instructions(["AGENTS.override.md", "AGENTS.md"].map(|f| root.join(f))).into_iter().collect();
    codex.extend(skills_in(&root.join(".agents").join("skills"), RunByName::CodexPolicy, library));
    let mut opencode: Vec<ContextItem> =
        first_instructions(["AGENTS.md", "CLAUDE.md"].map(|f| root.join(f))).into_iter().collect();
    opencode.extend(skills_once([".opencode", ".claude", ".agents"].map(|d| root.join(d).join("skills")), library));
    vec![
        agent("claude-code", "Claude Code", claude),
        agent("codex", "Codex", codex),
        agent("opencode", "OpenCode", opencode),
    ]
}

/// A project folder the user added, with what each agent adds there and how many of the library's skills Pygg has a
/// copy of in it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub path: String,
    pub name: String,
    pub added_at: i64,
    /// The folder is gone or cannot be read; its rows stay until the user removes them.
    pub missing: bool,
    pub pygg_skills: u64,
    pub agents: Vec<AgentContext>,
}

fn project_paths(conn: &Connection) -> Result<Vec<PathBuf>> {
    let mut stmt =
        conn.prepare("SELECT path FROM skill_projects ORDER BY path COLLATE NOCASE").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
    rows.map(|r| r.map(PathBuf::from).map_err(|e| e.to_string())).collect()
}

/// A project the user added, as stored; anything else is refused, so Pygg writes only where it was told to.
fn listed_project(conn: &Connection, project: Option<&str>, must_exist: bool) -> Result<Option<PathBuf>> {
    let Some(project) = project else { return Ok(None) };
    if !project_paths(conn)?.iter().any(|p| p.as_os_str() == project) {
        return Err(format!("{project} is not one of the projects added to Skills & Prompts."));
    }
    let root = PathBuf::from(project);
    if must_exist && !root.is_dir() {
        return Err(format!("{project} no longer exists."));
    }
    Ok(Some(root))
}

/// The library's skills Pygg has a copy of in a place, edited or not.
fn pygg_copies(skills: &[Skill], project: &str) -> u64 {
    skills
        .iter()
        .filter(|s| {
            s.installs.iter().any(|i| {
                i.project.as_deref() == Some(project) && matches!(i.state, "installed" | "outdated" | "changed")
            })
        })
        .count() as u64
}

/// The projects the user added, by path.
pub fn projects(conn: &Connection, homes: &Homes) -> Result<Vec<Project>> {
    let skills = library(conn, homes)?;
    let names: Vec<String> = skills.iter().map(|s| s.id.clone()).collect();
    let mut stmt = conn
        .prepare("SELECT path, added_at FROM skill_projects ORDER BY path COLLATE NOCASE")
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|(path, added_at)| {
            let root = PathBuf::from(&path);
            let missing = !root.is_dir();
            Project {
                name: root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone()),
                added_at,
                missing,
                pygg_skills: pygg_copies(&skills, &path),
                agents: if missing { Vec::new() } else { project_context(&root, &names) },
                path,
            }
        })
        .collect())
}

/// A path as the user knows it: `canonicalize`'s `\\?\` prefix taken off a drive path on Windows.
fn plain(path: PathBuf) -> PathBuf {
    let text = path.display().to_string();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => path,
    }
}

/// Adds a project folder. Its own skills folders become places to install, and its instruction files are read.
pub fn add_project(conn: &Connection, homes: &Homes, path: &str) -> Result<Vec<Project>> {
    let root = std::fs::canonicalize(path.trim())
        .ok()
        .filter(|p| p.is_dir())
        .map(plain)
        .ok_or_else(|| format!("{path} is not a folder Pygg can read."))?;
    let home = [homes.claude.parent(), homes.agents.parent()]
        .into_iter()
        .flatten()
        .filter_map(|h| std::fs::canonicalize(h).ok().map(plain))
        .any(|h| h == root);
    if home {
        return Err("That is your home folder, whose .claude and .agents folders are the agents' global ones. Use \
                    Install for Claude Code or for Codex and OpenCode instead."
            .into());
    }
    conn.execute(
        "INSERT OR IGNORE INTO skill_projects (path, added_at) VALUES (?1, ?2)",
        params![root.display().to_string(), now_ms()],
    )
    .map_err(|e| e.to_string())?;
    projects(conn, homes)
}

/// Forgets a project folder. Pygg's copies in it must be removed first, so none is left behind unmanaged; a folder that
/// no longer exists has none.
pub fn remove_project(conn: &Connection, homes: &Homes, path: &str) -> Result<Vec<Project>> {
    let skills = library(conn, homes)?;
    let copies = pygg_copies(&skills, path);
    if copies > 0 {
        return Err(format!(
            "Pygg still has {copies} skill{} installed in {path}; remove {} there first.",
            if copies == 1 { "" } else { "s" },
            if copies == 1 { "it" } else { "them" }
        ));
    }
    conn.execute("DELETE FROM skill_projects WHERE path = ?1", [path]).map_err(|e| e.to_string())?;
    projects(conn, homes)
}

/// Copies a skill found in an agent's folder into the library as the user's own, so it can be edited and installed for
/// the other agents. Only a `SKILL.md` inside one of the folders `context` reads.
pub fn import(conn: &Connection, homes: &Homes, path: &str) -> Result<Skill> {
    let path = PathBuf::from(path);
    let mut allowed = vec![homes.claude.join("skills"), homes.agents.join("skills"), homes.opencode.join("skills")];
    for root in project_paths(conn)? {
        allowed.extend([".claude", ".agents", ".opencode"].map(|d| root.join(d).join("skills")));
    }
    let dir = path.parent().ok_or("Choose a skill's SKILL.md.")?;
    if path.file_name().and_then(|n| n.to_str()) != Some("SKILL.md")
        || !allowed.iter().any(|a| dir.parent() == Some(a.as_path()))
    {
        return Err("Only a skill from an agent's skills folder can be added to the library.".into());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    let parsed = parse(&text);
    let id = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if starter(&id).is_some() || mine(conn, &id)?.is_some() {
        return Err(format!("The library already has a skill named {id}."));
    }
    save(
        conn,
        homes,
        &SkillEdit {
            id,
            description: parsed.description.unwrap_or_default(),
            body: parsed.body,
            manual: parsed.manual,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn homes(dir: &Path) -> Homes {
        Homes {
            claude: dir.join(".claude"),
            agents: dir.join(".agents"),
            codex: dir.join(".codex"),
            opencode: dir.join(".config/opencode"),
        }
    }

    fn edit(id: &str) -> SkillEdit {
        SkillEdit {
            id: id.into(),
            description: "Checks the \"build\" and says\nwhat broke".into(),
            body: "Run the build.\n".into(),
            manual: false,
        }
    }

    #[test]
    fn starters_are_valid_prompts() {
        for s in STARTERS {
            check(&SkillEdit { id: s.id.into(), description: s.description.into(), body: s.body.into(), manual: true })
                .unwrap();
            assert!(s.manual, "{} adds to every request", s.id);
            let parsed = parse(&render(s.id, s.description, s.body, s.manual));
            assert_eq!(parsed.name.as_deref(), Some(s.id));
            assert_eq!(parsed.description.as_deref(), Some(s.description));
            assert!(parsed.manual);
        }
    }

    #[test]
    fn names_follow_the_agent_skills_rules() {
        for ok in ["a", "review-diff", "v2-tests"] {
            assert!(valid_id(ok), "{ok}");
        }
        for bad in ["", "Review", "a--b", "-a", "a-", "a_b", "a b", "../x", &"x".repeat(65)] {
            assert!(!valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn rendered_skill_reads_back_and_its_hash_detects_edits() {
        let text = render("check-build", "Says \"what\" broke\nin the build", "Run it.\n", true);
        let parsed = parse(&text);
        assert_eq!(parsed.description.as_deref(), Some("Says \"what\" broke in the build"));
        assert!(parsed.manual);
        assert_eq!(parsed.body, "Run it.\n");
        assert_eq!(ours_unchanged(&text), Some(true));
        assert_eq!(ours_unchanged(&text.replace("\n", "\r\n")), Some(true), "line endings are not an edit");
        assert_eq!(ours_unchanged(&text.replace("Run it.", "Run it twice.")), Some(false));
        assert_eq!(ours_unchanged("---\nname: x\ndescription: y\n---\nbody"), None);
    }

    #[test]
    fn parses_block_and_quoted_values() {
        let p = parse("---\nname: 'it''s'\ndescription: >\n  one\n  two\nother: x\n---\nBody\n");
        assert_eq!(
            (p.name.as_deref(), p.description.as_deref(), p.body.as_str()),
            (Some("it's"), Some("one two"), "Body\n")
        );
        let p = parse("no front matter");
        assert_eq!((p.name, p.body.as_str()), (None, "no front matter"));
    }

    #[test]
    fn install_update_and_remove_only_what_pygg_wrote() {
        let dir = tempfile::tempdir().unwrap();
        let h = homes(dir.path());
        let conn = db::open_in_memory().unwrap();
        save(&conn, &h, &edit("check-build")).unwrap();

        let s = install(&conn, &h, "check-build", Target::ClaudeCode, None).unwrap();
        assert_eq!(s.installs[0].state, "installed");
        assert_eq!(s.installs[1].state, "absent");
        let file = h.claude.join("skills/check-build/SKILL.md");
        assert!(std::fs::read_to_string(&file).unwrap().contains("pygg-hash:"));

        // Edited in the library: the copy is outdated until installed again.
        save(&conn, &h, &SkillEdit { body: "Run the build twice.\n".into(), ..edit("check-build") }).unwrap();
        assert_eq!(get(&conn, &h, "check-build").unwrap().installs[0].state, "outdated");
        install(&conn, &h, "check-build", Target::ClaudeCode, None).unwrap();
        assert!(std::fs::read_to_string(&file).unwrap().contains("twice"));

        // Edited by hand: neither overwritten nor removed.
        std::fs::write(&file, std::fs::read_to_string(&file).unwrap().replace("twice", "three times")).unwrap();
        assert_eq!(get(&conn, &h, "check-build").unwrap().installs[0].state, "changed");
        assert!(install(&conn, &h, "check-build", Target::ClaudeCode, None).unwrap_err().contains("edited"));
        assert!(uninstall(&conn, &h, "check-build", Target::ClaudeCode, None).unwrap_err().contains("edited"));
        assert!(delete(&conn, &h, "check-build").unwrap_err().contains("remove it there first"));

        // Put back as Pygg wrote it: removed, and the folder with it.
        install(&conn, &h, "check-build", Target::Agents, None).unwrap();
        std::fs::remove_file(&file).unwrap();
        std::fs::remove_dir(file.parent().unwrap()).unwrap();
        let s = uninstall(&conn, &h, "check-build", Target::Agents, None).unwrap();
        assert!(s.installs.iter().all(|i| i.state == "absent"));
        assert!(!h.agents.join("skills/check-build").exists());
        delete(&conn, &h, "check-build").unwrap();
        assert!(get(&conn, &h, "check-build").is_err());
    }

    #[test]
    fn a_skill_pygg_did_not_write_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let h = homes(dir.path());
        let conn = db::open_in_memory().unwrap();
        let theirs = h.agents.join("skills/review-diff/SKILL.md");
        std::fs::create_dir_all(theirs.parent().unwrap()).unwrap();
        std::fs::write(&theirs, "---\nname: review-diff\ndescription: mine\n---\nMine.\n").unwrap();
        assert_eq!(get(&conn, &h, "review-diff").unwrap().installs[1].state, "other");
        assert!(install(&conn, &h, "review-diff", Target::Agents, None).unwrap_err().contains("did not write"));
        assert!(uninstall(&conn, &h, "review-diff", Target::Agents, None).unwrap_err().contains("not written by Pygg"));
        assert_eq!(
            std::fs::read_to_string(&theirs).unwrap(),
            "---\nname: review-diff\ndescription: mine\n---\nMine.\n"
        );
    }

    #[test]
    fn starters_cannot_be_overwritten_or_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let h = homes(dir.path());
        let conn = db::open_in_memory().unwrap();
        assert!(save(&conn, &h, &edit("review-diff")).unwrap_err().contains("starter"));
        assert!(delete(&conn, &h, "review-diff").unwrap_err().contains("starter"));
        assert!(save(&conn, &h, &SkillEdit { id: "Bad Name".into(), ..edit("x") }).is_err());
        assert!(save(&conn, &h, &SkillEdit { description: " ".into(), ..edit("x") }).is_err());
    }

    #[test]
    fn context_counts_what_each_agent_sends_with_every_request() {
        let dir = tempfile::tempdir().unwrap();
        let h = homes(dir.path());
        let conn = db::open_in_memory().unwrap();
        std::fs::create_dir_all(&h.claude).unwrap();
        std::fs::write(h.claude.join("CLAUDE.md"), "x".repeat(300)).unwrap();
        let listed = h.claude.join("skills/deploy/SKILL.md");
        std::fs::create_dir_all(listed.parent().unwrap()).unwrap();
        std::fs::write(&listed, "---\nname: deploy\ndescription: Deploys the app\n---\nLong instructions\n").unwrap();
        std::fs::create_dir_all(h.claude.join("commands")).unwrap();
        std::fs::write(h.claude.join("commands/notes.md"), "---\ndisable-model-invocation: true\n---\nTake notes\n")
            .unwrap();
        std::fs::create_dir_all(h.codex.join("prompts")).unwrap();
        std::fs::write(h.codex.join("prompts/fix.md"), "Fix it\n").unwrap();
        install(&conn, &h, "review-diff", Target::Agents, None).unwrap();

        let ctx = context(&conn, &h).unwrap();
        let claude = &ctx[0];
        assert_eq!(claude.items[0].kind, "instructions", "largest first");
        assert_eq!(claude.items[0].tokens, 100);
        let deploy = claude.items.iter().find(|i| i.name == "deploy").unwrap();
        assert_eq!(deploy.tokens, estimate_text_tokens("deploy: Deploys the app"));
        assert!(deploy.full_tokens > deploy.tokens);
        assert_eq!(claude.items.iter().find(|i| i.name == "notes").unwrap().tokens, 0, "run only by name");
        assert_eq!(claude.always_on_tokens, 100 + deploy.tokens);

        let codex = &ctx[1];
        let review = codex.items.iter().find(|i| i.name == "review-diff").unwrap();
        assert!(review.pygg && review.in_library && review.manual);
        assert_eq!(review.tokens, 0, "Pygg's agents/openai.yaml keeps it out of Codex's list");
        assert_eq!(codex.items.iter().find(|i| i.name == "fix").unwrap().tokens, 0);
        // A prompt someone else put there without Codex's file is listed by Codex.
        let theirs = h.agents.join("skills/notes/SKILL.md");
        std::fs::create_dir_all(theirs.parent().unwrap()).unwrap();
        std::fs::write(&theirs, "---\nname: notes\ndescription: Notes\ndisable-model-invocation: true\n---\nx\n")
            .unwrap();
        let ctx = context(&conn, &h).unwrap();
        let notes = ctx[1].items.iter().find(|i| i.name == "notes").unwrap();
        assert!(notes.tokens > 0 && !notes.manual);
        assert_eq!(ctx[2].items.iter().find(|i| i.name == "notes").unwrap().tokens, 0, "OpenCode leaves it out");

        let opencode = &ctx[2];
        assert_eq!(opencode.items.iter().filter(|i| i.kind == "skill").count(), 3, "both shared folders");
        assert_eq!(opencode.items[0].name, "CLAUDE.md", "OpenCode falls back to CLAUDE.md");
        assert_eq!(opencode.items.iter().find(|i| i.name == "review-diff").unwrap().tokens, 0);
    }

    #[test]
    fn a_prompt_for_codex_carries_its_policy_file_and_pygg_keeps_it_in_step() {
        let dir = tempfile::tempdir().unwrap();
        let h = homes(dir.path());
        let conn = db::open_in_memory().unwrap();
        let skill_dir = h.agents.join("skills/release-notes");
        let policy = skill_dir.join("agents/openai.yaml");
        save(&conn, &h, &SkillEdit { manual: true, ..edit("release-notes") }).unwrap();

        // Claude Code's folder gets no Codex file; the shared folder does.
        install(&conn, &h, "release-notes", Target::ClaudeCode, None).unwrap();
        assert!(!h.claude.join("skills/release-notes/agents").exists());
        let s = install(&conn, &h, "release-notes", Target::Agents, None).unwrap();
        assert_eq!(s.installs[1].state, "installed");
        assert_eq!(std::fs::read_to_string(&policy).unwrap(), CODEX_POLICY);
        assert!(implicit_invocation_off(CODEX_POLICY));

        // A copy installed before Pygg wrote Codex's file is brought up to date.
        std::fs::remove_file(&policy).unwrap();
        assert_eq!(get(&conn, &h, "release-notes").unwrap().installs[1].state, "outdated");
        install(&conn, &h, "release-notes", Target::Agents, None).unwrap();
        assert!(policy.exists());

        // No longer a prompt: the file goes.
        save(&conn, &h, &edit("release-notes")).unwrap();
        assert_eq!(get(&conn, &h, "release-notes").unwrap().installs[1].state, "outdated");
        install(&conn, &h, "release-notes", Target::Agents, None).unwrap();
        assert!(!skill_dir.join("agents").exists());

        // A prompt again, then Codex's file edited by hand: neither overwritten nor removed.
        save(&conn, &h, &SkillEdit { manual: true, ..edit("release-notes") }).unwrap();
        install(&conn, &h, "release-notes", Target::Agents, None).unwrap();
        std::fs::write(&policy, "policy:\n  allow_implicit_invocation: false\ninterface:\n  brand_color: \"#000\"\n")
            .unwrap();
        assert_eq!(get(&conn, &h, "release-notes").unwrap().installs[1].state, "changed");
        assert!(install(&conn, &h, "release-notes", Target::Agents, None).unwrap_err().contains("edited"));
        assert!(uninstall(&conn, &h, "release-notes", Target::Agents, None).unwrap_err().contains("openai.yaml"));
        assert!(skill_dir.join("SKILL.md").exists());

        // Pygg's own: removed with the skill, and the folders with them.
        install(&conn, &h, "release-notes", Target::Agents, None).unwrap_err();
        std::fs::write(&policy, CODEX_POLICY.replace('\n', "\r\n")).unwrap();
        assert_eq!(get(&conn, &h, "release-notes").unwrap().installs[1].state, "installed", "line endings");
        uninstall(&conn, &h, "release-notes", Target::Agents, None).unwrap();
        assert!(!skill_dir.exists());
    }

    #[test]
    fn reads_codex_implicit_invocation_policy() {
        assert!(implicit_invocation_off("policy:\n  allow_implicit_invocation: \"false\"  # by name\n"));
        assert!(implicit_invocation_off(
            "interface:\n  display_name: \"X\"\npolicy:\r\n  products: []\r\n  allow_implicit_invocation: False\r\n"
        ));
        assert!(!implicit_invocation_off("policy:\n  allow_implicit_invocation: true\n"));
        assert!(!implicit_invocation_off("interface:\n  allow_implicit_invocation: false\n"));
        assert!(!implicit_invocation_off("# policy:\n#  allow_implicit_invocation: false\n"));
    }

    #[test]
    fn a_project_added_gets_its_own_installs_and_its_own_count() {
        let dir = tempfile::tempdir().unwrap();
        let h = homes(dir.path());
        let conn = db::open_in_memory().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("CLAUDE.md"), "y".repeat(90)).unwrap();
        std::fs::write(repo.join("AGENTS.md"), "z".repeat(60)).unwrap();
        let given = repo.display().to_string();

        // Only a project the user added is a place to write.
        let refused = install(&conn, &h, "review-diff", Target::Agents, Some(&given)).unwrap_err();
        assert!(refused.contains("not one of the projects"), "{refused}");
        assert!(add_project(&conn, &h, &dir.path().display().to_string()).unwrap_err().contains("home folder"));
        assert!(add_project(&conn, &h, &dir.path().join("gone").display().to_string()).is_err());
        let list = add_project(&conn, &h, &given).unwrap();
        assert_eq!((list.len(), list[0].name.as_str(), list[0].missing), (1, "repo", false));
        assert_eq!(add_project(&conn, &h, &given).unwrap().len(), 1, "added once");
        let path = list[0].path.clone();

        let s = install(&conn, &h, "review-diff", Target::Agents, Some(&path)).unwrap();
        assert_eq!(s.installs.len(), 4, "two global places and two in the project");
        let here = |s: &Skill, t: Target| {
            s.installs.iter().find(|i| i.target == t && i.project.as_deref() == Some(path.as_str())).unwrap().state
        };
        assert_eq!(here(&s, Target::Agents), "installed");
        assert!(repo.join(".agents/skills/review-diff/agents/openai.yaml").exists(), "Codex reads the project's too");
        assert!(!h.agents.join("skills/review-diff").exists(), "the global folder is untouched");
        let s = install(&conn, &h, "review-diff", Target::ClaudeCode, Some(&path)).unwrap();
        assert_eq!(here(&s, Target::ClaudeCode), "installed");
        assert!(repo.join(".claude/skills/review-diff/SKILL.md").exists());

        // What each agent adds in the project: its instruction file, not the prompt.
        let p = &projects(&conn, &h).unwrap()[0];
        assert_eq!(p.pygg_skills, 1);
        assert_eq!(p.agents[0].always_on_tokens, 30, "Claude Code: CLAUDE.md");
        assert_eq!(p.agents[1].always_on_tokens, 20, "Codex: AGENTS.md");
        assert_eq!(p.agents[2].always_on_tokens, 20, "OpenCode: AGENTS.md before CLAUDE.md");
        assert_eq!(p.agents[2].items.iter().filter(|i| i.kind == "skill").count(), 1, "one name, two folders");

        // A skill found in the project can join the library.
        let theirs = Path::new(&path).join(".claude/skills/deploy/SKILL.md");
        std::fs::create_dir_all(theirs.parent().unwrap()).unwrap();
        std::fs::write(&theirs, "---\nname: deploy\ndescription: Deploys\n---\nSteps\n").unwrap();
        import(&conn, &h, &theirs.display().to_string()).unwrap();

        // Forgotten only once Pygg's copies there are gone.
        assert!(remove_project(&conn, &h, &path).unwrap_err().contains("remove it there first"));
        uninstall(&conn, &h, "review-diff", Target::Agents, Some(&path)).unwrap();
        uninstall(&conn, &h, "review-diff", Target::ClaudeCode, Some(&path)).unwrap();
        assert!(!repo.join(".agents/skills/review-diff").exists());
        assert!(remove_project(&conn, &h, &path).unwrap().is_empty());
        assert_eq!(get(&conn, &h, "review-diff").unwrap().installs.len(), 2);
        assert!(uninstall(&conn, &h, "review-diff", Target::Agents, Some(&path)).is_err());
    }

    #[test]
    fn import_copies_an_installed_skill_into_the_library() {
        let dir = tempfile::tempdir().unwrap();
        let h = homes(dir.path());
        let conn = db::open_in_memory().unwrap();
        let theirs = h.claude.join("skills/deploy/SKILL.md");
        std::fs::create_dir_all(theirs.parent().unwrap()).unwrap();
        std::fs::write(&theirs, "---\nname: deploy\ndescription: Deploys the app\n---\nSteps\n").unwrap();
        let s = import(&conn, &h, &theirs.display().to_string()).unwrap();
        assert_eq!((s.origin, s.description.as_str(), s.body.as_str()), ("mine", "Deploys the app", "Steps"));
        assert!(import(&conn, &h, &theirs.display().to_string()).unwrap_err().contains("already"));
        let outside = dir.path().join("SKILL.md");
        std::fs::write(&outside, "x").unwrap();
        assert!(import(&conn, &h, &outside.display().to_string()).is_err());
    }
}
