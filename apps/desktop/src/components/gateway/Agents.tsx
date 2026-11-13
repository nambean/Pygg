import { useEffect, useState, type ReactNode } from 'react';
import { Bot, ExternalLink, FolderGit2, GitBranch, Play, Plug, ShieldCheck, Trash2 } from 'lucide-react';
import { call, errorText, gatewayChanged, type AgentInfo, type AgentRun, type Integration } from '../../lib/api';
import { t } from '../../lib/i18n';
import { tokens } from '../../lib/money';
import { useGateway } from '../../hooks/useGateway';
import { Badge, CodeBlock, ConfirmSheet, CopyButton, ErrorNote, FormRow, Segmented, Sheet, SkeletonRows, Spinner } from '../ui';

/**
 * Agents: the official agent CLIs the user already has (Claude Code, Codex CLI, Gemini CLI, OpenCode), each with
 * its own sign-in, which Pygg never reads. A task runs in a separate Git worktree of the chosen repository; the
 * result is kept as a branch or discarded, and every run is in Usage.
 */
export function Agents() {
  const agents = useGateway(api => api.agents(), [], undefined, 'agents');
  const installed = (agents.data ?? []).filter(a => a.installed);
  return <div className="page">
    <section className="group">
      <h2>{t('Agents on This Computer')}</h2>
      <p className="group-intro"><ShieldCheck size={15} aria-hidden/>
        {t('Pygg starts each agent’s own program with the sign-in it already has: a subscription, a free tier or a key it manages. Pygg never reads, stores or shares that sign-in, and never offers it to other apps.')}</p>
      {agents.error && <ErrorNote>{agents.error}</ErrorNote>}
      {!agents.data && agents.loading && <SkeletonRows count={4} label={t('Looking for agents: asking each one for its version and sign-in…')}/>}
      <ul className="group-box agent-list">
        {(agents.data ?? []).map(a => <AgentRow key={a.id} agent={a}/>)}
      </ul>
    </section>
    <IdeSetup/>
    {agents.data && <RunTask agents={installed}/>}
  </div>;
}

function AgentRow({agent: a}: {agent: AgentInfo}) {
  return <li className="agent-row">
    <span className="provider-logo" aria-hidden><Bot size={15}/></span>
    <span className="agent-text">
      <span className="agent-title">{a.name}
        {a.installed ? <Badge tone="ok">{t('Installed')}</Badge> : <Badge tone="off">{t('Not found')}</Badge>}
        {a.version && <span className="secondary mono"> {a.version}</span>}</span>
      <small className="secondary">{a.login ? `${a.login} · ` : ''}{t(a.signIn)}</small>
    </span>
    {!a.installed && <a className="button small" href={a.install} target="_blank" rel="noreferrer">
      {t('How to Install')}<ExternalLink size={12} aria-hidden/></a>}
  </li>;
}

function RunTask({agents}: {agents: AgentInfo[]}) {
  const [agent, setAgent] = useState(agents[0]?.id ?? '');
  const [workspace, setWorkspace] = useState('');
  const [prompt, setPrompt] = useState('');
  const [model, setModel] = useState('');
  const [allowShell, setAllowShell] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [run, setRun] = useState<AgentRun | null>(null);
  useEffect(() => { if (!agent && agents[0]) setAgent(agents[0].id); }, [agents, agent]);
  const canRun = !!agent && workspace.trim() !== '' && prompt.trim() !== '' && !busy;

  const start = () => {
    setBusy(true);
    setError(null);
    setRun(null);
    call(api => api.runAgent({agent, workspace: workspace.trim(), prompt: prompt.trim(), model: model.trim() || null, allowShell,
      timeoutSecs: null}))
      .then(r => { setRun(r); gatewayChanged(); }, e => setError(errorText(e)))
      .finally(() => setBusy(false));
  };

  if (agents.length === 0) {
    return <section className="group"><h2>{t('Run a Task')}</h2>
      <p className="group-footnote">{t('Install one of the agents above to run tasks with it.')}</p></section>;
  }
  return <section className="group">
    <h2>{t('Run a Task')}</h2>
    <div className="group-box form agent-form">
      <FormRow label={t('Agent')} htmlFor="agent-pick">
        <select id="agent-pick" className="popup-button" value={agent} onChange={e => setAgent(e.target.value)} disabled={busy}>
          {agents.map(a => <option key={a.id} value={a.id}>{a.name}</option>)}
        </select>
      </FormRow>
      <FormRow label={t('Repository')} htmlFor="agent-workspace"
        hint={t('A folder under Git. The agent works on a separate copy (a worktree) of its current commit; your files and branch are not touched.')}>
        <span className="input-with-icon"><FolderGit2 size={13} aria-hidden/>
          <input id="agent-workspace" className="text-input mono" value={workspace} onChange={e => setWorkspace(e.target.value)}
            placeholder="C:\\path\\to\\repo" spellCheck={false} disabled={busy}/></span>
      </FormRow>
      <FormRow label={t('Task')} htmlFor="agent-prompt">
        <textarea id="agent-prompt" className="text-input agent-prompt" rows={4} value={prompt} onChange={e => setPrompt(e.target.value)}
          placeholder={t('What should the agent do? Be specific about the files and what done looks like.')} disabled={busy}/>
      </FormRow>
      <FormRow label={t('Model')} htmlFor="agent-model" hint={t('Optional, in the agent’s own naming. Empty uses the agent’s default.')}>
        <input id="agent-model" className="text-input mono" value={model} onChange={e => setModel(e.target.value)} disabled={busy}/>
      </FormRow>
      <FormRow label={t('Shell commands')} htmlFor="agent-shell"
        hint={allowShell ? t('The agent may run commands in the copy, such as tests. Codex always runs in its own sandbox.')
          : t('Off: the agent may read and edit files only. Codex always runs commands in its own sandbox.')}>
        <label className="check-row"><input id="agent-shell" type="checkbox" checked={allowShell} onChange={e => setAllowShell(e.target.checked)}
          disabled={busy}/>{t('Allow')}</label>
      </FormRow>
      <div className="form-actions">
        <span className="secondary">{t('Up to 15 minutes. The run uses the agent’s own plan or quota.')}</span>
        <button type="button" className="button prominent" onClick={start} disabled={!canRun}>
          {busy ? <Spinner/> : <Play size={13} aria-hidden/>}{busy ? t('Running…') : t('Run Task')}</button>
      </div>
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
    {run && <RunResult run={run} onDone={() => setRun(null)}/>}
  </section>;
}

function RunResult({run, onDone}: {run: AgentRun; onDone: () => void}) {
  const [diff, setDiff] = useState<string | null>(null);
  const [title, setTitle] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const u = run.usage;
  const ok = run.outcome === 'ok';
  const finish = (keep: string | null) => {
    setBusy(true);
    call(api => api.finishAgentRun(run.id, keep)).then(
      branch => { setMessage(branch ? t('Kept as branch {branch}. Your checkout was not changed.', {branch}) : t('Discarded.')); setBusy(false); },
      e => { setError(errorText(e)); setBusy(false); });
  };
  return <div className="group-box agent-result">
    <div className="agent-result-head">
      {ok ? <Badge tone="ok">{t('Done')}</Badge> : <Badge tone="error">{run.outcome === 'timed_out' ? t('Timed out') : t('Failed')}</Badge>}
      <span className="secondary">{t('{s} s', {s: (run.durationMs / 1000).toFixed(1)})} · {run.exit}</span>
      <span className="secondary">· {t('{files} files, +{ins} −{del}', {files: run.filesChanged, ins: run.insertions, del: run.deletions})}</span>
    </div>
    {u.result && <p className="agent-message">{u.result}</p>}
    {u.error && <ErrorNote>{u.error}</ErrorNote>}
    <dl className="detail-list">
      <dt>{t('Model')}</dt><dd className="mono">{u.model ?? run.model ?? t('agent’s default')}</dd>
      <dt>{t('Tokens')}</dt><dd>{u.inputTokens != null ? t('{input} in · {output} out · {cache} from cache', {input: tokens(u.inputTokens),
        output: tokens(u.outputTokens ?? 0), cache: tokens(u.cacheReadTokens ?? 0)}) : t('Not reported')}</dd>
      <dt>{t('Cost')}</dt><dd>{u.estimateUsd != null
        ? t('Unknown: depends on the agent’s sign-in. Its own list-price estimate: ${amount}', {amount: u.estimateUsd.toFixed(4)})
        : t('Unknown: depends on the agent’s sign-in (a subscription is not billed per request).')}</dd>
    </dl>
    {!ok && run.logTail && <details><summary>{t('Agent output')}</summary><pre className="log-tail">{run.logTail}</pre></details>}
    {message ? <p className="ok-note">{message} <button type="button" className="link-button" onClick={onDone}>{t('Close')}</button></p> : <>
      {run.filesChanged > 0 && <details onToggle={e => {
        if ((e.target as HTMLDetailsElement).open && diff === null) call(api => api.agentDiff(run.id)).then(setDiff, x => setError(errorText(x)));
      }}>
        <summary>{t('Changes')}</summary>
        {diff === null ? <Spinner/> : <div className="diff-box"><CopyButton text={diff}/><pre className="log-tail">{diff}</pre></div>}
      </details>}
      <div className="form-actions">
        {run.filesChanged > 0 && <>
          <input className="text-input" value={title} onChange={e => setTitle(e.target.value)} placeholder={t('Branch title, e.g. Fix add()')}
            aria-label={t('Branch title')} disabled={busy}/>
          <button type="button" className="button prominent" onClick={() => finish(title.trim() || 'agent task')} disabled={busy}>
            <GitBranch size={13} aria-hidden/>{t('Keep as Branch')}</button>
        </>}
        <button type="button" className="button" onClick={() => finish(null)} disabled={busy}><Trash2 size={13} aria-hidden/>{t('Discard')}</button>
      </div>
    </>}
    {error && <ErrorNote>{error}</ErrorNote>}
  </div>;
}

/** Setting up pygg-mcp, so an IDE agent can hand tasks to Pygg; they run here and show up in Tasks. */
function IdeSetup() {
  const setup = useGateway(api => api.mcpSetup());
  const [target, setTarget] = useState<'claude' | 'codex' | 'json'>('claude');
  // The app's own program serves MCP when started with `mcp`.
  const path = setup.data?.path ?? 'Pygg.exe';
  const args = setup.data?.args ?? ['mcp'];
  const url = setup.data && setup.data.url !== 'http://127.0.0.1:8741' ? setup.data.url : null;
  const code = target === 'claude'
    ? `claude mcp add pygg -e PYGG_TOKEN=<token>${url ? ` -e PYGG_URL=${url}` : ''} -- "${path}" ${args.join(' ')}`
    : target === 'codex'
    ? `# ~/.codex/config.toml\n[mcp_servers.pygg]\ncommand = '${path}'\nargs = ${JSON.stringify(args)}\nenv = { PYGG_TOKEN = "<token>"${url ? `, PYGG_URL = "${url}"` : ''} }\n# Tasks take minutes; Codex gives up on a tool call after 60 s by default.\ntool_timeout_sec = 600\n# Needed for codex exec, which cannot ask: run Pygg's tools without asking each time.\n# default_tools_approval_mode = "approve"`
    : JSON.stringify({mcpServers: {pygg: {command: path, args, env: {PYGG_TOKEN: '<token>', ...(url ? {PYGG_URL: url} : {})}}}}, null, 2);
  return <section className="group">
    <h2>{t('Let Your Agent Delegate Tasks')}</h2>
    <div className="group-box ide-setup">
      <p className="group-intro">{t('Your coding agent (Claude Code, Codex, Cursor…) can hand a subtask to Pygg with the pygg_run_task tool. It runs here, on the agents below, in a separate worktree and against your approved checks; your agent reviews the result and keeps or discards it. Tasks appear in Tasks, where paid steps wait for your approval.')}</p>
      <Integrations kind="mcp"/>
      <details className="by-hand">
      <summary>{t('Set It Up by Hand')}</summary>
      <ol className="setup-steps">
        <li>{t('In Clients, create a token for your IDE, then open Limits… and turn on “May start tasks”.')}</li>
        <li>{t('Register Pygg as an MCP server with your agent, with that token in place of <token>. The app’s own program serves it:')}</li>
      </ol>
      <Segmented label={t('Agent')} value={target} onChange={setTarget}
        options={[{value: 'claude', label: 'Claude Code'}, {value: 'codex', label: 'Codex CLI'}, {value: 'json', label: t('Cursor and others (JSON)')}]}/>
      <CodeBlock code={code} label={t('Setup')}/>
      <p className="secondary small">{t('The token ends up in your agent’s configuration file. It can only reach this computer’s gateway, and you can revoke it in Clients.')}</p>
      </details>
    </div>
  </section>;
}

/** Codex's models through Pygg's gateway: a provider in config.toml and the profile file pygg.config.toml (integrations.rs). */
const CODEX_GATEWAY = 'codex-gateway';

const INTEGRATION_STATE: Record<Integration['state'], {tone: 'ok' | 'warn' | 'off' | 'info'; label: string}> = {
  set_up: {tone: 'ok', label: 'Set up'},
  not_set_up: {tone: 'off', label: 'Not set up'},
  set_up_elsewhere: {tone: 'info', label: 'Set up by hand'},
  removed_elsewhere: {tone: 'warn', label: 'Removed outside Pygg'},
  not_installed: {tone: 'off', label: 'Not found'},
};

/**
 * One click per agent (ROADMAP D2): Pygg shows what it will change, backs up the agent's configuration, makes a
 * token that may start tasks and reaches no model, registers its MCP server and checks the agent lists it.
 */
export function Integrations({kind, status, note}: {kind: 'mcp' | 'gateway'; status?: (i: Integration) => ReactNode;
  /** Shown under a tool that can be set up, before Set Up: on CLI Tools, whether auto has a model for its requests. */
  note?: (i: Integration) => ReactNode}) {
  const all = useGateway(api => api.integrations(), [], undefined, 'integrations');
  const list = {...all, data: all.data?.filter(i => (i.kind ?? 'mcp') === kind)};
  const [planning, setPlanning] = useState<Integration | null>(null);
  const [removing, setRemoving] = useState<Integration | null>(null);
  if (list.error) return <ErrorNote>{list.error}</ErrorNote>;
  if (!list.data) return <SkeletonRows count={kind === 'gateway' ? 2 : 3} label={t('Checking what is set up in each tool…')}/>;
  return <>
    <ul className="group-box agent-list integrations">
      {list.data.map(i => {
        const state = INTEGRATION_STATE[i.state];
        const canSetUp = i.installed && (i.state === 'not_set_up' || i.state === 'removed_elsewhere');
        const canRemove = i.state === 'set_up' || i.state === 'set_up_elsewhere' || i.state === 'removed_elsewhere';
        return <li key={i.id} className="agent-row">
          <span className="provider-logo" aria-hidden><Plug size={15}/></span>
          <span className="agent-text">
            <span className="agent-title">{i.name}<Badge tone={state.tone}>{t(state.label)}</Badge></span>
            <small className="secondary">
              {i.state === 'set_up' && i.setUpAt
                ? t('Set up {date} with the token {client}. Backup: {backup}', {date: new Date(i.setUpAt).toLocaleString(),
                    client: i.clientName ?? '', backup: i.backup ?? t('none, the file did not exist')})
                : i.state === 'set_up_elsewhere' ? (i.id === CODEX_GATEWAY
                    ? t('Codex already has a model provider named pygg that Pygg did not make. Pygg leaves it alone.')
                    : i.kind === 'gateway'
                    ? t('OpenCode already has a provider named pygg that Pygg did not make. Pygg leaves it alone.')
                    : t('{name} already has a server named pygg that Pygg did not make. Pygg leaves it alone.', {name: i.name}))
                : i.id === CODEX_GATEWAY && i.state === 'not_set_up' ? t('Codex can use your connections through Pygg, with auto routing, limits and Pygg’s own record of what a task step cost. Your default model stays; you choose Pygg per run. {file}', {file: i.configPath})
                : i.kind === 'gateway' && i.state === 'not_set_up' ? t('OpenCode can use your connections through Pygg, with auto routing, limits and Pygg’s own record of what a task step cost. {file}', {file: i.configPath})
                : i.state === 'removed_elsewhere' ? t('Pygg set this up, and the entry is gone. Remove revokes its token.')
                : i.state === 'not_installed' ? t('Not found on this computer.')
                : i.configPath}
              {i.kind === 'gateway' && i.state === 'set_up' && <><br/>{t('Use it with:')} <span className="mono">{i.id === CODEX_GATEWAY ? 'pygg run codex' : 'pygg run opencode'}</span>
                {' · '}{t('each launch keeps its own routing session')}</>}
              {i.detail && <><br/><span className="mono">{i.detail}</span></>}
            </small>
            {status && i.state === 'set_up' && status(i)}
            {note && canSetUp && note(i)}
          </span>
          <span className="row-actions">
            {canSetUp && <button type="button" className="button small" onClick={() => setPlanning(i)}>{t('Set Up…')}</button>}
            {canRemove && <button type="button" className="button small" onClick={() => setRemoving(i)}>{t('Remove…')}</button>}
          </span>
        </li>;
      })}
    </ul>
    {planning && <SetUpSheet integration={planning} onClose={() => setPlanning(null)}/>}
    {removing && <ConfirmSheet title={t('Remove Pygg from {name}?', {name: removing.name})} confirmLabel={t('Remove')}
      message={removing.id === CODEX_GATEWAY
        ? (removing.state === 'set_up_elsewhere'
          ? t('This removes the pygg model provider you added to Codex by hand, and pygg.config.toml if it uses it. Its token, if you made one, stays valid until you revoke it in Clients.')
          : t('This removes the pygg model provider from Codex’s config.toml and the profile file pygg.config.toml, and revokes the token Pygg made for it. Both files are backed up first; everything else in config.toml stays as it is.'))
        : removing.kind === 'gateway'
        ? (removing.state === 'set_up_elsewhere'
          ? t('This removes the pygg provider you added to OpenCode by hand. Its token, if you made one, stays valid until you revoke it in Clients.')
          : t('This removes the pygg provider from OpenCode and revokes the token Pygg made for it. The configuration file is backed up first. OpenCode then uses only its own providers.'))
        : removing.state === 'set_up_elsewhere'
        ? t('This removes the pygg server you added to {name} by hand. Its token, if you made one, stays valid until you revoke it in Clients.', {name: removing.name})
        : t('This removes the pygg server from {name} and revokes the token Pygg made for it. The configuration file is backed up first. Tasks already started are not affected.', {name: removing.name})}
      onConfirm={async () => { await call(api => api.removeIntegration(removing.id)); gatewayChanged(); }}
      onClose={() => setRemoving(null)}/>}
  </>;
}

function SetUpSheet({integration: i, onClose}: {integration: Integration; onClose: () => void}) {
  const plan = useGateway(api => api.integrationPlan(i.id), [i.id]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const apply = async () => {
    setBusy(true);
    setError(null);
    try { await call(api => api.applyIntegration(i.id)); gatewayChanged(); onClose(); }
    catch (e) { setError(errorText(e)); setBusy(false); }
  };
  const p = plan.data;
  const codex = i.id === CODEX_GATEWAY;
  return <Sheet title={codex ? t('Send Codex’s Models through Pygg') : i.kind === 'gateway' ? t('Send OpenCode’s Models through Pygg')
    : t('Set Up Pygg in {name}', {name: i.name})} subtitle={t('Nothing changes until you choose Set Up.')}
    onClose={onClose} busy={busy} footer={<>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
      <button type="button" className="button prominent" onClick={apply} disabled={busy || !p}>
        {busy ? t('Setting Up…') : t('Set Up')}</button>
    </>}>
    {plan.error && <ErrorNote>{plan.error}</ErrorNote>}
    {!p && !plan.error && <Spinner/>}
    {p && <ol className="setup-steps">
      <li>{p.configExists
        ? t('Copy {file} into {dir} first.', {file: p.configPath, dir: p.backupDir})
        : t('{file} does not exist yet; it will be created.', {file: p.configPath})}</li>
      <li>{p.kind === 'gateway'
        ? t('Create a client token, “{client}”, that reaches your connections like any client; set its limits in Clients.', {client: p.clientName})
        : t('Create a client token, “{client}”, that may start tasks and cannot use any model connection.', {client: p.clientName})}</li>
      <li>{p.method === 'command'
        ? t('Run {name}’s own command, so {name} edits its configuration itself (the new token in place of <new-token>):', {name: p.name})
        : codex ? t('Add this provider at the end of {file}, keeping every other setting and comment in it, and create the profile file pygg.config.toml next to it (the new token in place of <new-token>):', {file: p.configPath})
        : t('Add this entry to {file}, keeping everything else in it (the new token in place of <new-token>):', {file: p.configPath})}
        <CodeBlock code={p.change} label={t('Change')}/></li>
      <li>{p.kind === 'gateway'
        ? t('Check that the pygg provider is there. If anything fails, the file is put back and the token revoked.')
        : t('Check that {name} lists the pygg server. If anything fails, the file is put back and the token revoked.', {name: p.name})}</li>
    </ol>}
    <p className="secondary small">{codex
      ? t('Your default model and provider are not changed. Run pygg run codex to use Pygg with auto (or codex --profile pygg; add -m <connection>/<model> for a model from Connections). Through pygg run, each launch has a routing session of its own, so the gateway keeps it on the model that answered it, and its prompt cache; when Pygg runs Codex for a task, the session is the task step. Codex’s own sign-in is not touched.')
      : i.kind === 'gateway'
      ? t('OpenCode then lists auto, auto/cheap and the models enabled in Providers under the provider “Pygg”; pick one with /models. Pygg keeps the list current as models are enabled, disabled or looked up; only the list changes. Start it with pygg run opencode and each launch has a routing session of its own, so the gateway keeps it on the model that answered it, and its prompt cache; when Pygg runs OpenCode for a task, the session is the task step. OpenCode’s own providers and sign-in are not touched.')
      : t('The entry serves all your projects: the server takes the project from the folder your agent runs in. The token is written into {name}’s own configuration for your user, never into a project file. {name}’s own sign-in is not touched.', {name: i.name})}</p>
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}