import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { BookMarked, BookOpen, Bot, ChartColumn, ChevronLeft, FolderGit2, House, Inbox, Link2, ListOrdered, PanelLeft, Plug, Plus, Search, Settings as SettingsIcon,
  SlidersHorizontal, TerminalSquare } from 'lucide-react';
import './styles.css';
import brandMark from './assets/brand-mark.svg';
import { applyLanguage, LanguageContext, loadLanguage, resolve, t, type Language, type LanguageChoice } from './lib/i18n';
import { runningInDesktop, savedLanguage, saveLanguage } from './lib/native';
import { isMac, MOD } from './lib/platform';
import { FILTERS, matchesFilter, needsAttention, type Brainstorm, type TaskFilter, type TaskSummary, brainstormNeedsAttention } from './lib/tasks';
import type { GatewayStatus as Status } from './lib/api';
import { useGateway } from './hooks/useGateway';
import { gatewayChanged } from './lib/api';
import { onTasksChanged, sendShellStrings } from './lib/shell';
import { BudgetSubtitle } from './components/BudgetSubtitle';
import { CommandPalette, type Command } from './components/CommandPalette';
import { GatewayStatus } from './components/GatewayStatus';
import { Guide } from './components/Guide';
import { NewTaskView, type WorkMode } from './components/NewTaskView';
import { SettingsView } from './components/SettingsView';
import { TaskDetail } from './components/TaskDetail';
import { BrainstormDetail } from './components/BrainstormDetail';
import { BrainStorming } from './components/BrainStorming';
import { ProjectsView } from './components/ProjectsView';
import { ProjectView, type ProjectTab } from './components/ProjectView';
import { WorkList } from './components/WorkList';
import { needsChecks, WorkspacesView } from './components/WorkspacesView';
import { WindowControls } from './components/WindowControls';
import { ErrorNote, Spinner, useRowHints } from './components/ui';
import { CliTools } from './components/gateway/CliTools';
import { Combos } from './components/gateway/Combos';
import { Endpoint } from './components/gateway/Endpoint';
import { Connections } from './components/gateway/Connections';
import { Overview, type GatewayDestination } from './components/gateway/Overview';
import { Usage } from './components/gateway/Usage';
import { Policies } from './components/gateway/Policies';
import { Agents } from './components/gateway/Agents';
import { Skills } from './components/gateway/Skills';

// A task, a plan or the New work form remembers the page it was opened from, and its back link returns there.
type View =
  | {kind: GatewayDestination}
  | {kind: 'work'}
  | {kind: 'projects'}
  | {kind: 'project'; id: string; tab: ProjectTab; back?: View; plan?: string}
  | {kind: 'task'; id: string; back: View}
  | {kind: 'brainstorm'; id: string; back: View}
  | {kind: 'brain'}
  | {kind: 'new'; workspaceId: string | null; mode: WorkMode; back: View}
  | {kind: 'policies'}
  | {kind: 'skills'}
  | {kind: 'workspaces'; back?: View}
  | {kind: 'agents'}
  | {kind: 'guide'}
  | {kind: 'settings'};
type Intent = 'connect' | 'create' | 'unpriced' | 'auto' | `requests:${string}` | `failed:${string}`;

// The gateway's screens in the order of setting it up, named as 9router and OmniRoute name theirs (SCREENS.md §2).
const GATEWAY: {kind: GatewayDestination; title: string; icon: React.ReactElement}[] = [
  {kind: 'overview', title: 'Home', icon: <House/>},
  {kind: 'connections', title: 'Providers', icon: <Plug/>},
  {kind: 'endpoint', title: 'Endpoint', icon: <Link2/>},
  {kind: 'tools', title: 'CLI Tools', icon: <TerminalSquare/>},
  {kind: 'combos', title: 'Combos', icon: <ListOrdered/>},
  {kind: 'usage', title: 'Usage', icon: <ChartColumn/>},
];
/** The toolbar's primary action on each gateway screen. */
const PRIMARY: Partial<Record<GatewayDestination, {label: string; intent: Intent}>> = {
  connections: {label: 'Connect Provider…', intent: 'connect'},
  combos: {label: 'New Combo…', intent: 'create'},
  endpoint: {label: 'Create API Key…', intent: 'create'},
};

// Windows and Linux draw no title bar (tauri.windows.conf.json), so the app draws its own controls there.
const customChrome = runningInDesktop && !isMac;
const isGateway = (kind: View['kind']): kind is GatewayDestination => GATEWAY.some(g => g.kind === kind);

/** The browser preview can open on a screen (`?screen=usage`, `?screen=guide`), for reviews and screenshots; the app
 *  itself always starts on Home. */
function startView(): View {
  if (runningInDesktop) return {kind: 'overview'};
  const screen = new URLSearchParams(location.search).get('screen') ?? '';
  if (isGateway(screen as View['kind'])) return {kind: screen as GatewayDestination};
  if (screen === 'guide' || screen === 'settings' || screen === 'agents' || screen === 'policies' || screen === 'skills') return {kind: screen};
  // The demo's brainstorms (lib/demo.ts), for the layout check and screenshots of the pygg/brain screens.
  if (screen === 'brainstorm') return {kind: 'brainstorm', id: 'bs-proposed', back: {kind: 'work'}};
  if (screen === 'brainstorm-pick') return {kind: 'brainstorm', id: 'bs-pick', back: {kind: 'work'}};
  if (screen === 'brainstorm-failed') return {kind: 'brainstorm', id: 'bs-failed', back: {kind: 'work'}};
  if (screen === 'plan-running') return {kind: 'brainstorm', id: 'bs-running', back: {kind: 'work'}};
  if (screen === 'board') return {kind: 'project', id: '', tab: 'board'};
  if (screen === 'brain') return {kind: 'brain'};
  if (screen === 'work' || screen === 'tasks') return {kind: 'work'};
  if (screen === 'projects') return {kind: 'projects'};
  if (screen === 'project') return {kind: 'project', id: '', tab: 'work'};
  if (screen === 'new') return {kind: 'new', workspaceId: null, mode: 'task', back: {kind: 'work'}};
  if (screen === 'new-plan') return {kind: 'new', workspaceId: null, mode: 'plan', back: {kind: 'work'}};
  return {kind: 'overview'};
}

function App({language, onLanguage}: {language: LanguageChoice; onLanguage: (choice: LanguageChoice) => void}) {
  const [view, setView] = useState<View>(startView);
  const [intent, setIntent] = useState<Intent | undefined>();
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [palette, setPalette] = useState(false);

  const gateway = useGateway(api => api.gatewayStatus(), [], 5_000);
  const [status, setStatus] = useState<Status | null>(null);
  useEffect(() => { if (gateway.data) setStatus(gateway.data); }, [gateway.data]);

  // Tasks and workspaces from the core (tasks.rs); live while anything runs. The browser preview shows sample tasks.
  const taskList = useGateway(api => api.tasks(), [], 3_000);
  // A task's runner stopped (one from an IDE included): every screen reloads at once, not at the next poll.
  useEffect(() => onTasksChanged(gatewayChanged), []);
  const workspaceList = useGateway(api => api.workspaces());
  // Brainstorms from pygg/brain (ADR-041): rows beside the tasks, live while one thinks or waits.
  const brainList = useGateway(api => api.brainstorms(), [], 3_000);
  const boardList = useGateway(api => api.boards(), [], 10_000);
  const brainstorms = useMemo(() => brainList.data ?? [], [brainList.data]);
  const budget = useGateway(api => api.taskBudget(), [], 10_000);
  const allTasks = useMemo(() => taskList.data ?? [], [taskList.data]);
  const workspaces = workspaceList.data ?? [];
  const counts = useMemo(() => Object.fromEntries(FILTERS.map(f => [f, allTasks.filter(task => matchesFilter(task, f)).length])) as
    Record<TaskFilter, number>, [allTasks]);
  const attention = allTasks.filter(needsAttention).length + brainstorms.filter(brainstormNeedsAttention).length;

  const go = useCallback((kind: GatewayDestination, next?: Intent) => { setView({kind}); setIntent(next); }, []);
  const intentDone = useCallback(() => setIntent(undefined), []);
  // New work opens from the page in view and returns there; on a project's page it starts in that project.
  const newWork = useCallback((mode: WorkMode = 'task') => setView(current => ({kind: 'new', mode, back: current.kind === 'new' ? current.back : current,
    workspaceId: current.kind === 'project' ? current.id || null : current.kind === 'new' ? current.workspaceId : null})), []);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (document.querySelector('[aria-modal="true"]')) return;   // an open sheet takes every key
      const mod = isMac ? event.metaKey : event.ctrlKey;
      if (!mod || event.altKey) return;
      const key = event.key.toLowerCase();
      if (key === 'k' && !event.shiftKey) { event.preventDefault(); setPalette(true); return; }
      if (event.shiftKey) {
        if (key === 'l') { event.preventDefault(); setSidebarOpen(open => !open); }
        return;
      }
      const index = Number(key) - 1;
      if (index >= 0 && index < GATEWAY.length) { event.preventDefault(); go(GATEWAY[index].kind); }
      else if (key === '7') { event.preventDefault(); setView({kind: 'work'}); }
      else if (key === '8') { event.preventDefault(); setView({kind: 'projects'}); }
      else if (key === '9') { event.preventDefault(); setView({kind: 'agents'}); }
      else if (key === 'n') { event.preventDefault(); newWork(); }
      else if (key === ',') { event.preventDefault(); setView({kind: 'settings'}); }
      else if (key === '/') { event.preventDefault(); setView({kind: 'guide'}); }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [go, newWork]);

  // The palette's commands (SCREENS.md §2.6). None spends money or deletes: those open the screen that asks.
  const commands = useMemo<Command[]>(() => {
    const nav = t('Go to');
    const list: Command[] = [
      ...GATEWAY.map((g, i) => ({id: `go-${g.kind}`, group: nav, label: t(g.title), hint: `${MOD}${i + 1}`, run: () => go(g.kind)})),
      {id: 'go-work', group: nav, label: t('Work'), hint: `${MOD}7`, keywords: 'tasks plans questions', run: () => setView({kind: 'work'})},
      {id: 'go-projects', group: nav, label: t('Projects'), hint: `${MOD}8`, keywords: 'repositories workspaces', run: () => setView({kind: 'projects'})},
      {id: 'go-agents', group: nav, label: t('Agents & defaults'), hint: `${MOD}9`, run: () => setView({kind: 'agents'})},
      {id: 'go-workspaces', group: nav, label: t('Project settings'), run: () => setView({kind: 'workspaces'})},
      {id: 'go-policies', group: nav, label: t('Policies'), run: () => setView({kind: 'policies'})},
      {id: 'go-skills', group: nav, label: t('Skills & Prompts'), keywords: 'prompt hub library slash command claude.md agents.md context tokens',
        run: () => setView({kind: 'skills'})},
      {id: 'go-settings', group: nav, label: t('Settings'), hint: `${MOD},`, run: () => setView({kind: 'settings'})},
      {id: 'go-guide', group: nav, label: t('Guide'), hint: `${MOD}/`, keywords: 'help how to commands cli npm docs',
        run: () => setView({kind: 'guide'})},
    ];
    const gateway = t('Gateway');
    list.push(
      {id: 'connect', group: gateway, label: t('Connect Provider…'), keywords: 'add key api', run: () => go('connections', 'connect')},
      {id: 'combo', group: gateway, label: t('New Combo…'), keywords: 'fallback', run: () => go('combos', 'create')},
      {id: 'client', group: gateway, label: t('Create API Key…'), keywords: 'key token client', run: () => go('endpoint', 'create')},
      {id: 'auto', group: gateway, label: t('Simulate auto routing'), keywords: 'auto cheap route', run: () => go('combos')},
      {id: 'usage-export', group: gateway, label: t('Open Usage to export CSV'), keywords: 'ledger csv', run: () => go('usage')},
      {id: 'ide', group: gateway, label: t('Set up a coding tool'), keywords: 'claude code codex opencode cline continue cli', run: () => go('tools')},
      {id: 'new-skill', group: gateway, label: t('New Skill…'), keywords: 'prompt slash command', run: () => { setView({kind: 'skills'}); setIntent('create'); }},
      {id: 'mcp', group: gateway, label: t('Let an IDE agent delegate tasks (MCP)'), keywords: 'mcp agent smart task', run: () => setView({kind: 'agents'})},
    );
    const work = t('Smart Tasks');
    list.push(
      {id: 'new-task', group: work, label: t('New task'), hint: `${MOD}N`, keywords: 'change solo', run: () => newWork('task')},
      {id: 'new-ask', group: work, label: t('Ask about a project'), keywords: 'question explain read architecture', run: () => newWork('ask')},
      {id: 'new-plan', group: work, label: t('Plan a change (Brain Storming)'), keywords: 'brainstorm plan cards board', run: () => newWork('plan')},
    );
    const app = t('App');
    list.push(
      {id: 'sidebar', group: app, label: sidebarOpen ? t('Hide sidebar') : t('Show sidebar'), hint: `${MOD}Shift+L`,
        run: () => setSidebarOpen(open => !open)},
      {id: 'diagnostics', group: app, label: t('Open Settings to save diagnostics'), keywords: 'support bug report', run: () => setView({kind: 'settings'})},
      {id: 'language', group: app, label: t('Language'), keywords: 'english tiếng việt', run: () => setView({kind: 'settings'})},
    );
    return list;
  }, [go, newWork, sidebarOpen]);

  const openTaskId = (id: string) => { taskList.reload(); setView({kind: 'task', id, back: view}); };
  const openTask = (task: TaskSummary) => openTaskId(task.id);
  const openPlan = (plan: Brainstorm) => setView({kind: 'brainstorm', id: plan.id, back: view});
  const projectId = view.kind === 'project' ? (view.id || workspaces[0]?.id || '') : '';
  const openedProject = workspaces.find(w => w.id === projectId);
  const openedTask = view.kind === 'task' ? allTasks.find(task => task.id === view.id) : undefined;
  const openedPlan = view.kind === 'brainstorm' ? brainstorms.find(b => b.id === view.id) : undefined;
  // What a back link says: the page it returns to.
  const backLabel = (back: View): string => back.kind === 'work' ? t('Work') : back.kind === 'projects' ? t('Projects')
    : back.kind === 'project' ? workspaces.find(w => w.id === back.id)?.name ?? t('Project')
    : back.kind === 'brainstorm' ? t('Plan') : back.kind === 'task' ? t('Task') : t('Back');
  const goBack = (back: View) => { taskList.reload(); brainList.reload(); boardList.reload(); setView(back); };
  // New work opens what it made; its back link returns to where New work was opened from.
  const created = (id: string) => { taskList.reload(); workspaceList.reload(); setView({kind: 'task', id, back: view.kind === 'new' ? view.back : view}); };
  const planCreated = (id: string) => { brainList.reload(); setView({kind: 'brainstorm', id, back: view.kind === 'new' ? view.back : view}); };
  const title = isGateway(view.kind) ? t(GATEWAY.find(g => g.kind === view.kind)!.title)
    : view.kind === 'work' ? t('Work')
    : view.kind === 'projects' ? t('Projects')
    : view.kind === 'project' ? openedProject?.name ?? t('Project')
    : view.kind === 'task' ? openedTask?.title ?? t('Task')
    : view.kind === 'brainstorm' ? (openedPlan ? t('Plan: {title}', {title: openedPlan.requirement.split('\n')[0].slice(0, 80)}) : t('Plan'))
    : view.kind === 'brain' ? t('Agents & defaults')
    : view.kind === 'new' ? t('New work')
    : view.kind === 'policies' ? t('Policies')
    : view.kind === 'skills' ? t('Skills & Prompts')
    : view.kind === 'workspaces' ? t('Project settings')
    : view.kind === 'agents' ? t('Agents & defaults')
    : view.kind === 'guide' ? t('Guide')
    : t('Settings');
  const isTaskScreen = view.kind === 'work' || view.kind === 'projects' || view.kind === 'project' || view.kind === 'task' || view.kind === 'brainstorm' || view.kind === 'brain' || view.kind === 'new' || view.kind === 'workspaces';
  const primary = isGateway(view.kind) ? PRIMARY[view.kind] : undefined;
  useRowHints();
  // After moving to another page, focus goes to its heading so a screen reader says where it is and Tab starts at the
  // top of the page, unless the page already put focus in itself (a search field, a sheet). Not on start.
  const heading = useRef<HTMLHeadingElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const page = view.kind === 'task' || view.kind === 'brainstorm' ? `${view.kind}:${view.id}` : view.kind;
  const firstPage = useRef(true);
  useEffect(() => {
    if (firstPage.current) { firstPage.current = false; return; }
    const id = requestAnimationFrame(() => {
      const active = document.activeElement;
      if (active && active !== document.body && content.current?.contains(active)) return;
      if (active?.closest('[role="dialog"]')) return;
      heading.current?.focus({preventScroll: true});
    });
    return () => cancelAnimationFrame(id);
  }, [page]);

  return <div className={`window${isMac || customChrome ? ' has-lights' : ''}${sidebarOpen ? '' : ' sidebar-hidden'}`}>
    <a className="skip-link" href="#content" onClick={e => { e.preventDefault(); content.current?.focus(); }}>{t('Skip to content')}</a>
    {customChrome && <WindowControls/>}
    <aside className="sidebar" aria-label={t('Sidebar')} hidden={!sidebarOpen}>
      <div className="sidebar-header" data-tauri-drag-region>
        <img src={brandMark} alt="" width={20} height={20}/>
        <span>Pygg</span>
      </div>
      <nav>
        <SidebarGroup title={t('Gateway')}>
          {GATEWAY.map((g, i) => <SidebarItem key={g.kind} icon={g.icon} label={t(g.title)} shortcut={`${MOD}${i + 1}`}
            selected={view.kind === g.kind} onSelect={() => go(g.kind)}/>)}
          <SidebarItem icon={<SlidersHorizontal/>} label={t('Policies')}
            selected={view.kind === 'policies'} onSelect={() => setView({kind: 'policies'})}/>
          <SidebarItem icon={<BookMarked/>} label={t('Skills & Prompts')}
            selected={view.kind === 'skills'} onSelect={() => setView({kind: 'skills'})}/>
        </SidebarGroup>
        <SidebarGroup title={t('Smart Tasks · Experimental')}>
          <SidebarItem icon={<Inbox/>} label={t('Work')} count={attention} emphasized shortcut={`${MOD}7`}
            selected={view.kind === 'work' || view.kind === 'task' || view.kind === 'brainstorm' || view.kind === 'new'} onSelect={() => setView({kind: 'work'})}/>
          <SidebarItem icon={<FolderGit2/>} label={t('Projects')} count={workspaces.filter(needsChecks).length} shortcut={`${MOD}8`}
            selected={view.kind === 'projects' || view.kind === 'project' || view.kind === 'workspaces'} onSelect={() => setView({kind: 'projects'})}/>
          <SidebarItem icon={<Bot/>} label={t('Agents & defaults')} shortcut={`${MOD}9`}
            selected={view.kind === 'agents' || view.kind === 'brain'} onSelect={() => setView({kind: 'agents'})}/>
        </SidebarGroup>
        <SidebarGroup title={t('App')}>
          <SidebarItem icon={<BookOpen/>} label={t('Guide')} shortcut={`${MOD}/`}
            selected={view.kind === 'guide'} onSelect={() => setView({kind: 'guide'})}/>
          <SidebarItem icon={<SettingsIcon/>} label={t('Settings')} shortcut={`${MOD},`}
            selected={view.kind === 'settings'} onSelect={() => setView({kind: 'settings'})}/>
        </SidebarGroup>
      </nav>
      <div className="sidebar-footer">
        <GatewayStatus status={status} error={gateway.error} onOpen={() => go('overview')}/>
        <span className="app-version-number">v0.1.0</span>
      </div>
    </aside>

    <main className="main">
      <header className="toolbar" data-tauri-drag-region>
        <button className="toolbar-button" type="button" onClick={() => setSidebarOpen(open => !open)}
          aria-label={sidebarOpen ? t('Hide sidebar') : t('Show sidebar')}
          title={`${sidebarOpen ? t('Hide sidebar') : t('Show sidebar')} (${MOD}Shift+L)`} aria-pressed={sidebarOpen}>
          <PanelLeft size={17} strokeWidth={1.75}/>
        </button>
        <div className="toolbar-title" data-tauri-drag-region>
          <h1 ref={heading} tabIndex={-1}>{title}</h1>
          {(view.kind === 'work' || view.kind === 'task') &&
            <BudgetSubtitle running={counts.running} waiting={counts.approval} budget={budget.data ?? undefined}/>}
        </div>
        <span className="spacer" data-tauri-drag-region/>
        <button className="toolbar-button" type="button" onClick={() => setPalette(true)} aria-label={t('Command palette')}
          title={`${t('Command palette')} (${MOD}K)`}><Search size={16} strokeWidth={1.75}/></button>
        {primary && <button type="button" className="button prominent" onClick={() => setIntent(primary.intent)}>
          <Plus size={14} strokeWidth={2.4} aria-hidden/>{t(primary.label)}
        </button>}
        {view.kind === 'skills' && <button type="button" className="button prominent" onClick={() => setIntent('create')}>
          <Plus size={14} strokeWidth={2.4} aria-hidden/>{t('New Skill…')}
        </button>}
        {(view.kind === 'work' || view.kind === 'projects' || view.kind === 'task' || view.kind === 'brainstorm') && <button type="button" className="button prominent"
          onClick={() => newWork()} title={`${t('New work')} (${MOD}N)`}>
          <Plus size={14} strokeWidth={2.4} aria-hidden/>{t('New work')}
        </button>}
      </header>

      {(isGateway(view.kind) || view.kind === 'agents' || view.kind === 'policies' || view.kind === 'skills' || isTaskScreen) && !runningInDesktop && <p className="banner sample" role="status">
        {t('Browser preview with demo data. Nothing is stored or sent; the desktop app runs the real gateway.')}
      </p>}

      <div className="content" id="content" ref={content} tabIndex={-1}>
        {view.kind === 'overview' && <Overview status={status} onStatus={setStatus} navigate={go}/>}
        {view.kind === 'connections' && <Connections intent={intent === 'connect' || intent === 'unpriced' ? intent : undefined}
          onIntentDone={intentDone} onNext={() => go('tools')}/>}
        {view.kind === 'combos' && <Combos intent={intent === 'create' || intent === 'auto' ? intent : undefined} onIntentDone={intentDone}
          onConnect={() => go('connections', 'connect')}/>}
        {view.kind === 'endpoint' && <Endpoint status={status} onStatus={setStatus} intent={intent === 'create' ? 'create' : undefined}
          onIntentDone={intentDone}/>}
        {view.kind === 'tools' && <CliTools status={status} navigate={go}/>}
        {view.kind === 'usage' && <Usage onOpenTools={() => go('tools')} onIntentDone={intentDone}
          intent={intent?.startsWith('requests:') || intent?.startsWith('failed:') ? intent : undefined}/>}
        {(view.kind === 'agents' || view.kind === 'brain') && <>
          <Agents/>
          <BrainStorming onSkills={() => setView({kind: 'skills'})} onWorkspaces={() => setView({kind: 'workspaces'})}/>
        </>}
        {(view.kind === 'work' || view.kind === 'project') && <LoadErrors sources={[
          {what: t('Tasks'), error: taskList.data ? null : taskList.error, loading: taskList.loading, retry: taskList.reload},
          {what: t('Plans'), error: brainList.data ? null : brainList.error, loading: brainList.loading, retry: brainList.reload}]}/>}
        {view.kind === 'work' && (!taskList.data || !brainList.data
          ? (taskList.error || brainList.error ? null : <div className="page"><Spinner label={t('Loading work…')}/></div>)
          : <WorkList tasks={allTasks} brainstorms={brainstorms} workspaces={workspaces} onOpenTask={openTask} onOpenPlan={openPlan}
              onNewWork={() => newWork()}/>)}
        {view.kind === 'projects' && <ProjectsView tasks={allTasks} brainstorms={brainstorms} workspaces={workspaces}
          onOpenProject={id => setView({kind: 'project', id, tab: 'work'})} onChanged={() => { workspaceList.reload(); taskList.reload(); }}/>}
        {view.kind === 'project' && (openedProject
          ? <ProjectView workspace={openedProject} tasks={allTasks.filter(task => task.workspaceId === openedProject.id)}
              brainstorms={brainstorms.filter(b => b.workspaceId === openedProject.id)} board={(boardList.data ?? []).find(b => b.workspaceId === openedProject.id) ?? null}
              tab={view.tab} onTab={tab => setView({...view, id: openedProject.id, tab})}
              plan={view.plan ? {id: view.plan, title: brainstorms.find(b => b.id === view.plan)?.requirement.split('\n')[0] ?? ''} : null}
              onAllPlans={() => setView({...view, plan: undefined})}
              backTo={view.back ? backLabel(view.back) : t('Projects')} onBack={() => goBack(view.back ?? {kind: 'projects'})}
              onOpenTask={openTask} onOpenTaskId={openTaskId} onOpenPlan={openPlan} onNewWork={() => newWork()}
              onSettings={() => setView({kind: 'workspaces', back: view})}/>
          : <div className="page">{workspaceList.data ? <p className="group-footnote">{t('That project is no longer here.')}</p> : <Spinner/>}</div>)}
        {view.kind === 'skills' && <Skills intent={intent === 'create' ? 'create' : undefined} onIntentDone={intentDone}/>}
        {view.kind === 'guide' && <Guide go={go} base={status?.baseUrl ?? (status ? `http://127.0.0.1:${status.port}` : null)}/>}
        {view.kind === 'task' && <TaskDetail key={view.id} id={view.id} backTo={backLabel(view.back)} onBack={() => goBack(view.back)}/>}
        {view.kind === 'brainstorm' && <BrainstormDetail key={view.id} id={view.id} tasks={allTasks} backTo={backLabel(view.back)}
          onBack={() => goBack(view.back)} onOpenTask={openTaskId}
          onOpenBoard={workspaceId => { boardList.reload(); taskList.reload(); setView({kind: 'project', id: workspaceId, tab: 'board', back: view, plan: view.id}); }}/>}
        {view.kind === 'new' && <div className="detail-back-wrap">
          <button type="button" className="back-button" onClick={() => setView(view.back)}>
            <ChevronLeft size={15} strokeWidth={2} aria-hidden/>{backLabel(view.back)}</button>
          <NewTaskView onCreated={created} onPlanCreated={planCreated} workspaceId={view.workspaceId} mode={view.mode}
            onMode={mode => setView({...view, mode})}/></div>}
        {view.kind === 'workspaces' && <>
          {view.back && <div className="detail-back-wrap"><button type="button" className="back-button" onClick={() => setView(view.back!)}>
            <ChevronLeft size={15} strokeWidth={2} aria-hidden/>{backLabel(view.back)}</button></div>}
          <WorkspacesView/></>}
        {view.kind === 'policies' && <Policies go={go} onSettings={() => setView({kind: 'settings'})}
          onWorkspaces={() => setView({kind: 'workspaces'})}/>}
        {view.kind === 'settings' && <SettingsView language={language} onLanguage={onLanguage} onGateway={() => go('endpoint')}/>}
      </div>
    </main>
    {palette && <CommandPalette commands={commands} onClose={() => setPalette(false)}/>}
  </div>;
}

/** A list that could not load, each with its own Try Again, so a failed part never reads as "nothing here". */
function LoadErrors({sources}: {sources: {what: string; error: string | null | undefined; loading: boolean; retry: () => void}[]}) {
  const failed = sources.filter(x => x.error);
  if (failed.length === 0) return null;
  return <div className="page">{failed.map(x => <ErrorNote key={x.what}>{t('{what} could not be loaded: {error}', {what: x.what, error: x.error ?? ''})}{' '}
    <button type="button" className="link-button" onClick={x.retry} disabled={x.loading}>{x.loading ? t('Trying again…') : t('Try Again')}</button></ErrorNote>)}</div>;
}

function SidebarGroup({title, children}: {title: string; children: React.ReactNode}) {
  return <section className="sidebar-group" aria-label={title}>
    <h2>{title}</h2>
    <ul>{children}</ul>
  </section>;
}

function SidebarItem({icon, label, title, count, selected, emphasized, shortcut, onSelect}: {
  icon: React.ReactElement; label: string; title?: string; count?: number; selected: boolean;
  emphasized?: boolean; shortcut?: string; onSelect: () => void;
}) {
  return <li>
    <button type="button" className={selected ? 'sidebar-item selected' : 'sidebar-item'}
      aria-current={selected ? 'page' : undefined} onClick={onSelect}
      title={shortcut ? `${label} (${shortcut})` : title ?? label}>
      {React.cloneElement(icon as React.ReactElement<{size: number; strokeWidth: number}>, {size: 16, strokeWidth: 1.9})}
      <span>{label}</span>
      {count ? <span className={emphasized ? 'count emphasized' : 'count'}>{count}</span> : null}
    </button>
  </li>;
}

/** Fades out the start-up screen from index.html once the interface has rendered. */
function dismissSplash() {
  const splash = document.getElementById('splash');
  if (!splash) return;
  splash.classList.add('done');
  setTimeout(() => splash.remove(), 300);
}

/** Loads the chosen language before the first paint; a change re-renders the interface in place. */
function Localized() {
  const [choice, setChoice] = useState<LanguageChoice>(savedLanguage);
  const [lang, setLang] = useState<Language | null>(null);
  useEffect(() => {
    const next = resolve(choice);
    let current = true;
    loadLanguage(next).catch(() => undefined).then(() => {
      if (!current) return;
      applyLanguage(next);
      sendShellStrings();
      setLang(next);
    });
    return () => { current = false; };
  }, [choice]);
  useEffect(() => { if (lang) dismissSplash(); }, [lang]);
  if (!lang) return null;
  const change = (next: LanguageChoice) => { saveLanguage(next); setChoice(next); };
  return <LanguageContext.Provider value={lang}><App language={choice} onLanguage={change}/></LanguageContext.Provider>;
}

createRoot(document.getElementById('root')!).render(<React.StrictMode><Localized/></React.StrictMode>);
