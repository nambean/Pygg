import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Bot, ChartColumn, Gauge, KeyRound, ListOrdered, ListTodo, PanelLeft, Plug, Plus, Settings as SettingsIcon,
  SlidersHorizontal } from 'lucide-react';
import './styles.css';
import brandMark from './assets/brand-mark.svg';
import { applyLanguage, LanguageContext, loadLanguage, resolve, t, type Language, type LanguageChoice } from './lib/i18n';
import { runningInDesktop, savedLanguage, saveLanguage } from './lib/native';
import { isMac, MOD } from './lib/platform';
import { SAMPLE_BUDGET, SAMPLE_WORKSPACES, sampleTasks } from './lib/sample';
import { FILTERS, matchesFilter, type TaskFilter, type TaskSummary } from './lib/tasks';
import type { GatewayStatus as Status } from './lib/api';
import { useGateway } from './hooks/useGateway';
import { BudgetSubtitle } from './components/BudgetSubtitle';
import { GatewayStatus } from './components/GatewayStatus';
import { NewTaskView } from './components/NewTaskView';
import { PendingView } from './components/PendingView';
import { SettingsView } from './components/SettingsView';
import { TaskDetail } from './components/TaskDetail';
import { TaskList } from './components/TaskList';
import { WindowControls } from './components/WindowControls';
import { Segmented } from './components/ui';
import { Clients } from './components/gateway/Clients';
import { Combos } from './components/gateway/Combos';
import { Connections } from './components/gateway/Connections';
import { Overview, type GatewayDestination } from './components/gateway/Overview';
import { Usage } from './components/gateway/Usage';
import { Agents } from './components/gateway/Agents';

type View =
  | {kind: GatewayDestination}
  | {kind: 'tasks'; filter: TaskFilter}
  | {kind: 'task'; id: string; from: TaskFilter}
  | {kind: 'new'}
  | {kind: 'policies'}
  | {kind: 'agents'}
  | {kind: 'settings'};
type Intent = 'connect' | 'create';

const GATEWAY: {kind: GatewayDestination; title: string; icon: React.ReactElement}[] = [
  {kind: 'overview', title: 'Overview', icon: <Gauge/>},
  {kind: 'connections', title: 'Connections', icon: <Plug/>},
  {kind: 'combos', title: 'Combos', icon: <ListOrdered/>},
  {kind: 'clients', title: 'Clients', icon: <KeyRound/>},
  {kind: 'usage', title: 'Usage', icon: <ChartColumn/>},
];
const FILTER_TITLES: Record<TaskFilter, string> = {
  all: 'All', running: 'Running', approval: 'Needs Approval', completed: 'Completed', failed: 'Failed', stopped: 'Stopped',
};
/** The toolbar's primary action on each gateway screen. */
const PRIMARY: Partial<Record<GatewayDestination, {label: string; intent: Intent}>> = {
  connections: {label: 'Connect Provider…', intent: 'connect'},
  combos: {label: 'New Combo…', intent: 'create'},
  clients: {label: 'Create Client Token…', intent: 'create'},
};

// Windows and Linux draw no title bar (tauri.windows.conf.json), so the app draws its own controls there.
const customChrome = runningInDesktop && !isMac;
const isGateway = (kind: View['kind']): kind is GatewayDestination => GATEWAY.some(g => g.kind === kind);

function App({language, onLanguage}: {language: LanguageChoice; onLanguage: (choice: LanguageChoice) => void}) {
  const [view, setView] = useState<View>({kind: 'overview'});
  const [intent, setIntent] = useState<Intent | undefined>();
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [query, setQuery] = useState('');
  const searchRef = useRef<HTMLInputElement>(null);

  const gateway = useGateway(api => api.gatewayStatus(), [], 5_000);
  const [status, setStatus] = useState<Status | null>(null);
  useEffect(() => { if (gateway.data) setStatus(gateway.data); }, [gateway.data]);

  // Smart task routing is a preview on sample data (ADR-014, ADR-016); the banner says so on every task screen.
  const [allTasks] = useState(() => sampleTasks());
  const workspaces = SAMPLE_WORKSPACES;
  const counts = useMemo(() => Object.fromEntries(FILTERS.map(f => [f, allTasks.filter(task => matchesFilter(task, f)).length])) as
    Record<TaskFilter, number>, [allTasks]);

  const go = useCallback((kind: GatewayDestination, next?: Intent) => { setView({kind}); setIntent(next); }, []);
  const intentDone = useCallback(() => setIntent(undefined), []);
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
      if (index >= 0 && index < GATEWAY.length) { event.preventDefault(); go(GATEWAY[index].kind); }
      else if (key === '6') { event.preventDefault(); setView({kind: 'tasks', filter: 'all'}); }
      else if (key === '7') { event.preventDefault(); setView({kind: 'agents'}); }
      else if (key === 'n' && (view.kind === 'tasks' || view.kind === 'task')) { event.preventDefault(); newTask(); }
      else if (key === ',') { event.preventDefault(); setView({kind: 'settings'}); }
      else if (key === 'f' && view.kind === 'tasks') { event.preventDefault(); searchRef.current?.focus(); }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [go, newTask, view.kind]);

  const openTask = (task: TaskSummary) => setView({kind: 'task', id: task.id, from: view.kind === 'tasks' ? view.filter : 'all'});
  const openedTask = view.kind === 'task' ? allTasks.find(task => task.id === view.id) : undefined;
  const title = isGateway(view.kind) ? t(GATEWAY.find(g => g.kind === view.kind)!.title)
    : view.kind === 'tasks' ? t('Tasks')
    : view.kind === 'task' ? openedTask?.title ?? t('Task')
    : view.kind === 'new' ? t('New Task')
    : view.kind === 'policies' ? t('Policies')
    : view.kind === 'agents' ? t('Agents')
    : t('Settings');
  const isTaskScreen = view.kind === 'tasks' || view.kind === 'task' || view.kind === 'new' || view.kind === 'policies';
  const primary = isGateway(view.kind) ? PRIMARY[view.kind] : undefined;

  return <div className={`window${isMac || customChrome ? ' has-lights' : ''}${sidebarOpen ? '' : ' sidebar-hidden'}`}>
    {customChrome && <WindowControls/>}
    <aside className="sidebar" aria-label={t('Sidebar')} hidden={!sidebarOpen}>
      <div className="sidebar-header" data-tauri-drag-region>
        <img src={brandMark} alt="" width={20} height={20}/>
        <span>SmartTaskRouter</span>
      </div>
      <nav>
        <SidebarGroup title={t('Gateway')}>
          {GATEWAY.map((g, i) => <SidebarItem key={g.kind} icon={g.icon} label={t(g.title)} shortcut={`${MOD}${i + 1}`}
            selected={view.kind === g.kind} onSelect={() => go(g.kind)}/>)}
        </SidebarGroup>
        <SidebarGroup title={t('Agents')}>
          <SidebarItem icon={<Bot/>} label={t('Agents')} shortcut={`${MOD}7`}
            selected={view.kind === 'agents'} onSelect={() => setView({kind: 'agents'})}/>
        </SidebarGroup>
        <SidebarGroup title={t('Smart Tasks · Preview')}>
          <SidebarItem icon={<ListTodo/>} label={t('Tasks')} count={counts.all} shortcut={`${MOD}6`}
            selected={view.kind === 'tasks' || view.kind === 'task' || view.kind === 'new'} onSelect={() => setView({kind: 'tasks', filter: 'all'})}/>
          <SidebarItem icon={<SlidersHorizontal/>} label={t('Policies')}
            selected={view.kind === 'policies'} onSelect={() => setView({kind: 'policies'})}/>
        </SidebarGroup>
        <SidebarGroup title={t('App')}>
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
          <h1>{title}</h1>
          {(view.kind === 'tasks' || view.kind === 'task') &&
            <BudgetSubtitle running={counts.running} waiting={counts.approval} budget={SAMPLE_BUDGET}/>}
        </div>
        <span className="spacer" data-tauri-drag-region/>
        {primary && <button type="button" className="button prominent" onClick={() => setIntent(primary.intent)}>
          <Plus size={14} strokeWidth={2.4} aria-hidden/>{t(primary.label)}
        </button>}
        {(view.kind === 'tasks' || view.kind === 'task') && <button type="button" className="button prominent" onClick={newTask}
          title={`${t('New Task')} (${MOD}N)`}>
          <Plus size={14} strokeWidth={2.4} aria-hidden/>{t('New Task')}
        </button>}
      </header>

      {(isGateway(view.kind) || view.kind === 'agents') && !runningInDesktop && <p className="banner sample" role="status">
        {t('Browser preview with demo data. Nothing is stored or sent; the desktop app runs the real gateway.')}
      </p>}
      {isTaskScreen && <p className="banner sample" role="status">
        {t('Preview of smart task routing on sample tasks. The task engine isn’t built yet, so nothing here ran.')}
      </p>}

      <div className="content">
        {view.kind === 'overview' && <Overview status={status} onStatus={setStatus} navigate={go}/>}
        {view.kind === 'connections' && <Connections intent={intent === 'connect' ? 'connect' : undefined} onIntentDone={intentDone}/>}
        {view.kind === 'combos' && <Combos intent={intent === 'create' ? 'create' : undefined} onIntentDone={intentDone}/>}
        {view.kind === 'clients' && <Clients intent={intent === 'create' ? 'create' : undefined} onIntentDone={intentDone}/>}
        {view.kind === 'usage' && <Usage/>}
        {view.kind === 'agents' && <Agents/>}
        {view.kind === 'tasks' && <>
          <div className="page-bar task-filters">
            <Segmented label={t('Show')} value={view.filter} onChange={filter => setView({kind: 'tasks', filter})}
              options={FILTERS.map(f => ({value: f, label: `${t(FILTER_TITLES[f])}${counts[f] ? ` ${counts[f]}` : ''}`}))}/>
          </div>
          <TaskList tasks={allTasks} filter={view.filter} workspaces={workspaces} scoped={false} query={query} onQuery={setQuery}
            searchRef={searchRef} onOpen={openTask} onNewTask={newTask} onWorkspaceSettings={null}/>
        </>}
        {view.kind === 'task' && openedTask && <TaskDetail task={openedTask}
          workspace={workspaces.find(w => w.id === openedTask.workspaceId)}
          onBack={() => setView({kind: 'tasks', filter: view.from})}/>}
        {view.kind === 'new' && <NewTaskView workspace={undefined} workspaces={workspaces} onWorkspace={() => undefined}/>}
        {view.kind === 'policies' && <PendingView title={t('Policies')} section="§5.5"
          purpose={t('Strategies, tiers, attempt limits, budgets and when paid steps need your approval.')}/>}
        {view.kind === 'settings' && <SettingsView language={language} onLanguage={onLanguage} onGateway={() => go('overview')}/>}
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
