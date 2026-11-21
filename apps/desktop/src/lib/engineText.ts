import {t} from './i18n';

/**
 * Text the task engine writes in English (a task's reason, an attempt's escalation reason, a check's detail, a step's
 * label), shown in the interface language. The engine fills these templates (src-tauri/src/messages.rs); a Rust test
 * keeps the list below equal to its own, so keep one template per line between the markers.
 */
const TEMPLATES: string[] = [
  // templates: start
  "Step {0} ({1}) is paid, and today's budget for paid steps is used; it resets at 00:00 UTC. Raise the budget in Settings, or start the task again after the reset.",
  "Step {0} ({1}) is marked paid, so it waits for your approval.",
  "Step {0} ({1}) waits for your approval: the workspace is sensitive, so every step asks before an agent sees its code.",
  "You stopped it",
  "You declined the step it was waiting on",
  "{0}. Pygg stopped before trying another step.",
  "Every step failed. Last: {0}",
  "no attempt ran",
  "Stopped at this task's cap of {0}: it has cost {1} so far. No further step was started.",
  "Answered. Nothing was changed; the answer is below.",
  "The agent finished without an answer",
  "Skipped {0}: its account is {1} ({2}). Fix it under Providers to use this model again.",
  "No checks are approved for this workspace, so nothing confirmed the result. Review the changes yourself.",
  "No checks are approved for this workspace, so nothing confirmed the result. Review the changes yourself. {0}.",
  "The checks passed, but this attempt changed {0}, which defines them. Review the changes before trusting the result.",
  "The checks passed, but this attempt changed {0}, which defines them. Review the changes before trusting the result. {1}.",
  "{0}.",
  "You discarded the result",
  "The app closed while this task ran. Retry runs the step again from the same code.",
  "{0} for this step, ${1}, is over your cap of ${2}",
  "Pygg's ledger",
  "{0}'s own estimate",
  "{0} of this step's requests through Pygg had no price (enter the model's price in Connections), so your cap for this step could not be checked",
  "{0} reported no cost, so your cap for this step could not be checked",
  "Skipped: {0} timed out on its last two runs in the past 30 minutes without answering, so Pygg did not wait for it again",
  "You declined this step",
  "{0} could not run: {1}",
  "{0} did not finish: {1}",
  "The agent made no changes",
  "`{0}` {1}",
  "{0}: {1}",
  "exit {0}",
  "exited {0}",
  "ended abnormally",
  "timed out",
  "canceled",
  "could not start: {0}",
  "the agent reported an error",
  "unknown agent {0}",
  "Describe the task",
  "The workspace must be a Git repository: the agent works on a separate copy of it.",
  "{0} is not installed, or not on PATH. See {1}",
  "{0} is not a Git repository",
  "Git was not found on this computer",
  "A branch named {0} already exists",
  "git {0} failed: {1}",
  "default model",
  "{0} · {1}",
  "Choose the workspace folder",
  "{0} is not a folder",
  "The workspace must be a Git repository: every attempt works on its own worktree, so your files and branch are never touched.",
  "That workspace no longer exists",
  "That check no longer exists",
  "Type the command, e.g. pnpm test",
  "A check is one command line of at most 500 characters",
  "Unknown privacy class {0}",
  "This workspace has task history, so it stays.",
  "Unknown strategy {0}",
  "Add at least one step",
  "A task has at most {0} steps",
  "Unknown agent {0}",
  "A step's tier is T0 to T4",
  "Unknown billing {0}",
  "Unknown handoff {0}: use none, tail or evidence",
  "That task no longer exists",
  "Only a running or waiting task can be stopped",
  "This task is not waiting for an approval",
  "The waiting step is not in the task's policy",
  "Set a spending cap for this paid step",
  "This step's estimate or your budget changed since you were asked. Check the new estimate and approve again.",
  "The cap is more than what is left of today's budget for paid steps (${0})",
  "Only an interrupted task can be retried",
  "The task has no base commit",
  "This task has no result waiting for you",
  "The result's worktree is gone",
  // templates: end
];

/** Parts of a filled template that stay as they are: commands, file names, amounts, counts. */
const VERBATIM = new Set([
  "`{0}` {1}#0", "The checks passed, but this attempt changed {0}, which defines them. Review the changes before trusting the result.#0",
  "The checks passed, but this attempt changed {0}, which defines them. Review the changes before trusting the result. {1}.#0",
  "git {0} failed: {1}#0", "git {0} failed: {1}#1", "A branch named {0} already exists#0", "{0} is not a Git repository#0",
  "{0} is not installed, or not on PATH. See {1}#1", "{0} is not a folder#0",
]);

type Matcher = {template: string; pattern: RegExp; literal: number};

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** The most specific templates first: the more fixed text a template has, the less it can match by accident. */
const MATCHERS: Matcher[] = TEMPLATES.map(template => {
  const parts = template.split(/\{\d\}/);
  const pattern = new RegExp('^' + parts.map(escape).join('([\\s\\S]+?)') + '$');
  return {template, pattern, literal: parts.join('').length};
}).sort((a, b) => b.literal - a.literal);

/** Engine text in the interface language; text that matches no template (an agent's own words) stays as it is. */
export function engineText(text: string | null | undefined, depth = 0): string {
  if (!text) return '';
  if (depth > 6) return text;
  for (const {template, pattern} of MATCHERS) {
    const found = pattern.exec(text);
    if (!found) continue;
    const vars: Record<string, string> = {};
    found.slice(1).forEach((part, i) => {
      vars[String(i)] = VERBATIM.has(`${template}#${i}`) ? part : engineText(part, depth + 1);
    });
    return t(template, vars);
  }
  return text;
}
