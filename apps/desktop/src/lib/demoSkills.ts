/**
 * Skills & Prompts in the browser preview (ADR-039): the same starters as skills.rs, an in-memory library, and agent
 * folders shaped like a real machine's. Nothing is written anywhere. `?empty` has no agent files; `?stress` has many,
 * with long names and descriptions.
 */
import type { AgentContext, ContextItem, GatewayApi, Skill, SkillEdit, SkillTarget } from './api';

type SkillsApi = Pick<GatewayApi, 'skills' | 'saveSkill' | 'deleteSkill' | 'installSkill' | 'uninstallSkill' | 'skillsContext' | 'importSkill'>;

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

const DIRS: Record<SkillTarget, string> = {'claude-code': 'C:\\Users\\you\\.claude\\skills', agents: 'C:\\Users\\you\\.agents\\skills'};

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
  /** What is in each agent folder, by target and skill name: `pygg` copies carry the text they were written from. */
  const installed: Record<SkillTarget, Map<string, {text: string; pygg: boolean}>> = {'claude-code': new Map(), agents: new Map()};
  if (!empty) {
    installed['claude-code'].set('review-diff', {text: textOf(STARTERS[2].id, STARTERS[2].description, STARTERS[2].body, true), pygg: true});
    installed['claude-code'].set('deploy', {text: 'deploy|Deploys the web app to staging, then production after the smoke tests pass.', pygg: false});
    installed.agents.set('fix-failing-test', {text: textOf(STARTERS[1].id, STARTERS[1].description, 'An earlier version.', true), pygg: true});
  }
  if (stress) {
    installed['claude-code'].set('brief-answers', {text: 'edited', pygg: true});
    installed.agents.set('write-tests', {text: 'theirs', pygg: false});
  }

  function textOf(id: string, description: string, body: string, manual: boolean) { return `${id}|${description}|${manual}|${body}`; }
  function all() {
    return [...STARTERS.map(s => ({...s, manual: true, origin: 'pygg' as const, updatedAt: null})),
      ...mine.map(m => ({...m, origin: 'mine' as const}))];
  }
  function view(s: ReturnType<typeof all>[number]): Skill {
    const expected = textOf(s.id, s.description, s.body, s.manual);
    return {...s, listedTokens: s.manual ? 0 : tokens(`${s.id}: ${s.description}`), bodyTokens: tokens(s.body),
      installs: (['claude-code', 'agents'] as const).map(target => {
        const there = installed[target].get(s.id);
        const state = !there ? 'absent' : !there.pygg ? 'other' : there.text === 'edited' ? 'changed' : there.text === expected ? 'installed' : 'outdated';
        return {target, state, path: `${DIRS[target]}\\${s.id}\\SKILL.md`};
      })};
  }
  const find = (id: string) => {
    const s = all().find(s => s.id === id);
    if (!s) throw `There is no skill ${id}.`;
    return s;
  };

  return {
    skills: () => touch(all().map(view)),
    saveSkill: async (edit: SkillEdit) => {
      if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(edit.id) || edit.id.length > 64)
        throw 'A skill\'s name is lowercase letters, digits and single hyphens, at most 64 characters, such as review-diff.';
      if (!edit.description.trim()) throw 'Describe what the skill does and when to use it: the agent reads only this to choose it.';
      if (edit.description.trim().length > 1024) throw 'Keep the description under 1,024 characters (the Agent Skills limit).';
      if (!edit.body.trim()) throw 'Write the skill\'s instructions.';
      if (STARTERS.some(s => s.id === edit.id)) throw `${edit.id} is one of Pygg's starter skills; save your version under another name.`;
      const at = mine.findIndex(m => m.id === edit.id);
      const row = {id: edit.id, description: edit.description.trim(), body: edit.body.trimEnd(), manual: edit.manual, updatedAt: Date.now()};
      if (at >= 0) mine[at] = row; else mine.push(row);
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
    installSkill: async (id, target) => {
      const s = find(id), state = view(s).installs.find(i => i.target === target)!.state;
      if (state === 'other') throw `${DIRS[target]} already holds a skill named ${id} that Pygg did not write; it was left as it is.`;
      if (state === 'changed') throw `${DIRS[target]}\\${id}\\SKILL.md was edited after Pygg installed it; it was left as it is. Remove or rename it to install again.`;
      installed[target].set(id, {text: textOf(s.id, s.description, s.body, s.manual), pygg: true});
      return touch(view(s));
    },
    uninstallSkill: async (id, target) => {
      const there = installed[target].get(id), path = `${DIRS[target]}\\${id}\\SKILL.md`;
      if (there && !there.pygg) throw `${path} was not written by Pygg; it was left as it is.`;
      if (there?.text === 'edited') throw `${path} was edited after Pygg installed it; it was left as it is.`;
      installed[target].delete(id);
      return touch(view(find(id)));
    },
    skillsContext: async () => {
      const library = all().map(s => s.id);
      const skillItems = (target: SkillTarget, honoursManual: boolean): ContextItem[] => [...installed[target]].map(([name, v]) => {
        const s = all().find(s => s.id === name);
        const description = s?.description ?? v.text.split('|')[1] ?? '';
        const manual = s?.manual ?? false;
        return {kind: 'skill', name, path: `${DIRS[target]}\\${name}\\SKILL.md`, tokens: manual && honoursManual ? 0 : tokens(`${name}: ${description}`),
          fullTokens: tokens(s?.body ?? description) + 40, manual, pygg: v.pygg && v.text !== 'edited', inLibrary: library.includes(name)};
      });
      const file = (name: string, path: string, size: number): ContextItem =>
        ({kind: 'instructions', name, path, tokens: size, fullTokens: size, manual: false, pygg: false, inLibrary: false});
      const agent = (agent: AgentContext['agent'], label: string, items: ContextItem[]): AgentContext => {
        items.sort((a, b) => b.tokens - a.tokens || a.name.localeCompare(b.name));
        return {agent, label, alwaysOnTokens: items.reduce((n, i) => n + i.tokens, 0), items};
      };
      if (empty) return touch([agent('claude-code', 'Claude Code', []), agent('codex', 'Codex', []), agent('opencode', 'OpenCode', [])]);
      const claude = [file('CLAUDE.md', 'C:\\Users\\you\\.claude\\CLAUDE.md', stress ? 9_840 : 1_210), ...skillItems('claude-code', true),
        {kind: 'command' as const, name: 'standup', path: 'C:\\Users\\you\\.claude\\commands\\standup.md', tokens: 14, fullTokens: 160, manual: false, pygg: false, inLibrary: false}];
      if (stress) for (let i = 0; i < 14; i++) claude.push({kind: 'skill', name: `team-convention-${i + 1}-for-the-payments-monorepo-and-its-services`,
        path: `C:\\Users\\you\\.claude\\skills\\team-convention-${i + 1}-for-the-payments-monorepo-and-its-services\\SKILL.md`, tokens: 60 + i * 7, fullTokens: 2_400, manual: false, pygg: false, inLibrary: false});
      const codex = [file('AGENTS.md', 'C:\\Users\\you\\.codex\\AGENTS.md', 640), ...skillItems('agents', false),
        {kind: 'prompt' as const, name: 'refactor', path: 'C:\\Users\\you\\.codex\\prompts\\refactor.md', tokens: 0, fullTokens: 210, manual: true, pygg: false, inLibrary: false}];
      const opencode = [...skillItems('claude-code', true), ...skillItems('agents', true)]
        .filter((item, i, list) => list.findIndex(other => other.name === item.name) === i);
      return touch([agent('claude-code', 'Claude Code', claude), agent('codex', 'Codex', codex), agent('opencode', 'OpenCode', opencode)]);
    },
    importSkill: async path => {
      const name = /\\([^\\]+)\\SKILL\.md$/.exec(path)?.[1];
      if (!name) throw 'Only a skill from an agent\'s skills folder can be added to the library.';
      if (all().some(s => s.id === name)) throw `The library already has a skill named ${name}.`;
      const there = installed['claude-code'].get(name) ?? installed.agents.get(name);
      const row = {id: name, description: there?.text.split('|')[1] ?? name, body: 'Instructions copied from the agent\'s folder.\n', manual: false, updatedAt: Date.now()};
      mine.push(row);
      return touch(view({...row, origin: 'mine'}));
    },
  };
}
