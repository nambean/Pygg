import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { ChartColumn, CircleAlert, CircleCheck, CircleX, Folder, FolderPlus, Hourglass, ListTodo, LoaderCircle,
  PanelLeft, Plug, Plus, Settings as SettingsIcon, SlidersHorizontal, Waypoints } from 'lucide-react';
import './styles.css';
import brandMark from './assets/brand-mark.svg';
import { applyLanguage, LanguageContext, loadLanguage, resolve, t, type Language, type LanguageChoice } from './lib/i18n';
import { runningInDesktop, savedLanguage, saveLanguage } from './lib/native';
import { isMac, MOD } from './lib/platform';
import { SAMPLE_BUDGET, SAMPLE_WORKSPACES, sampleTasks } from './lib/sample';
import { FILTERS, matchesFilter, type TaskFilter, type TaskSummary } from './lib/tasks';
import { BudgetSubtitle } from './components/BudgetSubtitle';
import { GatewayStatus } from './components/GatewayStatus';
import { NewTaskView } from './components/NewTaskView';
import { PendingView } from './components/PendingView';
import { SettingsView } from './components/SettingsView';
import { TaskDetail } from './components/TaskDetail';
import { TaskList } from './components/TaskList';
import { WindowControls } from './components/WindowControls';

type Destination = 'connections' | 'policies' | 'usage' | 'endpoints' | 'settings';
type View =
  | {kind: 'tasks'; filter: TaskFilter}
  | {kind: 'task'; id: string; from: TaskFilter}
  | {kind: 'new'}
  | {kind: 'workspace-settings'}
  | {kind: 'add-workspace'}
  | {kind: Destination};

const FILTER_TITLES: Record<TaskFilter, string> = {
  all: 'All Tasks', running: 'Running', approval: 'Needs Approval', completed: 'Completed', failed: 'Failed',
  stopped: 'Stopped',
};
const DESTINATION_TITLES: Record<Destination, string> = {
  connections: 'Connections', policies: 'Policies', usage: 'Usage', endpoints: 'Endpoints', settings: 'Settings',
};

// Windows and Linux draw no title bar (tauri.windows.conf.json), so the app draws its own controls there.
const customChrome = runningInDesktop && !isMac;

function App({language, onLanguage}: {language: LanguageChoice; onLanguage: (choice: LanguageChoice) => void}) {
  const [view, setView] = useState<View>({kind: 'tasks', filter: 'all'});
  const [workspaceId, setWorkspaceId] = useState<string | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [query, setQuery] = useState('');
  const searchRef = useRef<HTMLInputElement>(null);

  // Sample data until the task engine exists; SampleBanner says so on every task screen.
  const [allTasks] = useState(() => sampleTasks());
  const workspaces = SAMPLE_WORKSPACES;
  const workspace = workspaces.find(w => w.id === workspaceId);
  const tasks = useMemo(() => workspaceId ? allTasks.filter(task => task.workspaceId === workspaceId) : allTasks,
    [allTasks, workspaceId]);
  const counts = useMemo(() => Object.fromEntries(FILTERS.map(f => [f, tasks.filter(task => matchesFilter(task, f)).length])) as
    Record<TaskFilter, number>, [tasks]);

  const showFilter = useCallback((filter: TaskFilter) => setView({kind: 'tasks', filter}), []);
  const newTask = useCallback(() => setView({kind: 'new'}), []);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (document.querySelector('[aria-modal="true"]')) return;   // an open sheet takes every key
      const mod = isMac ? event.metaKey : event.ctrlKey;
      if (!mod || event.altKey) return;
      const key = event.key.toLowerCase();
      if (event.shiftKey) {
        if (key === 'l') { event.preventDefault(); setSidebarOpen(open => !open); }
        return;
      }
      const index = Number(key) - 1;
      if (index >= 0 && index < FILTERS.length) { event.preventDefault(); showFilter(FILTERS[index]); }
      else if (key === 'n') { event.preventDefault(); newTask(); }
      else if (key === ',') { event.preventDefault(); setView({kind: 'settings'}); }
      else if (key === 'f' && view.kind === 'tasks') { event.preventDefault(); searchRef.current?.focus(); }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [newTask, showFilter, view.kind]);

  const openTask = (task: TaskSummary) => setView({kind: 'task', id: task.id,
    from: view.kind === 'tasks' ? view.filter : 'all'});
  const selectWorkspace = (id: string) => {
    setWorkspaceId(current => current === id ? null : id);
    if (view.kind !== 'tasks') setView({kind: 'tasks', filter: 'all'});
  };

  const scopeName = workspace ? ` · ${workspace.name}` : '';
  const openedTask = view.kind === 'task' ? allTasks.find(task => task.id === view.id) : undefined;
  const title = view.kind === 'tasks' ? t(FILTER_TITLES[view.filter]) + scopeName
    : view.kind === 'task' ? openedTask?.title ?? t('Task')
    : view.kind === 'new' ? t('New Task') + scopeName
    : view.kind === 'workspace-settings' ? t('Workspace Settings') + scopeName
    : view.kind === 'add-workspace' ? t('Add Workspace')
    : t(DESTINATION_TITLES[view.kind]);
  const isTaskScreen = view.kind === 'tasks' || view.kind === 'task' || view.kind === 'new';

  return <div className={`window${isMac || customChrome ? ' has-lights' : ''}${sidebarOpen ? '' : ' sidebar-hidden'}`}>
    {customChrome && <WindowControls/>}
    <aside className="sidebar" aria-label={t('Sidebar')} hidden={!sidebarOpen}>
      <div className="sidebar-header" data-tauri-drag-region>
        <img src={brandMark} alt="" width={20} height={20}/>
        <span>SmartTaskRouter</span>
      </div>
      <nav>
        <SidebarGroup title={t('Tasks')}>
          <SidebarItem icon={<ListTodo/>} label={t('All Tasks')} count={counts.all} shortcut={`${MOD}1`}
            selected={view.kind === 'tasks' && view.filter === 'all'} onSelect={() => showFilter('all')}/>
          <SidebarItem icon={<LoaderCircle/>} label={t('Running')} count={counts.running} shortcut={`${MOD}2`}
            selected={view.kind === 'tasks' && view.filter === 'running'} onSelect={() => showFilter('running')}/>
          <SidebarItem icon={<Hourglass/>} label={t('Needs Approval')} count={counts.approval} emphasized shortcut={`${MOD}3`}
            selected={view.kind === 'tasks' && view.filter === 'approval'} onSelect={() => showFilter('approval')}/>
          <SidebarItem icon={<CircleCheck/>} label={t('Completed')} count={counts.completed} shortcut={`${MOD}4`}
            selected={view.kind === 'tasks' && view.filter === 'completed'} onSelect={() => showFilter('completed')}/>
          <SidebarItem icon={<CircleAlert/>} label={t('Failed')} count={counts.failed} shortcut={`${MOD}5`}
            selected={view.kind === 'tasks' && view.filter === 'failed'} onSelect={() => showFilter('failed')}/>
          <SidebarItem icon={<CircleX/>} label={t('Stopped')} count={counts.stopped} shortcut={`${MOD}6`}
            selected={view.kind === 'tasks' && view.filter === 'stopped'} onSelect={() => showFilter('stopped')}/>
        </SidebarGroup>
        <SidebarGroup title={t('Workspaces')}>
          {workspaces.map(w => <SidebarItem key={w.id} icon={<Folder/>} label={w.name} title={w.path}
            selected={workspaceId === w.id} onSelect={() => selectWorkspace(w.id)}/>)}
          <SidebarItem icon={<FolderPlus/>} label={t('Add Workspace…')}
            selected={view.kind === 'add-workspace'} onSelect={() => setView({kind: 'add-workspace'})}/>
        </SidebarGroup>
        <SidebarGroup title={t('Manage')}>
          <SidebarItem icon={<Plug/>} label={t('Connections')}
            selected={view.kind === 'connections'} onSelect={() => setView({kind: 'connections'})}/>
          <SidebarItem icon={<SlidersHorizontal/>} label={t('Policies')}
            selected={view.kind === 'policies'} onSelect={() => setView({kind: 'policies'})}/>
          <SidebarItem icon={<ChartColumn/>} label={t('Usage')}
            selected={view.kind === 'usage'} onSelect={() => setView({kind: 'usage'})}/>
          <SidebarItem icon={<Waypoints/>} label={t('Endpoints')}
            selected={view.kind === 'endpoints'} onSelect={() => setView({kind: 'endpoints'})}/>
        </SidebarGroup>
        <SidebarGroup title={t('App')}>
          <SidebarItem icon={<SettingsIcon/>} label={t('Settings')} shortcut={`${MOD},`}
            selected={view.kind === 'settings'} onSelect={() => setView({kind: 'settings'})}/>
        </SidebarGroup>
      </nav>
      <div className="sidebar-footer"><GatewayStatus/><span className="app-version-number">v0.1.0</span></div>
    </aside>

    <main className="main">
      <header className="toolbar" data-tauri-drag-region>
        <button className="toolbar-button" type="button" onClick={() => setSidebarOpen(open => !open)}
          aria-label={sidebarOpen ? t('Hide sidebar') : t('Show sidebar')}
          title={`${sidebarOpen ? t('Hide sidebar') : t('Show sidebar')} (${MOD}Shift+L)`} aria-pressed={sidebarOpen}>
          <PanelLeft size={17} strokeWidth={1.75}/>
        </button>
        <div className="toolbar-title" data-tauri-drag-region>
          <h1>{title}</h1>
          {isTaskScreen && <BudgetSubtitle running={counts.running} waiting={counts.approval} budget={SAMPLE_BUDGET}/>}
        </div>
        <span className="spacer" data-tauri-drag-region/>
        {isTaskScreen && view.kind !== 'new' && <button type="button" className="button prominent" onClick={newTask}
          title={`${t('New Task')} (${MOD}N)`}>
          <Plus size={14} strokeWidth={2.4} aria-hidden/>{t('New Task')}
        </button>}
      </header>

      {isTaskScreen && <p className="banner sample" role="status">
        {t('Sample tasks for design review. The task engine isn’t built yet, so nothing here ran.')}
      </p>}

      <div className="content">
        {view.kind === 'tasks' && <TaskList tasks={tasks} filter={view.filter} workspaces={workspaces}
          scoped={workspaceId !== null} query={query} onQuery={setQuery} searchRef={searchRef}
          onOpen={openTask} onNewTask={newTask}
          onWorkspaceSettings={workspace ? () => setView({kind: 'workspace-settings'}) : null}/>}
        {view.kind === 'task' && openedTask && <TaskDetail task={openedTask}
          workspace={workspaces.find(w => w.id === openedTask.workspaceId)}
          onBack={() => showFilter(view.from)}/>}
        {view.kind === 'new' && <NewTaskView workspace={workspace} workspaces={workspaces}
          onWorkspace={id => setWorkspaceId(id)}/>}
        {view.kind === 'workspace-settings' && <PendingView title={t('Workspace Settings')} section="§5.3"
          purpose={t('Folder, Git and isolation readiness, privacy class, approved checks and policy overrides for one workspace.')}/>}
        {view.kind === 'add-workspace' && <PendingView title={t('Add Workspace')} section="§5.3"
          purpose={t('Choose a folder, check that it can be isolated with Git, detect its checks and set its privacy class.')}/>}
        {view.kind === 'connections' && <PendingView title={t('Connections')} section="§5.4"
          purpose={t('Providers and accounts, the model catalog with where each fact comes from, and the agent that runs tasks.')}/>}
        {view.kind === 'policies' && <PendingView title={t('Policies')} section="§5.5"
          purpose={t('Strategies, tiers, attempt limits, budgets and when paid steps need your approval.')}/>}
        {view.kind === 'usage' && <PendingView title={t('Usage')} section="§5.6"
          purpose={t('The usage ledger with actual, estimated and unknown costs kept apart, plus subscription and local use.')}/>}
        {view.kind === 'endpoints' && <PendingView title={t('Endpoints')} section="§5.7"
          purpose={t('The local gateway address, client tokens and setup for SDKs and agents.')}/>}
        {view.kind === 'settings' && <SettingsView language={language} onLanguage={onLanguage}/>}
      </div>
    </main>
  </div>;
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
      title={shortcut ? `${label} (${shortcut})` : title}>
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
