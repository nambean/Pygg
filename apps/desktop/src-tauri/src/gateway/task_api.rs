//! The task API: smart tasks started from where people already work (an IDE agent through `pygg-mcp`, a
//! script), run by the app's task engine (tasks.rs) and watched in the app.
//!
//! - A client needs a token and the user's permission to start tasks (Clients → Limits… → Smart tasks).
//! - A client sees and acts on only the tasks it started.
//! - A paid step's approval belongs to the user, in the app: there is no API for it.
//! - The folder an agent names is resolved to its Git repository, which becomes a workspace if it is not one.
//!   Checks found there are suggestions until the user approves them in the app; until then a result waits for
//!   review instead of being confirmed.
//!
//!   GET  /pygg/v1/agents                      agents installed on this computer
//!   GET  /pygg/v1/tasks                       this client's tasks
//!   POST /pygg/v1/tasks                       start one: {workspace, prompt, title?, strategy?, steps?, allowShell?}
//!   GET  /pygg/v1/tasks/{id}?wait=SECONDS     its state; waits up to 600 s while it runs
//!   GET  /pygg/v1/tasks/{id}/diff             the result's changes
//!   POST /pygg/v1/tasks/{id}/stop | keep {title?} | discard

use super::{authenticate, openai_error, Shared};
use crate::gateway::store::Client;
use crate::tasks::{self, NewTask, Step};
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/pygg/v1/agents", get(agents))
        .route("/pygg/v1/tasks", get(list).post(start))
        .route("/pygg/v1/tasks/{id}", get(status))
        .route("/pygg/v1/tasks/{id}/diff", get(diff))
        .route("/pygg/v1/tasks/{id}/stop", post(stop))
        .route("/pygg/v1/tasks/{id}/keep", post(keep))
        .route("/pygg/v1/tasks/{id}/discard", post(discard))
        // pygg/brain (ADR-041): a client reads its own brainstorms; nothing here approves one.
        .route("/pygg/v1/brainstorms", get(brainstorms))
        .route("/pygg/v1/brainstorms/{id}", get(brainstorm))
        .route("/pygg/v1/facts", get(facts))
}

/// One value of the request's query string, un-escaped only as far as `+` and `%2F` go (ids and roles are plain).
fn query_value(uri: &Uri, name: &str) -> Option<String> {
    uri.query()?
        .split('&')
        .find_map(|pair| {
            pair.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| v.replace('+', " ").replace("%2F", "/"))
        })
        .filter(|v| !v.is_empty())
}

/// A client's brainstorms, newest first, or the one of `?session=`.
async fn brainstorms(State(shared): State<Shared>, headers: HeaderMap, uri: Uri) -> Response {
    let client = match authenticate(&shared, &headers) {
        Ok(client) => client,
        Err(refusal) => return refusal.openai(),
    };
    let session = query_value(&uri, "session");
    let result = blocking(&shared, move |db| match session {
        Some(s) => Ok(super::brain::by_session(db, &client.id, &s)?.into_iter().collect::<Vec<_>>()),
        None => super::brain::list(db, Some(&client.id)),
    })
    .await;
    match result {
        Ok(list) => Json(json!({ "brainstorms": list })).into_response(),
        Err(r) => r,
    }
}

async fn brainstorm(State(shared): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let client = match authenticate(&shared, &headers) {
        Ok(client) => client,
        Err(refusal) => return refusal.openai(),
    };
    match blocking(&shared, move |db| super::brain::get(db, &id)).await {
        Ok(Some(b)) if b.client_id.as_deref() == Some(client.id.as_str()) => Json(b).into_response(),
        Ok(_) => error(StatusCode::NOT_FOUND, "No such brainstorm of this client"),
        Err(r) => r,
    }
}

/// The connected models ranked for a role from the model facts feed (ADR-040), each with its source and date.
async fn facts(State(shared): State<Shared>, headers: HeaderMap, uri: Uri) -> Response {
    if let Err(refusal) = authenticate(&shared, &headers) {
        return refusal.openai();
    }
    let role = query_value(&uri, "role").unwrap_or_else(|| "code".into());
    let result = blocking(&shared, move |db| {
        let feed = super::facts::for_conn(db);
        let lines = super::facts::rank_role(db, feed.as_deref(), &role)?;
        Ok(json!({ "role": role, "built": feed.map(|f| f.built.clone()), "models": lines }))
    })
    .await;
    match result {
        Ok(v) => Json(v).into_response(),
        Err(r) => r,
    }
}

const MAX_WAIT_SECS: u64 = 600;

fn error(status: StatusCode, message: impl AsRef<str>) -> Response {
    openai_error(
        status,
        if status == StatusCode::NOT_FOUND { "not_found_error" } else { "invalid_request_error" },
        message.as_ref(),
    )
}

fn failure(message: String) -> Response {
    let status = if message == "That task no longer exists" { StatusCode::NOT_FOUND } else { StatusCode::BAD_REQUEST };
    error(status, message)
}

/// The client behind the token, if it may start tasks and this gateway runs them.
fn permitted(shared: &Shared, headers: &HeaderMap) -> Result<(Client, tasks::Runner), Response> {
    let client = authenticate(shared, headers).map_err(|r| r.openai())?;
    if !client.may_run_tasks {
        return Err(openai_error(
            StatusCode::FORBIDDEN,
            "permission_error",
            &format!(
                "Client \"{}\" may not start tasks. Allow it in Pygg → Clients → Limits… → Smart tasks.",
                client.name
            ),
        ));
    }
    let runner =
        shared.tasks.clone().ok_or_else(|| {
            openai_error(StatusCode::SERVICE_UNAVAILABLE, "unavailable",
        "Smart tasks run in the Pygg app; this gateway (`gateway serve`) does not run them. Open the app instead.")
        })?;
    Ok((client, runner))
}

/// Runs store and Git work off the async threads: keeping a result or resolving a repository runs git.
async fn blocking<T: Send + 'static>(
    shared: &Shared, f: impl FnOnce(&rusqlite::Connection) -> tasks::Result<T> + Send + 'static,
) -> Result<T, Response> {
    let shared = shared.clone();
    tokio::task::spawn_blocking(move || shared.with_db(f))
        .await
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "The task engine failed"))?
        .map_err(failure)
}

async fn agents(State(shared): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = permitted(&shared, &headers) {
        return r;
    }
    let installed = tasks::installed_agents();
    Json(json!({ "agents": crate::agents::AGENTS.iter().map(|a| json!({
        "id": a.id, "name": a.name, "installed": installed.contains(&a.id), "signIn": a.sign_in,
    })).collect::<Vec<_>>() }))
    .into_response()
}

async fn list(State(shared): State<Shared>, headers: HeaderMap) -> Response {
    let (client, _) = match permitted(&shared, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    match blocking(&shared, move |db| tasks::list_tasks_for(db, &client.id)).await {
        Ok(list) => Json(json!({ "tasks": list })).into_response(),
        Err(r) => r,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StepInput {
    agent: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    tier: Option<u8>,
    #[serde(default)]
    billing: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartTask {
    /// A folder inside the repository the task is about.
    workspace: String,
    prompt: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    strategy: Option<String>,
    /// Empty: the installed agents in the strategy's order.
    #[serde(default)]
    steps: Option<Vec<StepInput>>,
    #[serde(default)]
    allow_shell: bool,
    #[serde(default)]
    timeout_secs: Option<u64>,
    /// none, tail (the default) or evidence (H1, ADR-030).
    #[serde(default)]
    handoff: Option<String>,
}

async fn start(State(shared): State<Shared>, headers: HeaderMap, body: Bytes) -> Response {
    let (client, runner) = match permitted(&shared, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let request: StartTask = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error(StatusCode::BAD_REQUEST, format!("The request body is not a task: {e}")),
    };
    if request.prompt.chars().count() > 100_000 {
        return error(StatusCode::BAD_REQUEST, "The task is longer than 100,000 characters");
    }
    let strategy = request.strategy.clone().unwrap_or_else(|| "free_first".into());
    let steps: Vec<Step> = match request.steps {
        Some(steps) if !steps.is_empty() => steps
            .into_iter()
            .map(|s| Step {
                tier: s.tier.unwrap_or_else(|| tasks::tier_guess(&s.agent)),
                agent: s.agent,
                model: s.model,
                // A paid step waits for the user's approval in the app.
                billing: s.billing.unwrap_or_else(|| "unknown".into()),
            })
            .collect(),
        _ => tasks::default_steps(&strategy, &tasks::installed_agents()),
    };
    if steps.is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "No agent is installed on this computer. Install Claude Code, Codex CLI, Gemini CLI or OpenCode.",
        );
    }
    let client_id = client.id.clone();
    let created = blocking(&shared, move |db| {
        let workspace = tasks::workspace_for_folder(db, &request.workspace)?;
        let new = NewTask {
            workspace_id: workspace.id,
            title: request.title,
            prompt: request.prompt,
            strategy,
            steps,
            allow_shell: request.allow_shell,
            timeout_secs: request.timeout_secs,
            handoff: request.handoff,
        };
        let id = tasks::create_task_for(db, &new, Some(&client_id))?;
        tasks::task_detail_for(db, &id, Some(&client_id))
    })
    .await;
    match created {
        Ok(detail) => {
            runner.start(&detail.summary.id);
            (StatusCode::CREATED, Json(detail)).into_response()
        }
        Err(r) => r,
    }
}

fn wait_of(uri: &Uri) -> u64 {
    uri.query()
        .unwrap_or("")
        .split('&')
        .find_map(|pair| pair.strip_prefix("wait="))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
        .min(MAX_WAIT_SECS)
}

async fn status(State(shared): State<Shared>, headers: HeaderMap, Path(id): Path<String>, uri: Uri) -> Response {
    let (client, _) = match permitted(&shared, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(wait_of(&uri));
    loop {
        let (id2, client_id) = (id.clone(), client.id.clone());
        let detail = match blocking(&shared, move |db| tasks::task_detail_for(db, &id2, Some(&client_id))).await {
            Ok(d) => d,
            Err(r) => return r,
        };
        if detail.summary.state != "running" || tokio::time::Instant::now() >= deadline {
            return Json(detail).into_response();
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn diff(State(shared): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (client, _) = match permitted(&shared, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    match blocking(&shared, move |db| {
        tasks::check_owner(db, &id, &client.id)?;
        tasks::result_diff(db, &id)
    })
    .await
    {
        Ok(diff) => Json(json!({ "diff": diff })).into_response(),
        Err(r) => r,
    }
}

async fn stop(State(shared): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (client, runner) = match permitted(&shared, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let id2 = id.clone();
    if let Err(r) = blocking(&shared, move |db| tasks::check_owner(db, &id2, &client.id)).await {
        return r;
    }
    match runner.cancel(&id) {
        Ok(()) => Json(json!({ "stopping": true })).into_response(),
        Err(e) => failure(e),
    }
}

#[derive(Deserialize, Default)]
struct KeepBody {
    #[serde(default)]
    title: Option<String>,
}

async fn keep(State(shared): State<Shared>, headers: HeaderMap, Path(id): Path<String>, body: Bytes) -> Response {
    let (client, _) = match permitted(&shared, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let body: KeepBody = if body.is_empty() {
        KeepBody::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(b) => b,
            Err(e) => return error(StatusCode::BAD_REQUEST, e.to_string()),
        }
    };
    match blocking(&shared, move |db| {
        tasks::check_owner(db, &id, &client.id)?;
        tasks::keep(db, &id, body.title.as_deref())
    })
    .await
    {
        Ok(branch) => Json(json!({ "branch": branch })).into_response(),
        Err(r) => r,
    }
}

async fn discard(State(shared): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (client, _) = match permitted(&shared, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    match blocking(&shared, move |db| {
        tasks::check_owner(db, &id, &client.id)?;
        tasks::discard(db, &id)
    })
    .await
    {
        Ok(()) => Json(json!({ "discarded": true })).into_response(),
        Err(r) => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{AgentUsage, TaskRun};
    use crate::gateway::store;
    use crate::isolation;
    use crate::tasks::{Cancel, ExecRequest, Executor, Runner};
    use futures_util::future::BoxFuture;
    use serde_json::Value;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    /// An agent that writes `fixed.txt` in its worktree.
    fn writer() -> Executor {
        Arc::new(
            |request: ExecRequest, worktrees: PathBuf, _cancel: Cancel| -> BoxFuture<'static, tasks::Result<TaskRun>> {
                Box::pin(async move {
                    let id = uuid::Uuid::new_v4().simple().to_string();
                    std::fs::create_dir_all(&worktrees).unwrap();
                    let wt = isolation::create(&request.workspace, &request.base, &worktrees.join(&id[..12]))
                        .map_err(|e| e.to_string())?;
                    std::fs::write(wt.path.join("fixed.txt"), "fixed\n").unwrap();
                    let stat = isolation::changes(&wt).unwrap();
                    Ok(TaskRun {
                        id,
                        agent: request.agent,
                        model: request.model,
                        workspace: request.workspace.to_string_lossy().into_owned(),
                        worktree: wt.path.to_string_lossy().into_owned(),
                        base: wt.base,
                        outcome: "ok".into(),
                        exit: "exit 0".into(),
                        duration_ms: 1,
                        files_changed: stat.files,
                        insertions: stat.insertions,
                        deletions: stat.deletions,
                        usage: AgentUsage::default(),
                        log_tail: String::new(),
                    })
                })
            },
        )
    }

    struct Api {
        _dir: tempfile::TempDir,
        base: String,
        shared: Shared,
        repo: PathBuf,
        token: String,
        client: String,
        http: reqwest::Client,
        _server: crate::gateway::Server,
    }

    async fn api(with_runner: bool) -> Api {
        let dir = tempfile::tempdir().unwrap();
        let repo = isolation::tests::repo_with_commit(dir.path());
        std::fs::create_dir_all(repo.join("src")).unwrap();
        let conn = crate::db::open_in_memory().unwrap();
        let (client, token) = store::create_client(&conn, "Claude Code", None).unwrap();
        let db = Arc::new(Mutex::new(conn));
        let shared = Shared::new(db.clone(), Arc::new(crate::gateway::OsSecrets));
        let shared = if with_runner {
            shared.with_tasks(Runner::new(db, dir.path().join("worktrees"), writer()))
        } else {
            shared
        };
        let server = crate::gateway::start(shared.clone(), 0).await.unwrap();
        Api {
            base: format!("http://{}", server.addr),
            _dir: dir,
            shared,
            repo,
            token,
            client: client.id,
            http: reqwest::Client::new(),
            _server: server,
        }
    }

    impl Api {
        fn allow(&self) {
            self.shared.with_db(|db| store::set_client_may_run_tasks(db, &self.client, true)).unwrap();
        }
        async fn send(&self, method: reqwest::Method, path: &str, body: Option<Value>, token: &str) -> (u16, Value) {
            let mut request = self.http.request(method, format!("{}{path}", self.base)).bearer_auth(token);
            if let Some(body) = body {
                request = request.json(&body);
            }
            let response = request.send().await.unwrap();
            (response.status().as_u16(), response.json().await.unwrap_or(Value::Null))
        }
        fn start_body(&self, steps: Value) -> Value {
            json!({ "workspace": self.repo.join("src").to_string_lossy(), "prompt": "Add fixed.txt", "steps": steps })
        }
    }

    #[tokio::test]
    async fn a_client_starts_a_task_in_its_folder_and_sees_only_its_own() {
        let api = api(true).await;
        let (status, body) = api
            .send(
                reqwest::Method::POST,
                "/pygg/v1/tasks",
                Some(api.start_body(json!([{"agent": "codex"}]))),
                &api.token,
            )
            .await;
        assert_eq!(status, 403, "a client may not start tasks until the user allows it: {body}");
        assert!(body["error"]["message"].as_str().unwrap().contains("Limits"));

        api.allow();
        let (status, body) = api
            .send(
                reqwest::Method::POST,
                "/pygg/v1/tasks",
                Some(api.start_body(json!([{"agent": "codex"}]))),
                &api.token,
            )
            .await;
        assert_eq!(status, 201, "{body}");
        let id = body["id"].as_str().unwrap().to_string();
        assert_eq!(body["source"], "Claude Code");
        assert_eq!(body["policy"]["steps"][0]["tier"], 3, "a tier is guessed when none is given");

        let (status, task) =
            api.send(reqwest::Method::GET, &format!("/pygg/v1/tasks/{id}?wait=30"), None, &api.token).await;
        assert_eq!(status, 200);
        assert_eq!(task["state"], "needs_review", "no approved check, so the result waits for review: {task}");
        let workspace = api.shared.with_db(tasks::list_workspaces).unwrap();
        assert_eq!(workspace.len(), 1);
        assert!(std::path::Path::new(&workspace[0].path).ends_with("repo"), "the folder resolves to its repository");

        let (other, other_token) = api.shared.with_db(|db| store::create_client(db, "Other", None)).unwrap();
        api.shared.with_db(|db| store::set_client_may_run_tasks(db, &other.id, true)).unwrap();
        let (status, _) = api.send(reqwest::Method::GET, &format!("/pygg/v1/tasks/{id}"), None, &other_token).await;
        assert_eq!(status, 404, "another client does not see it");
        let (status, _) =
            api.send(reqwest::Method::POST, &format!("/pygg/v1/tasks/{id}/keep"), None, &other_token).await;
        assert_eq!(status, 404);
        let (_, list) = api.send(reqwest::Method::GET, "/pygg/v1/tasks", None, &other_token).await;
        assert_eq!(list["tasks"].as_array().unwrap().len(), 0);

        let (_, diff) = api.send(reqwest::Method::GET, &format!("/pygg/v1/tasks/{id}/diff"), None, &api.token).await;
        assert!(diff["diff"].as_str().unwrap().contains("+fixed"));
        let (status, kept) = api
            .send(
                reqwest::Method::POST,
                &format!("/pygg/v1/tasks/{id}/keep"),
                Some(json!({"title": "Add fixed"})),
                &api.token,
            )
            .await;
        assert_eq!(status, 200, "{kept}");
        assert!(kept["branch"].as_str().unwrap().starts_with("str/add-fixed-"));
        let (_, task) = api.send(reqwest::Method::GET, &format!("/pygg/v1/tasks/{id}"), None, &api.token).await;
        assert_eq!((task["state"].as_str(), task["result"].as_str()), (Some("manually_accepted"), Some("kept")));
    }

    #[tokio::test]
    async fn a_paid_step_waits_for_the_user_and_the_client_can_only_stop_it() {
        let api = api(true).await;
        api.allow();
        let (_, body) = api
            .send(
                reqwest::Method::POST,
                "/pygg/v1/tasks",
                Some(api.start_body(json!([{"agent": "claude-code", "billing": "paid"}]))),
                &api.token,
            )
            .await;
        let id = body["id"].as_str().unwrap().to_string();
        let (_, task) = api.send(reqwest::Method::GET, &format!("/pygg/v1/tasks/{id}?wait=10"), None, &api.token).await;
        assert_eq!(task["state"], "waiting_approval", "{task}");
        let (status, _) =
            api.send(reqwest::Method::POST, &format!("/pygg/v1/tasks/{id}/approve"), None, &api.token).await;
        assert!(status == 404 || status == 405, "there is no approval through the API");
        let (status, _) = api.send(reqwest::Method::POST, &format!("/pygg/v1/tasks/{id}/stop"), None, &api.token).await;
        assert_eq!(status, 200);
        let (_, task) = api.send(reqwest::Method::GET, &format!("/pygg/v1/tasks/{id}"), None, &api.token).await;
        assert_eq!(task["state"], "canceled");
    }

    #[tokio::test]
    async fn a_gateway_without_the_app_does_not_run_tasks() {
        let api = api(false).await;
        api.allow();
        let (status, body) = api
            .send(
                reqwest::Method::POST,
                "/pygg/v1/tasks",
                Some(api.start_body(json!([{"agent": "codex"}]))),
                &api.token,
            )
            .await;
        assert_eq!(status, 503, "{body}");
        let (status, _) =
            api.send(reqwest::Method::POST, "/pygg/v1/tasks", Some(api.start_body(json!([]))), "str-not-a-token").await;
        assert_eq!(status, 401);
    }

    #[test]
    fn default_steps_follow_the_strategy_and_what_is_installed() {
        let agents: Vec<String> = tasks::default_steps("free_first", &["codex", "opencode", "claude-code"])
            .into_iter()
            .map(|s| s.agent)
            .collect();
        assert_eq!(agents, ["opencode", "codex", "claude-code"]);
        let quality = tasks::default_steps("quality", &["opencode", "claude-code"]);
        assert_eq!(
            (quality[0].agent.as_str(), quality[0].tier, quality[0].billing.as_str()),
            ("claude-code", 3, "unknown")
        );
        assert!(tasks::default_steps("balanced", &[]).is_empty());
    }
}
