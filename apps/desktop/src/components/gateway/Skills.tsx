import { useState } from 'react';
import { BookMarked, FileText, FolderGit2, FolderOpen, MessageSquareText, Sparkles } from 'lucide-react';
import { call, errorText, type AgentContext, type ContextItem, type Skill, type SkillEdit, type SkillInstall, type SkillProject, type SkillTarget } from '../../lib/api';
import { t } from '../../lib/i18n';
import { runningInDesktop } from '../../lib/native';
import { useGateway } from '../../hooks/useGateway';
import { Badge, ConfirmSheet, Empty, ErrorNote, FormRow, MenuButton, Sheet, SkeletonRows, Spinner, type MenuItem, type Tone } from '../ui';
import './skills.css';

/**
 * Skills & Prompts (ADR-039): reusable instructions in the Agent Skills format that Claude Code, Codex and OpenCode
 * share, installed into each agent's own skills folder or into a project's, and what each agent sends with every request
 * from its global files and from each project's root. Pygg writes one SKILL.md per install and never touches a skill it
 * did not write or one edited since.
 */
export function Skills({intent, onIntentDone}: {intent?: 'create'; onIntentDone: () => void}) {
  const library = useGateway(api => api.skills());
  const context = useGateway(api => api.skillsContext());
  const projects = useGateway(api => api.skillProjects());
  const [editing, setEditing] = useState<{skill: SkillEdit; existing: boolean; readOnly: boolean} | null>(null);
  const [deleting, setDeleting] = useState<Skill | null>(null);
  const [addingProject, setAddingProject] = useState(false);
  const [managing, setManaging] = useState<string | null>(null);
  const [forgetting, setForgetting] = useState<SkillProject | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const creating = intent === 'create' && !editing;
  const reload = () => { library.reload(); context.reload(); projects.reload(); };
  const act = (key: string, run: () => Promise<unknown>) => {
    setBusy(key);
    setError(null);
    run().then(reload, e => setError(errorText(e))).finally(() => setBusy(null));
  };
  const blank: SkillEdit = {id: '', description: '', body: '', manual: true};
  const sheet = editing ?? (creating ? {skill: blank, existing: false, readOnly: false} : null);
  const closeSheet = () => { setEditing(null); if (creating) onIntentDone(); };
  const projectName = (path: string) => projects.data?.find(p => p.path === path)?.name ?? path;
  const importItem = (item: ContextItem) => act(`import:${item.path}`, () => call(api => api.importSkill(item.path)));
  const managed = managing ? projects.data?.find(p => p.path === managing) : undefined;

  return <div className="page">
    <p className="page-intro">{t('Reusable instructions for your coding agents, in the Agent Skills format that Claude Code, Codex and OpenCode share. An agent keeps only each skill’s name and description in context and reads the rest when it uses the skill. A prompt runs only when you type /name, so it costs nothing until then.')}</p>

    <section className="group">
      <h2>{t('Sent With Every Request')}</h2>
      <p className="group-intro">{t('What each agent adds to every request from its global files, before your own words: instruction files are sent whole, listed skills and commands by name and description. A project’s own files add to this; see Projects below. Providers cache it, so repeated requests read it at a fraction of the price, but each new session writes it again.')}</p>
      {context.error ? <ErrorNote>{context.error}</ErrorNote>
        : !context.data ? <SkeletonRows count={3} label={t('Reading the agents’ folders…')}/>
        : <ul className="group-box agent-list skills-context">
          {context.data.map(a => <AgentRow key={a.agent} agent={a} busy={busy} onImport={importItem}/>)}
        </ul>}
    </section>

    <section className="group">
      <h2>{t('Library')}</h2>
      <p className="group-intro">{t('Pygg’s starter skills and your own. Install one for Claude Code (its skills folder) or for Codex and OpenCode (the shared ~/.agents/skills folder); remove takes Pygg’s copy away again.')}</p>
      {error && <ErrorNote>{error}</ErrorNote>}
      {library.error ? <ErrorNote>{library.error}</ErrorNote>
        : !library.data ? <SkeletonRows count={6} label={t('Loading skills…')}/>
        : library.data.length === 0 ? <Empty icon={<BookMarked/>} title={t('No skills yet')}/>
        : <ul className="group-box agent-list skills-library">
          {library.data.map(s => <SkillRow key={s.id} skill={s} busy={busy} projectName={projectName}
            onInstall={target => act(`${s.id}:${target}`, () => call(api => api.installSkill(s.id, target)))}
            onUninstall={target => act(`${s.id}:${target}`, () => call(api => api.uninstallSkill(s.id, target)))}
            onOpen={() => setEditing({skill: {id: s.id, description: s.description, body: s.body, manual: s.manual}, existing: true, readOnly: s.origin === 'pygg'})}
            onDuplicate={() => setEditing({skill: {id: `${s.id}-mine`, description: s.description, body: s.body, manual: s.manual}, existing: false, readOnly: false})}
            onDelete={() => setDeleting(s)}/>)}
        </ul>}
    </section>

    <section className="group">
      <h2>{t('Projects')}</h2>
      <p className="group-intro">{t('Skills for one repository go into its own .claude/skills (Claude Code) and .agents/skills (Codex and OpenCode), so they travel with the code. Pygg writes only into folders you add here, and counts what each agent adds to every request from the files at the project’s root.')}</p>
      {projects.error ? <ErrorNote>{projects.error}</ErrorNote>
        : !projects.data ? <SkeletonRows count={2} label={t('Reading the projects…')}/>
        : projects.data.length === 0 ? <Empty icon={<FolderGit2/>} title={t('No projects yet')}
          action={<button type="button" className="button" onClick={() => setAddingProject(true)}>{t('Add Project…')}</button>}>
          {t('Add a repository to install skills into it and see what its CLAUDE.md and AGENTS.md add to every request.')}</Empty>
        : <>
          <ul className="group-box agent-list skills-projects">
            {projects.data.map(p => <ProjectRow key={p.path} project={p} busy={busy} onImport={importItem}
              onManage={() => setManaging(p.path)} onForget={() => setForgetting(p)}/>)}
          </ul>
          <div className="group-footer"><button type="button" className="button" onClick={() => setAddingProject(true)}>{t('Add Project…')}</button></div>
        </>}
    </section>

    {sheet && <SkillSheet initial={sheet.skill} existing={sheet.existing} readOnly={sheet.readOnly} onClose={closeSheet}
      onDuplicate={() => setEditing({skill: {...sheet.skill, id: `${sheet.skill.id}-mine`}, existing: false, readOnly: false})}
      onSaved={() => { closeSheet(); reload(); }}/>}
    {deleting && <ConfirmSheet title={t('Delete {name}?', {name: deleting.id})} confirmLabel={t('Delete')}
      message={t('This deletes the skill from your library. It is not installed anywhere, so no agent loses it.')}
      onConfirm={async () => { await call(api => api.deleteSkill(deleting.id)); reload(); }} onClose={() => setDeleting(null)}/>}
    {addingProject && <AddProjectSheet onClose={() => setAddingProject(false)} onAdded={() => { setAddingProject(false); reload(); }}/>}
    {managed && library.data && <ProjectSkillsSheet project={managed} skills={library.data} onChanged={reload}
      onClose={() => setManaging(null)}/>}
    {forgetting && <ConfirmSheet title={t('Remove {name} from Pygg?', {name: forgetting.name})} confirmLabel={t('Remove')}
      message={t('Pygg stops listing this folder and will not write into it. Nothing in the folder is changed.')}
      onConfirm={async () => { await call(api => api.removeSkillProject(forgetting.path)); reload(); }} onClose={() => setForgetting(null)}/>}
  </div>;
}

const TARGET_LABEL: Record<SkillTarget, string> = {'claude-code': 'Claude Code', agents: 'Codex and OpenCode'};
const STATE: Record<SkillInstall['state'], {label: string; tone: Tone}> = {
  installed: {label: 'Installed', tone: 'ok'},
  outdated: {label: 'Older version', tone: 'warn'},
  changed: {label: 'Edited there', tone: 'warn'},
  other: {label: 'Another skill of this name', tone: 'off'},
  absent: {label: 'Not installed', tone: 'off'},
};

function tokensText(n: number) {
  return t('about {count} tokens', {count: n.toLocaleString()});
}

function SkillRow({skill, busy, projectName, onInstall, onUninstall, onOpen, onDuplicate, onDelete}: {
  skill: Skill; busy: string | null; projectName: (path: string) => string; onInstall: (target: SkillTarget) => void;
  onUninstall: (target: SkillTarget) => void; onOpen: () => void; onDuplicate: () => void; onDelete: () => void;
}) {
  const global = skill.installs.filter(i => !i.project);
  const inProjects = skill.installs.filter(i => i.project && i.state !== 'absent');
  const items: MenuItem[] = [];
  for (const i of global) {
    const target = t(TARGET_LABEL[i.target]);
    if (i.state === 'absent') items.push({label: t('Install for {target}', {target}), onSelect: () => onInstall(i.target)});
    if (i.state === 'outdated') items.push({label: t('Update for {target}', {target}), onSelect: () => onInstall(i.target)});
    if (i.state === 'installed' || i.state === 'outdated') items.push({label: t('Remove from {target}', {target}), onSelect: () => onUninstall(i.target)});
  }
  items.push({label: skill.origin === 'pygg' ? t('View…') : t('Edit…'), onSelect: onOpen});
  items.push({label: t('Duplicate…'), onSelect: onDuplicate});
  if (skill.origin === 'mine') items.push({label: t('Delete…'), onSelect: onDelete, destructive: true});
  const working = busy?.startsWith(`${skill.id}:`);
  const absent = global.filter(i => i.state === 'absent');
  return <li className="agent-row skill-row">
    <span className="provider-logo" aria-hidden>{skill.manual ? <MessageSquareText size={15}/> : <Sparkles size={15}/>}</span>
    <span className="agent-text">
      <span className="agent-title"><span className="mono skill-name">/{skill.id}</span>
        <Badge tone="info">{skill.origin === 'pygg' ? t('Pygg') : t('Yours')}</Badge>
        <Badge tone="off">{skill.manual ? t('Prompt') : t('Skill')}</Badge></span>
      <small className="secondary">{skill.description}</small>
      <small className="secondary skill-cost">{skill.manual
        ? t('Costs nothing until you type /{name}; then {tokens}.', {name: skill.id, tokens: tokensText(skill.bodyTokens)})
        : t('Adds {listed} to every request where it is installed; {tokens} when used.', {listed: tokensText(skill.listedTokens), tokens: tokensText(skill.bodyTokens)})}</small>
      <span className="skill-installs">
        {global.map(i => <Badge key={i.target} tone={STATE[i.state].tone} title={i.path}>
          {t(TARGET_LABEL[i.target])}: {t(STATE[i.state].label)}</Badge>)}
        {inProjects.map(i => <Badge key={`${i.project}|${i.target}`} tone={STATE[i.state].tone} title={i.path}>
          {projectName(i.project!)} · {t(TARGET_LABEL[i.target])}: {t(STATE[i.state].label)}</Badge>)}
      </span>
    </span>
    <span className="row-actions">
      {working ? <Spinner label={t('Working…')}/>
        : absent.length > 0 && <button type="button" className="button small" disabled={busy !== null}
          onClick={() => absent.forEach(i => onInstall(i.target))}>{absent.length === 2 ? t('Install for All')
            : t('Install for {target}', {target: t(TARGET_LABEL[absent[0].target])})}</button>}
      <MenuButton label={t('Actions for {name}', {name: skill.id})} items={items}/>
    </span>
  </li>;
}

const KIND_LABEL: Record<ContextItem['kind'], string> = {instructions: 'Instructions', skill: 'Skill', command: 'Command', prompt: 'Prompt'};

function ContextList({items, busy, onImport}: {items: ContextItem[]; busy: string | null; onImport: (item: ContextItem) => void}) {
  return <ul className="context-items">
    {items.map(item => <li key={item.path} className="context-item">
      <span className="context-name mono" title={item.path}>{item.name}</span>
      <span className="secondary context-kind">{t(KIND_LABEL[item.kind])}{item.pygg ? ` · ${t('from Pygg')}` : ''}</span>
      <span className="context-tokens">{item.tokens > 0 ? tokensText(item.tokens) : t('only when used')}</span>
      {item.kind === 'skill' && !item.inLibrary && <button type="button" className="link-button" disabled={busy !== null}
        onClick={() => onImport(item)}>{t('Add to Library')}</button>}
    </li>)}
  </ul>;
}

function AgentRow({agent, busy, onImport}: {agent: AgentContext; busy: string | null; onImport: (item: ContextItem) => void}) {
  const counted = agent.items.filter(i => i.tokens > 0).length;
  return <li className="agent-row context-row">
    <span className="provider-logo" aria-hidden><FileText size={15}/></span>
    <span className="agent-text">
      <span className="agent-title">{agent.label}
        <Badge tone={agent.alwaysOnTokens > 5_000 ? 'warn' : agent.alwaysOnTokens > 0 ? 'info' : 'off'}>{tokensText(agent.alwaysOnTokens)}</Badge></span>
      {agent.items.length === 0
        ? <small className="secondary">{t('No global instruction file, skill or command found.')}</small>
        : <details className="context-details">
          <summary>{t('{count} files, {counted} sent with every request', {count: agent.items.length, counted})}</summary>
          <ContextList items={agent.items} busy={busy} onImport={onImport}/>
        </details>}
    </span>
  </li>;
}

function ProjectRow({project, busy, onImport, onManage, onForget}: {
  project: SkillProject; busy: string | null; onImport: (item: ContextItem) => void; onManage: () => void; onForget: () => void;
}) {
  const files = project.agents.reduce((n, a) => n + a.items.length, 0);
  const most = Math.max(0, ...project.agents.map(a => a.alwaysOnTokens));
  return <li className="agent-row project-row">
    <span className="provider-logo" aria-hidden><FolderGit2 size={15}/></span>
    <span className="agent-text">
      <span className="agent-title"><span className="skill-name">{project.name}</span>
        {project.missing ? <Badge tone="warn">{t('Folder not found')}</Badge>
          : <Badge tone={most > 5_000 ? 'warn' : most > 0 ? 'info' : 'off'}>{t('up to {tokens} more', {tokens: tokensText(most)})}</Badge>}
        {project.pyggSkills > 0 && <Badge tone="ok">{project.pyggSkills === 1 ? t('1 skill from Pygg')
          : t('{count} skills from Pygg', {count: project.pyggSkills})}</Badge>}</span>
      <small className="secondary mono project-path" title={project.path}>{project.path}</small>
      {!project.missing && <small className="secondary project-agents">{project.agents.map(a =>
        `${a.label}: ${a.alwaysOnTokens > 0 ? tokensText(a.alwaysOnTokens) : t('nothing')}`).join(' · ')}</small>}
      {files > 0 && <details className="context-details">
        <summary>{t('{count} files at the project’s root', {count: files})}</summary>
        {project.agents.filter(a => a.items.length > 0).map(a => <div key={a.agent} className="project-agent">
          <small className="project-agent-label">{a.label}</small>
          <ContextList items={a.items} busy={busy} onImport={onImport}/>
        </div>)}
      </details>}
    </span>
    <span className="row-actions">
      {!project.missing && <button type="button" className="button small" disabled={busy !== null} onClick={onManage}>{t('Install Skills…')}</button>}
      <MenuButton label={t('Actions for {name}', {name: project.name})} items={[
        ...(project.missing ? [] : [{label: t('Install Skills…'), onSelect: onManage}]),
        {label: t('Remove from Pygg…'), onSelect: onForget, destructive: true},
      ]}/>
    </span>
  </li>;
}

function AddProjectSheet({onClose, onAdded}: {onClose: () => void; onAdded: () => void}) {
  const [path, setPath] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const add = (folder: string) => {
    setBusy(true);
    setError(null);
    call(api => api.addSkillProject(folder.trim())).then(onAdded, e => { setError(errorText(e)); setBusy(false); });
  };
  const browse = () => call(api => api.pickFolder()).then(folder => { if (folder) { setPath(folder); add(folder); } },
    e => setError(errorText(e)));
  return <Sheet title={t('Add Project')} onClose={onClose} busy={busy} className="narrow" footer={<>
    <span className="spacer"/>
    <button type="button" className="button" onClick={onClose} disabled={busy}>{t('Cancel')}</button>
    <button type="button" className="button prominent" onClick={() => add(path)} disabled={busy || !path.trim()}>
      {busy ? <Spinner/> : null}{t('Add Project')}</button>
  </>}>
    <FormRow label={t('Folder')} htmlFor="skill-project-path" hint={t('The repository’s root folder, where its CLAUDE.md or AGENTS.md is.')}>
      <span className="input-with-icon"><FolderGit2 size={13} aria-hidden/>
        <input id="skill-project-path" className="text-input mono" value={path} onChange={e => setPath(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter' && path.trim() && !busy) add(path); }}
          placeholder={'C:\\path\\to\\repo'} spellCheck={false} disabled={busy}/>
        {runningInDesktop && <button type="button" className="button" onClick={browse} disabled={busy}>
          <FolderOpen size={13} aria-hidden/>{t('Browse…')}</button>}</span>
    </FormRow>
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}

/** The library's skills in one project: install, update or remove Pygg's copy in each of its two skills folders. */
function ProjectSkillsSheet({project, skills, onChanged, onClose}: {
  project: SkillProject; skills: Skill[]; onChanged: () => void; onClose: () => void;
}) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const run = (key: string, action: () => Promise<unknown>) => {
    setBusy(key);
    setError(null);
    action().then(onChanged, e => setError(errorText(e))).finally(() => setBusy(null));
  };
  return <Sheet title={t('Skills in {name}', {name: project.name})} subtitle={project.path} onClose={onClose} className="skill-sheet project-skills-sheet"
    footer={<><span className="spacer"/><button type="button" className="button prominent" onClick={onClose}>{t('Done')}</button></>}>
    {error && <ErrorNote>{error}</ErrorNote>}
    <ul className="project-skills">
      {skills.map(s => <li key={s.id} className="project-skill">
        <span className="project-skill-text">
          <span className="mono skill-name">/{s.id}</span>
          <small className="secondary">{s.description}</small>
        </span>
        <span className="project-skill-places">
          {s.installs.filter(i => i.project === project.path).map(i => {
            const key = `${s.id}:${i.target}`, target = t(TARGET_LABEL[i.target]);
            const install = () => run(key, () => call(api => api.installSkill(s.id, i.target, project.path)));
            const remove = () => run(key, () => call(api => api.uninstallSkill(s.id, i.target, project.path)));
            return <span key={i.target} className="project-skill-place">
              <Badge tone={STATE[i.state].tone} title={i.path}>{target}: {t(STATE[i.state].label)}</Badge>
              {busy === key ? <Spinner label={t('Working…')}/>
                : i.state === 'absent' ? <button type="button" className="link-button" disabled={busy !== null} onClick={install}
                  aria-label={t('Install {name} for {target}', {name: s.id, target})}>{t('Install')}</button>
                : i.state === 'installed' ? <button type="button" className="link-button" disabled={busy !== null} onClick={remove}
                  aria-label={t('Remove {name} from {target}', {name: s.id, target})}>{t('Remove')}</button>
                : i.state === 'outdated' ? <button type="button" className="link-button" disabled={busy !== null} onClick={install}
                  aria-label={t('Update {name} for {target}', {name: s.id, target})}>{t('Update')}</button>
                : null}
            </span>;
          })}
        </span>
      </li>)}
    </ul>
  </Sheet>;
}

function SkillSheet({initial, existing, readOnly, onClose, onSaved, onDuplicate}: {
  initial: SkillEdit; existing: boolean; readOnly: boolean; onClose: () => void; onSaved: () => void; onDuplicate: () => void;
}) {
  const [skill, setSkill] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const set = (patch: Partial<SkillEdit>) => setSkill(s => ({...s, ...patch}));
  const save = () => {
    setBusy(true);
    setError(null);
    call(api => api.saveSkill(skill)).then(onSaved, e => { setError(errorText(e)); setBusy(false); });
  };
  const title = readOnly ? `/${skill.id}` : existing ? t('Edit {name}', {name: skill.id}) : t('New Skill');
  return <Sheet title={title} subtitle={readOnly ? t('One of Pygg’s starter skills. Duplicate it to make your own version.') : undefined}
    onClose={onClose} busy={busy} className="skill-sheet" footer={<>
      <span className="spacer"/>
      <button type="button" className="button" onClick={onClose} disabled={busy}>{readOnly ? t('Close') : t('Cancel')}</button>
      {readOnly
        ? <button type="button" className="button prominent" onClick={onDuplicate}>{t('Duplicate…')}</button>
        : <button type="button" className="button prominent" onClick={save} disabled={busy}>{busy ? <Spinner/> : null}{t('Save')}</button>}
    </>}>
    <FormRow label={t('Name')} htmlFor="skill-name" hint={t('Typed as /name. Lowercase letters, digits and hyphens.')}>
      <input id="skill-name" className="text-input mono" value={skill.id} disabled={existing || readOnly} spellCheck={false}
        onChange={e => set({id: e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, '-')})}/>
    </FormRow>
    <FormRow label={t('Description')} htmlFor="skill-description" hint={t('What it does and when to use it. The agent reads only this to choose a skill.')}>
      <textarea id="skill-description" className="text-input" rows={2} value={skill.description} readOnly={readOnly}
        onChange={e => set({description: e.target.value})}/>
    </FormRow>
    <FormRow label={t('Instructions')} htmlFor="skill-body" hint={t('Read when the skill is used. $ARGUMENTS is what you type after the name.')}>
      <textarea id="skill-body" className="text-input mono skill-body" rows={12} value={skill.body} readOnly={readOnly} spellCheck={false}
        onChange={e => set({body: e.target.value})}/>
    </FormRow>
    <label className="check-row top">
      <input id="skill-manual" type="checkbox" checked={skill.manual} disabled={readOnly} onChange={e => set({manual: e.target.checked})}/>
      <span>{t('Run only when I type its name (a prompt)')}
        <small className="secondary">{t('Claude Code and OpenCode then leave it out of the list they send with every request. For Codex, Pygg also writes agents/openai.yaml beside it, so Codex leaves it out too; there you call it as $name.')}</small></span>
    </label>
    {error && <ErrorNote>{error}</ErrorNote>}
  </Sheet>;
}
