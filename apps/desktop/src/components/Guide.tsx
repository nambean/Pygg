import { useState } from 'react';
import { BookOpen } from 'lucide-react';
import { t } from '../lib/i18n';
import { CodeBlock } from './ui';

type Go = (to: 'connections' | 'endpoint' | 'tools' | 'combos' | 'usage') => void;

type Section = {id: string; title: string; body: (go: Go) => React.ReactNode};

const code = (text: string, label: string) => <CodeBlock code={text} label={label}/>;

/**
 * Guide: how to use Pygg, in the order people set it up, with every command ready to copy (as 9router's and
 * OmniRoute's docs pages do). Static: nothing here sends a request or changes a setting.
 */
const SECTIONS: Section[] = [
  {id: 'start', title: 'Quick start', body: go => <>
    <ol className="guide-steps">
      <li><b>{t('Connect a provider.')}</b> {t('Providers → Connect Provider: choose one (Gemini, OpenRouter and Groq have free tiers; Ollama runs on this computer), paste its API key. Pygg reads its model list; nothing is billed.')}{' '}
        <button type="button" className="link-button" onClick={() => go('connections')}>{t('Open Providers')}</button></li>
      <li><b>{t('Set up your coding tool.')}</b> {t('CLI Tools: Codex and OpenCode in one click, Claude Code with a launcher, editors and SDKs with a snippet. Each tool gets a Pygg API key of its own.')}{' '}
        <button type="button" className="link-button" onClick={() => go('tools')}>{t('Open CLI Tools')}</button></li>
      <li><b>{t('Work as usual, with the model auto.')}</b> {t('Pygg picks an eligible model for each request, within your limits, and records what it cost. CLI Tools says “Working” once a request has come through; Usage shows the cost and an estimate of what you saved.')}</li>
    </ol>
    <p>{t('The gateway listens on this computer only:')}</p>
    {code('OpenAI-compatible:  http://127.0.0.1:8741/v1\nAnthropic:          http://127.0.0.1:8741\nModel:              auto', t('Addresses'))}
  </>},
  {id: 'claude', title: 'Claude Code', body: go => <>
    <p>{t('Pointing Claude Code’s own settings at another endpoint would stop it using your Claude plan, so Pygg uses a launcher: set it up once in CLI Tools (or with the command below), then start Claude Code through Pygg when you want it. Plain claude keeps your Claude sign-in.')}</p>
    {code('pygg setup claude-launcher\npygg run claude\npygg run --model auto/cheap claude\npygg run claude -p "Explain this repository"', t('Claude Code through Pygg'))}
    <p>{t('Arguments after claude are Claude Code’s own; --model before claude sets the model for every slot. Claude Code may say it does not recognize the model “auto” and assume a 200k context: that is expected.')}{' '}
      <button type="button" className="link-button" onClick={() => go('tools')}>{t('Open CLI Tools')}</button></p>
    <p>{t('To let Claude Code hand tasks to Pygg instead (smart tasks, experimental):')}</p>
    {code('pygg setup claude-code', t('MCP server for Claude Code'))}
  </>},
  {id: 'codex', title: 'Codex CLI', body: () => <>
    <p>{t('Pygg adds a pygg provider and a profile to Codex’s configuration, after a backup. Codex’s default model is not changed: choose Pygg per run with the profile.')}</p>
    {code('pygg setup codex-models --dry-run\npygg setup codex-models\ncodex --profile pygg\ncodex --profile pygg -m auto/cheap\ncodex --profile pygg -m anthropic/claude-haiku-5-5', t('Codex through Pygg'))}
    {code('pygg setup codex-models --remove', t('Undo'))}
  </>},
  {id: 'opencode', title: 'OpenCode', body: () => <>
    <p>{t('Pygg adds a provider named Pygg to opencode.json, after a backup, listing auto, auto/cheap and your enabled models; it keeps the list current when models change.')}</p>
    {code('pygg setup opencode-models\nopencode\n# then in OpenCode: /models → Pygg → auto', t('OpenCode through Pygg'))}
    {code('pygg setup opencode-models --remove', t('Undo'))}
  </>},
  {id: 'editors', title: 'Editors and SDKs', body: go => <>
    <p>{t('Cline, Continue, Roo Code and other tools that call the API from this computer: choose an OpenAI-compatible provider and fill in the address, a Pygg API key from Endpoint, and the model. Tools that send requests from their own servers cannot reach 127.0.0.1.')}{' '}
      <button type="button" className="link-button" onClick={() => go('endpoint')}>{t('Open Endpoint')}</button></p>
    {code('Provider:  OpenAI Compatible\nBase URL:  http://127.0.0.1:8741/v1\nAPI key:   a Pygg API key (Endpoint → API Keys)\nModel:     auto', t('Fields'))}
    {code('from openai import OpenAI\n\nclient = OpenAI(base_url="http://127.0.0.1:8741/v1", api_key="<your Pygg API key>")\nreply = client.chat.completions.create(model="auto", messages=[{"role": "user", "content": "Hello"}])\nprint(reply.choices[0].message.content)', 'Python')}
    {code('curl http://127.0.0.1:8741/v1/chat/completions \\\n  -H "Authorization: Bearer $PYGG_KEY" -H "Content-Type: application/json" \\\n  -d \'{"model":"auto","messages":[{"role":"user","content":"Hello"}]}\'', 'curl')}
  </>},
  {id: 'cli', title: 'Command line and npm', body: () => <>
    <p>{t('The installer puts pygg-cli.exe next to the app (usually %LOCALAPPDATA%\\Pygg). From npm, the same program is called pygg:')}</p>
    {code('npm install -g @pygg/cli@dev\npygg --version\npygg status\npygg doctor', t('Install and check'))}
    <table className="data-table guide-table">
      <thead><tr><th scope="col">{t('Command')}</th><th scope="col">{t('What it does')}</th></tr></thead>
      <tbody>
        {([
          ['pygg status', t('Is Pygg serving, on which store, and does the gateway answer')],
          ['pygg doctor', t('Status, plus the store’s schema, the agents found and what is set up in them')],
          ['pygg setup', t('List the tools Pygg can set up, and their state')],
          ['pygg setup <tool> [--dry-run | --remove]', t('codex-models, opencode-models, claude-launcher; claude-code, codex, opencode for MCP')],
          ['pygg run [--model M] claude [args…]', t('Claude Code through Pygg for one launch')],
          ['pygg gateway list', t('Connections, models, combos and API keys in the store')],
          ['pygg gateway usage', t('What the ledger recorded')],
          ['pygg gateway test-model --model C/M', t('One short request to a model (a paid model costs a little)')],
          ['pygg gateway lookup', t('Fill model facts (prices, context, tools) from the models.dev catalog')],
          ['pygg gateway auto --alias auto', t('Which model auto would choose, and why')],
          ['pygg gateway serve --port N', t('The gateway without the window, on another store (--db PATH)')],
          ['pygg agent detect', t('The agent CLIs installed and how they sign in')],
          ['pygg diagnostics --out FILE.zip', t('A redacted bundle for a bug report: no keys, tokens or prompts')],
          ['pygg mcp', t('The MCP server that IDE agents start')],
        ] as const).map(([cmd, what]) => <tr key={cmd}><td className="mono selectable nowrap">{cmd}</td><td>{what}</td></tr>)}
      </tbody>
    </table>
    <p className="secondary">{t('pygg <command> --help lists every option. Keys are never taken on the command line: add-connection reads one from an environment variable (--key-env).')}</p>
  </>},
  {id: 'models', title: 'auto, combos and model names', body: go => <>
    <ul className="guide-list">
      <li><span className="mono">auto</span>: {t('an eligible model at or above the key’s minimum tier (T2 unless changed), local, free and subscription before paid, then the lowest estimated cost. It chooses within your rules; it does not judge how hard a prompt is.')}</li>
      <li><span className="mono">auto/cheap</span>: {t('the lowest estimated cost among the models that can take the request.')}</li>
      <li><span className="mono">coding</span>: {t('a combo’s name: your own ordered list; the next model answers when one is out of quota or unavailable.')}{' '}
        <button type="button" className="link-button" onClick={() => go('combos')}>{t('Open Combos')}</button></li>
      <li><span className="mono">gemini/gemini-2.5-flash</span>: {t('one model of one connection, by name.')}</li>
    </ul>
  </>},
  {id: 'costs', title: 'Costs and limits', body: go => <>
    <ul className="guide-list">
      <li>{t('Each API key can have a monthly limit, a strict limit, and a minimum tier for auto (Endpoint → API Keys → Limits).')}</li>
      <li>{t('A cost Pygg cannot know is shown as unknown, never as $0, and is left out of totals. Give a model a price in Providers to estimate it.')}</li>
      <li>{t('Usage → Estimated Savings prices the same tokens at a model you choose, and says what it left out. It is an estimate, at list prices.')}{' '}
        <button type="button" className="link-button" onClick={() => go('usage')}>{t('Open Usage')}</button></li>
      <li>{t('Subscriptions count against their plan, not as money.')}</li>
    </ul>
  </>},
  {id: 'trouble', title: 'Troubleshooting', body: () => <>
    <table className="data-table guide-table">
      <thead><tr><th scope="col">{t('You see')}</th><th scope="col">{t('What to do')}</th></tr></thead>
      <tbody>
        {([
          [t('Connection refused, or “nothing is listening”'), t('Start the Pygg app (it serves the gateway), or check with pygg status. The port is set in Endpoint → Configure.')],
          ['HTTP 401', t('The API key is wrong or revoked. Endpoint → API Keys → Show copies it again (keys made before this version cannot be shown: create a new one).')],
          [t('HTTP 422 “No connected model is eligible for auto”'), t('No model has every fact auto needs (tier, price, tools, context). Run pygg gateway lookup, set them in Providers → Auto…, lower the key’s minimum tier, or name a model.')],
          [t('HTTP 429 or “cooling down”'), t('A provider’s rate limit or quota. auto and combos move on to the next model; Providers shows when it is tried again.')],
          [t('A cost shows as unknown'), t('The model has no price. Set one in Providers, or run pygg gateway lookup.')],
          [t('Gemini CLI: “client no longer supported”'), t('Google no longer accepts its free Google-account login from Gemini CLI. Use a Gemini API key, or connect Gemini in Providers and use it through Pygg.')],
        ] as const).map(([see, todo]) => <tr key={see}><td>{see}</td><td>{todo}</td></tr>)}
      </tbody>
    </table>
    {code('pygg doctor\npygg diagnostics --out pygg-diagnostics.zip', t('Before asking for help'))}
  </>},
];

export function Guide({go}: {go: Go}) {
  const [open, setOpen] = useState(SECTIONS[0].id);
  const section = SECTIONS.find(s => s.id === open) ?? SECTIONS[0];
  return <div className="page guide">
    <nav className="guide-nav" aria-label={t('Guide sections')}>
      {SECTIONS.map(s => <button key={s.id} type="button" className={s.id === open ? 'guide-nav-item selected' : 'guide-nav-item'}
        aria-current={s.id === open ? 'true' : undefined} onClick={() => setOpen(s.id)}>{t(s.title)}</button>)}
    </nav>
    <article className="guide-body" aria-labelledby="guide-title">
      <h2 id="guide-title"><BookOpen size={16} aria-hidden/>{t(section.title)}</h2>
      {section.body(go)}
    </article>
  </div>;
}
