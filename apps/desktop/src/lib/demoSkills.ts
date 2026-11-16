/**
 * Skills & Prompts in the browser preview (ADR-039): the same starters as skills.rs, an in-memory library, and agent
 * folders and project folders shaped like a real machine's. Nothing is written anywhere. `?empty` has no agent files; `?stress` has many,
 * with long names and descriptions.
 */
import type { AgentContext, ContextItem, GatewayApi, Skill, SkillEdit, SkillProject, SkillTarget } from './api';

type SkillsApi = Pick<GatewayApi, 'skills' | 'saveSkill' | 'deleteSkill' | 'installSkill' | 'uninstallSkill' | 'skillsContext' | 'importSkill'
  | 'skillProjects' | 'addSkillProject' | 'removeSkillProject'>;

/** As the gateway estimates: three ASCII characters a token, every other character a token of its own. */
const tokens = (text: string) => {
  let ascii = 0, other = 0;
  for (const c of text) (c.charCodeAt(0) < 128 ? ascii++ : other++);
  return Math.ceil(ascii / 3) + other;
};

const STARTERS: {id: string; description: string; body: string}[] = [
  {id: 'brief-answers', description: 'Answer with as few output tokens as the task allows for the rest of the session: no restating, no recap, changed code as diffs.',
    body: 'For the rest of this session, keep answers short. Output tokens cost the most.\n\n- Do not restate the question or the plan you were given.\n- Show changed code as a unified diff or the edited lines only, never a whole unchanged file.\n- No closing summary of what you did unless asked; one line saying what changed is enough.\n- Ask one question only when you cannot go on without the answer.\n- Keep explanations to the reason a reader could not see from the code.\n'},
  {id: 'fix-failing-test', description: 'Find and fix the cause of a failing test with the smallest change, without editing the test.',
    body: 'Fix the failing test named in $ARGUMENTS (or the first failing one if none is named).\n\n1. Run only that test and read the failure.\n2. Find the cause in the code under test. Read only the files the failure points to.\n3. Make the smallest change that fixes the cause. Do not edit, skip or weaken the test.\n4. Run the test again, then the tests of the module you changed.\n5. Reply with the cause in one sentence and the diff.\n'},
  {id: 'review-diff', description: 'Review the current changes for correctness bugs only, with file and line for each finding.',
    body: 'Review the uncommitted changes (or the branch\'s changes against main if there are none).\n\n- Look for defects: wrong logic, missed cases, errors swallowed, races, security holes, broken callers.\n- Skip style, naming and formatting.\n- For each finding: `path:line`, what goes wrong, and the input that makes it go wrong.\n- Rank by severity. Say "no findings" when there are none; do not invent any.\n'},
  {id: 'write-tests', description: 'Write focused tests for the named code: behaviour and edge cases, matching the project\'s style.',
    body: 'Write tests for $ARGUMENTS.\n\n1. Read the code and one existing test file next to it, and follow its style and helpers.\n2. Cover the main behaviour, each error path, and the edges (empty, one, many, limits).\n3. One behaviour per test, named for what it checks.\n4. Run the new tests. If one fails because the code is wrong, say so instead of changing the test to pass.\n'},
  {id: 'plan-first', description: 'Plan a change before editing: the files, the steps and the risks, then wait for approval.',
    body: 'Before changing anything for $ARGUMENTS:\n\n1. Read what you need to understand the change, no more.\n2. Reply with a plan: the files to change and why, the steps in order, how you will check it, and the risks.\n3. Stop and wait for approval. Do not edit files until the plan is approved.\n'},
  {id: 'explain-error', description: 'Explain an error message or stack trace: the cause, where it starts, and the fix.',
    body: 'Explain the error in $ARGUMENTS (or the last error in this session).\n\n- The cause in plain words, in one or two sentences.\n- Where it starts: the first frame or line in this project\'s code.\n- The fix, as a diff if it is small. If the cause is unclear, the one check that would settle it.\n'},
  {id: 'commit-message', description: 'Write a commit message for the staged changes in the project\'s own commit style.',
    body: 'Write a commit message for the staged changes.\n\n1. Read `git diff --staged` and the last ten commit subjects (`git log --oneline -10`) for the style.\n2. A subject under 72 characters that says what changes for a user or a caller.\n3. A short body only if the reason is not obvious from the diff.\n4. Print the message; do not commit.\n'},
  {id: 'handoff-note', description: 'Write a short handoff note so a fresh session or another model can continue this work.',
    body: 'Write a handoff note for this session, under 300 words, so a new session can continue without the history (a long history is re-sent with every request).\n\n- Goal: what the user asked for, in their words where it matters.\n- Done: what is finished and checked.\n- State: files changed, branch, anything uncommitted.\n- Next: the remaining steps in order, and the command that checks each.\n- Decisions and constraints a newcomer would get wrong.\n\nPrint the note only.\n'},
];

const HOME = 'C:\\Users\\you';
const DIRS: Record<SkillTarget, string> = {'claude-code': `${HOME}\\.claude\\skills`, agents: `${HOME}\\.agents\\skills`};
const dirOf = (target: SkillTarget, project?: string | null) =>
  project ? `${project}\\${target === 'claude-code' ? '.claude' : '.agents'}\\skills` : DIRS[target];
/** One place a skill can be installed: a target, globally ('') or in a project. */
const placeKey = (target: SkillTarget, project?: string | null) => `${project ?? ''}|${target}`;

export function demoSkills(): SkillsApi {
  const params = new URLSearchParams(location.search);
  const empty = params.has('empty'), stress = params.has('stress');
  const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
  const touch = async <T>(value: T): Promise<T> => { await sleep(120); return structuredClone(value); };

  const mine: {id: string; description: string; body: string; manual: boolean; updatedAt: number}[] = empty ? [] : [
    {id: 'release-notes', description: 'Draft release notes from the commits since the last tag, grouped for users.', body: 'Read `git log` since the last tag and write release notes: New, Changed, Fixed. One line each, for users, not developers.\n', manual: true, updatedAt: Date.now() - 3 * 86_400_000},
  ];
  if (stress) mine.push({id: 'database-migration-reviewer-for-the-billing-service-and-its-reporting-jobs',
    description: 'Reviews a new database migration for the billing service: locks taken on large tables, backfills that run in one transaction, columns dropped while the reporting jobs still read them, and indexes created without CONCURRENTLY on PostgreSQL.',
    body: 'Review the migration in $ARGUMENTS.\n'.repeat(40), manual: false, updatedAt: Date.now() - 86_400_000});

  /** Project folders added for skills, with the instruction files at their roots (tokens by file name). */
  const projects: {path: string; addedAt: number; missing: boolean; files: Record<string, number>}[] = empty ? [] : [
    {path: `${HOME}\\code\\billing-api`, addedAt: Date.now() - 5 * 86_400_000, missing: false, files: {'CLAUDE.md': 2_380, 'AGENTS.md': 1_140}},
    {path: `${HOME}\\code\\web`, addedAt: Date.now() - 2 * 86_400_000, missing: false, files: {'AGENTS.md': 420}},
  ];
  if (stress) projects.push(
    {path: `${HOME}\\source\\repos\\payments-platform-monorepo-with-a-very-long-folder-name\\services\\settlement-reconciliation`,
      addedAt: Date.now() - 86_400_000, missing: false, files: {'CLAUDE.md': 14_200, 'CLAUDE.local.md': 900, 'AGENTS.md': 6_100}},
    {path: `${HOME}\\code\\old-prototype`, addedAt: Date.now() - 40 * 86_400_000, missing: true, files: {}});

  /** What is in each place, by skill name: `pygg` copies carry the text they were written from. */
  const installed = new Map<string, Map<string, {text: string; pygg: boolean}>>();
  const at = (target: SkillTarget, project?: string | null) => {
    const key = placeKey(target, project);
    if (!installed.has(key)) installed.set(key, new Map());
    return installed.get(key)!;
  };
  if (!empty) {
    at('claude-code').set('review-diff', {text: textOf(STARTERS[2].id, STARTERS[2].description, STARTERS[2].body, true), pygg: true});
    at('claude-code').set('deploy', {text: 'deploy|Deploys the web app to staging, then production after the smoke tests pass.', pygg: false});
    at('agents').set('fix-failing-test', {text: textOf(STARTERS[1].id, STARTERS[1].description, 'An earlier version.', true), pygg: true});
    at('claude-code', projects[0].path).set('release-notes', {text: textOf(mine[0].id, mine[0].description, mine[0].body, true), pygg: true});
    at('agents', projects[0].path).set('migrations', {text: 'migrations|Checks new migrations against the billing schema rules before review.', pygg: false});
  }
  if (stress) {
    at('claude-code').set('brief-answers', {text: 'edited', pygg: true});
    at('agents').set('write-tests', {text: 'theirs', pygg: false});
    at('agents', projects[2].path).set('review-diff', {text: 'An earlier version.', pygg: true});
  }

  function textOf(id: string, description: string, body: string, manual: boolean) { return `${id}|${description}|${manual}|${body}`; }
  function all() {
    return [...STARTERS.map(s => ({...s, manual: true, origin: 'pygg' as const, updatedAt: null})),
      ...mine.map(m => ({...m, origin: 'mine' as const}))];
  }
  const places = () => [null, ...projects.map(p => p.path)].flatMap(project => (['claude-code', 'agents'] as const).map(target => ({target, project})));
  function view(s: ReturnType<typeof all>[number]): Skill {
    const expected = textOf(s.id, s.description, s.body, s.manual);
    return {...s, listedTokens: s.manual ? 0 : tokens(`${s.id}: ${s.description}`), bodyTokens: tokens(s.body),
      installs: places().map(({target, project}) => {
        const there = at(target, project).get(s.id);
        const state = !there ? 'absent' : !there.pygg ? 'other' : there.text === 'edited' ? 'changed' : there.text === expected ? 'installed' : 'outdated';
        return {target, project, state, path: `${dirOf(target, project)}\\${s.id}\\SKILL.md`};
      })};
  }
  const find = (id: string) => {
    const s = all().find(s => s.id === id);
    if (!s) throw `There is no skill ${id}.`;
    return s;
  };
  const listed = (project?: string | null) => {
    if (project && !projects.some(p => p.path === project)) throw `${project} is not one of the projects added to Skills & Prompts.`;
  };

  const library = () => all().map(s => s.id);
  // Codex honours Pygg's agents/openai.yaml, which every prompt Pygg installs in an .agents\skills folder carries.
  const skillItems = (target: SkillTarget, project?: string | null): ContextItem[] => [...at(target, project)].map(([name, v]) => {
    const s = all().find(s => s.id === name);
    const description = s?.description ?? v.text.split('|')[1] ?? '';
    const manual = s?.manual ?? false;
    return {kind: 'skill', name, path: `${dirOf(target, project)}\\${name}\\SKILL.md`, tokens: manual ? 0 : tokens(`${name}: ${description}`),
      fullTokens: tokens(s?.body ?? description) + 40, manual, pygg: v.pygg && v.text !== 'edited', inLibrary: library().includes(name)};
  });
  const file = (name: string, path: string, size: number): ContextItem =>
    ({kind: 'instructions', name, path, tokens: size, fullTokens: size, manual: false, pygg: false, inLibrary: false});
  const agent = (agent: AgentContext['agent'], label: string, items: ContextItem[]): AgentContext => {
    items.sort((a, b) => b.tokens - a.tokens || a.name.localeCompare(b.name));
    return {agent, label, alwaysOnTokens: items.reduce((n, i) => n + i.tokens, 0), items};
  };
  const once = (items: ContextItem[]) => items.filter((item, i, list) => list.findIndex(other => other.name === item.name) === i);
  const projectView = (p: typeof projects[number]): SkillProject => {
    const f = (name: string) => p.files[name] != null ? [file(name, `${p.path}\\${name}`, p.files[name])] : [];
    const pyggSkills = all().filter(s => view(s).installs.some(i => i.project === p.path && ['installed', 'outdated', 'changed'].includes(i.state))).length;
    return {path: p.path, name: p.path.split('\\').pop()!, addedAt: p.addedAt, missing: p.missing, pyggSkills,
      agents: p.missing ? [] : [
        agent('claude-code', 'Claude Code', [...f('CLAUDE.md'), ...f('CLAUDE.local.md'), ...skillItems('claude-code', p.path)]),
        agent('codex', 'Codex', [...f('AGENTS.md'), ...skillItems('agents', p.path)]),
        agent('opencode', 'OpenCode', [...(p.files['AGENTS.md'] != null ? f('AGENTS.md') : f('CLAUDE.md')),
          ...once([...skillItems('claude-code', p.path), ...skillItems('agents', p.path)])]),
      ]};
  };
  const projectList = () => touch([...projects].sort((a, b) => a.path.localeCompare(b.path)).map(projectView));

  return {
    skills: () => touch(all().map(view)),
    saveSkill: async (edit: SkillEdit) => {
      if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(edit.id) || edit.id.length > 64)
        throw 'A skill\'s name is lowercase letters, digits and single hyphens, at most 64 characters, such as review-diff.';
      if (!edit.description.trim()) throw 'Describe what the skill does and when to use it: the agent reads only this to choose it.';
      if (edit.description.trim().length > 1024) throw 'Keep the description under 1,024 characters (the Agent Skills limit).';
      if (!edit.body.trim()) throw 'Write the skill\'s instructions.';
      if (STARTERS.some(s => s.id === edit.id)) throw `${edit.id} is one of Pygg's starter skills; save your version under another name.`;
      const i = mine.findIndex(m => m.id === edit.id);
      const row = {id: edit.id, description: edit.description.trim(), body: edit.body.trimEnd(), manual: edit.manual, updatedAt: Date.now()};
      if (i >= 0) mine[i] = row; else mine.push(row);
      return touch(view({...row, origin: 'mine'}));
    },
    deleteSkill: async id => {
      if (STARTERS.some(s => s.id === id)) throw `${id} is one of Pygg's starter skills and cannot be deleted.`;
      const s = view(find(id));
      const there = s.installs.find(i => i.state === 'installed' || i.state === 'outdated' || i.state === 'changed');
      if (there) throw `${id} is installed in ${there.path}; remove it there first.`;
      mine.splice(mine.findIndex(m => m.id === id), 1);
      await sleep(120);
    },
    installSkill: async (id, target, project) => {
      listed(project);
      if (projects.find(p => p.path === project)?.missing) throw `${project} no longer exists.`;
      const s = find(id), state = view(s).installs.find(i => i.target === target && (i.project ?? null) === (project ?? null))!.state;
      if (state === 'other') throw `${dirOf(target, project)} already holds a skill named ${id} that Pygg did not write; it was left as it is.`;
      if (state === 'changed') throw `${dirOf(target, project)}\\${id}\\SKILL.md was edited after Pygg installed it; it was left as it is. Remove or rename it to install again.`;
      at(target, project).set(id, {text: textOf(s.id, s.description, s.body, s.manual), pygg: true});
      return touch(view(s));
    },
    uninstallSkill: async (id, target, project) => {
      listed(project);
      const there = at(target, project).get(id), path = `${dirOf(target, project)}\\${id}\\SKILL.md`;
      if (there && !there.pygg) throw `${path} was not written by Pygg; it was left as it is.`;
      if (there?.text === 'edited') throw `${path} was edited after Pygg installed it; it was left as it is.`;
      at(target, project).delete(id);
      return touch(view(find(id)));
    },
    skillsContext: async () => {
      if (empty) return touch([agent('claude-code', 'Claude Code', []), agent('codex', 'Codex', []), agent('opencode', 'OpenCode', [])]);
      const claudeMd = file('CLAUDE.md', `${HOME}\\.claude\\CLAUDE.md`, stress ? 9_840 : 1_210);
      const claude = [claudeMd, ...skillItems('claude-code'),
        {kind: 'command' as const, name: 'standup', path: `${HOME}\\.claude\\commands\\standup.md`, tokens: 14, fullTokens: 160, manual: false, pygg: false, inLibrary: false}];
      if (stress) for (let i = 0; i < 14; i++) claude.push({kind: 'skill', name: `team-convention-${i + 1}-for-the-payments-monorepo-and-its-services`,
        path: `${HOME}\\.claude\\skills\\team-convention-${i + 1}-for-the-payments-monorepo-and-its-services\\SKILL.md`, tokens: 60 + i * 7, fullTokens: 2_400, manual: false, pygg: false, inLibrary: false});
      const codex = [file('AGENTS.md', `${HOME}\\.codex\\AGENTS.md`, 640), ...skillItems('agents'),
        {kind: 'prompt' as const, name: 'refactor', path: `${HOME}\\.codex\\prompts\\refactor.md`, tokens: 0, fullTokens: 210, manual: true, pygg: false, inLibrary: false}];
      // OpenCode has no AGENTS.md of its own here, so it reads Claude Code's CLAUDE.md.
      const opencode = [claudeMd, ...once([...skillItems('claude-code'), ...skillItems('agents')])];
      return touch([agent('claude-code', 'Claude Code', claude), agent('codex', 'Codex', codex), agent('opencode', 'OpenCode', opencode)]);
    },
    importSkill: async path => {
      const name = /\\([^\\]+)\\SKILL\.md$/.exec(path)?.[1];
      if (!name) throw 'Only a skill from an agent\'s skills folder can be added to the library.';
      if (all().some(s => s.id === name)) throw `The library already has a skill named ${name}.`;
      const there = [...installed.values()].map(m => m.get(name)).find(Boolean);
      const row = {id: name, description: there?.text.split('|')[1] ?? name, body: 'Instructions copied from the agent\'s folder.\n', manual: false, updatedAt: Date.now()};
      mine.push(row);
      return touch(view({...row, origin: 'mine'}));
    },
    skillProjects: () => projectList(),
    addSkillProject: async path => {
      const clean = path.trim().replace(/[\\/]+$/, '').replace(/\//g, '\\');
      if (!/^[A-Za-z]:\\./.test(clean)) throw `${path} is not a folder Pygg can read.`;
      if (clean.toLowerCase() === HOME.toLowerCase())
        throw 'That is your home folder, whose .claude and .agents folders are the agents\' global ones. Use Install for Claude Code or for Codex and OpenCode instead.';
      if (!projects.some(p => p.path.toLowerCase() === clean.toLowerCase())) projects.push({path: clean, addedAt: Date.now(), missing: false, files: {'AGENTS.md': 310}});
      return projectList();
    },
    removeSkillProject: async path => {
      const p = projects.find(p => p.path === path);
      const copies = p ? projectView(p).pyggSkills : 0;
      if (copies > 0) throw `Pygg still has ${copies} skill${copies === 1 ? '' : 's'} installed in ${path}; remove ${copies === 1 ? 'it' : 'them'} there first.`;
      projects.splice(projects.findIndex(p => p.path === path), 1);
      return projectList();
    },
  };
}
