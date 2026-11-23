# Pygg for Claude Code

Pygg is a free, open-source (MIT) local AI gateway and task runner. This plugin lets Claude Code hand a well-defined
coding subtask (a bug fix, tests, a refactor) to cheaper coding agents through Pygg. Each subtask runs in its own Git
worktree, is checked by the commands you approved for the repository, and becomes a branch only when you keep it.

## What you need

- The Pygg app (Windows) or its command line (`npm install -g @pygg/cli@dev`), with the gateway running.
- A Pygg API key allowed to start tasks: in the Pygg app, **Endpoint → API Keys**, then **Limits… → May start tasks**.
  The plugin asks for it when you enable it and keeps it as a sensitive setting.

## What it adds

- **MCP server `pygg`**: tools to run a task, follow its status, read its diff and brief, and keep or discard its
  result (`pygg_run_task`, `pygg_task_status`, `pygg_task_diff`, `pygg_task_brief`, `pygg_keep_result`,
  `pygg_check_result`, `pygg_discard_result`, `pygg_stop_task`, `pygg_list_tasks`, `pygg_agents`); to plan a project into tasks
  (`pygg_brainstorm`, `pygg_board`, `pygg_board_move`); and to read what Pygg knows of a model (`pygg_model_facts`).
- **Skill `delegate-tasks`**: when and how to hand a subtask to Pygg, and how to review what comes back.
- **Skill `route-through-pygg`**: how to point Claude Code, Codex or OpenCode at Pygg's gateway.

## What it runs and sends

- `scripts/mcp.mjs` starts Pygg's own MCP server and passes its input and output through. It looks for, in order: the
  Pygg app's `pygg-mcp.exe` in your user folder, a `pygg` command on your PATH, then `npx -y @pygg/cli@<pinned
  version> mcp`, which downloads Pygg's command line from npm.
- The MCP server talks only to Pygg's gateway on your machine (`http://127.0.0.1:8741` unless you change the address),
  with the API key you gave. The plugin itself sends nothing anywhere else and stores nothing.
- What Pygg's gateway then does is set by you in Pygg: which agents and models a task may use, its checks, and its
  spending cap. Paid steps wait for your approval in the Pygg app.

## Links

- Source, documentation and releases: <https://github.com/nambean/Pygg>
- Security policy: <https://github.com/nambean/Pygg/blob/main/docs/SECURITY.md>
- Licence: MIT
