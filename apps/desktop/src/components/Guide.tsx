import { useState } from 'react';
import { BookOpen } from 'lucide-react';
import { t } from '../lib/i18n';
import { CodeBlock } from './ui';

type Go = (to: 'connections' | 'endpoint' | 'tools' | 'combos' | 'usage') => void;

/** The gateway's addresses, from the running gateway; the defaults until it answers. */
type Addresses = {base: string};
type Section = {id: string; title: string; body: (go: Go, a: Addresses) => React.ReactNode};

const code = (text: string, label: string) => <CodeBlock code={text} label={label}/>;

/**
 * Guide: how to use Pygg, in the order people set it up, with every command ready to copy (as 9router's and
 * OmniRoute's docs pages do). Static: nothing here sends a request or changes a setting.
 */
const SECTIONS: Section[] = [
  {id: 'start', title: 'Quick start', body: (go, a) => <>
    <ol className="guide-steps">
      <li><b>{t('Connect a provider.')}</b> {t('Providers → Connect Provider. A cloud provider takes its API key (Gemini, OpenRouter and Groq have free tiers); a local runtime such as Ollama takes its local address and no key. Pygg reads the model list; nothing is billed.')}{' '}
        <button type="button" className="link-button" onClick={() => go('connections')}>{t('Open Providers')}</button></li>
      <li><b>{t('Set up your coding tool.')}</b> {t('If CLI Tools says auto has no eligible model, open the connection’s Models and review its routing facts (Auto…).')} {t('CLI Tools: Codex and OpenCode in one click, Claude Code with a launcher, editors and SDKs with a snippet. CLI Tools makes a separate Pygg API key for each tool it sets up; for a tool set up by hand, make one in Endpoint → API Keys.')}{' '}
        <button type="button" className="link-button" onClick={() => go('tools')}>{t('Open CLI Tools')}</button></li>
      <li><b>{t('Work as usual, with the model auto.')}</b> {t('Pygg picks an eligible model for each request, within your limits, and records what it cost. CLI Tools says “Working” after that tool receives an answer; Usage shows the cost and an estimate of what you saved.')}</li>
    </ol>
    <p>{t('The gateway listens on this computer only. A tool set up by hand needs these three fields and a Pygg API key:')}</p>
    {code(`OpenAI-compatible:  ${a.base}/v1\nAnthropic:          ${a.base}\nModel:              auto\nAPI key:            a Pygg API key (Endpoint → API Keys)`, t('Addresses'))}
  </>},
  {id: 'claude', title: 'Claude Code', body: go => <>
    <p>{t('Pointing Claude Code’s own settings at another endpoint would stop it using your Claude plan, so Pygg uses a launcher: set it up once in CLI Tools (or with the command below), then start Claude Code through Pygg when you want it. Plain claude keeps your Claude sign-in.')}</p>
    {code('pygg setup claude --dry-run\npygg setup claude\npygg run claude\npygg run --model auto/cheap claude\npygg run claude -p "Explain this repository"', t('Claude Code through Pygg'))}
    <p>{t('Arguments after claude are Claude Code’s own; --model before claude sets the model for every slot. Claude Code may say it does not recognize the model “auto” and assume a 200k context: that is expected.')}{' '}
      <button type="button" className="link-button" onClick={() => go('tools')}>{t('Open CLI Tools')}</button></p>
    <p>{t('To let Claude Code hand tasks to Pygg instead (smart tasks, experimental):')}</p>
    {code('pygg mcp setup claude-code', t('MCP server for Claude Code'))}
  </>},
  {id: 'codex', title: 'Codex CLI', body: () => <>
    <p>{t('Pygg adds a pygg provider and a profile to Codex’s configuration, after a backup. Codex’s default model is not changed: choose Pygg per run with the profile.')}</p>
    {code('pygg setup codex --dry-run\npygg setup codex\npygg run codex\ncodex --profile pygg -m auto/cheap\ncodex --profile pygg -m anthropic/claude-haiku-5-5', t('Codex through Pygg'))}
    {code('pygg setup codex --remove', t('Undo'))}
  </>},
  {id: 'opencode', title: 'OpenCode', body: () => <>
    <p>{t('Pygg adds a provider named Pygg to opencode.json, after a backup, listing auto, auto/cheap and your enabled models; it keeps the list current when models change.')}</p>
    {code('pygg setup opencode\npygg run opencode\n# then in OpenCode: /models → Pygg → auto', t('OpenCode through Pygg'))}
    {code('pygg setup opencode --remove', t('Undo'))}
  </>},
  {id: 'editors', title: 'Editors and SDKs', body: go => <>
    <p>{t('Cline, Continue, Roo Code and other tools that call the API from this computer: choose an OpenAI-compatible provider and fill in the address, a Pygg API key from Endpoint, and the model. Tools that send requests from their own servers cannot reach 127.0.0.1.')}{' '}
      <button type="button" className="link-button" onClick={() => go('endpoint')}>{t('Open Endpoint')}</button></p>
    {code('Provider:  OpenAI Compatible\nBase URL:  http://127.0.0.1:8741/v1\nAPI key:   a Pygg API key (Endpoint → API Keys)\nModel:     auto', t('Fields'))}
    {code('from openai import OpenAI\n\nclient = OpenAI(base_url="http://127.0.0.1:8741/v1", api_key="<your Pygg API key>")\nreply = client.chat.completions.create(model="auto", messages=[{"role": "user", "content": "Hello"}])\nprint(reply.choices[0].message.content)', 'Python')}
    {code('$headers = @{ Authorization = "Bearer $env:PYGG_KEY" }\n$body = @{ model = "auto"; max_tokens = 16; messages = @(@{ role = "user"; content = "Reply with OK." }) } | ConvertTo-Json -Depth 5\nInvoke-RestMethod -Uri "http://127.0.0.1:8741/v1/chat/completions" -Method Post -Headers $headers -ContentType "application/json" -Body $body', 'PowerShell')}
    <p className="secondary">{t('These send a real request, recorded in Usage. The gateway serves three formats:')}</p>
    {code('OpenAI Chat Completions:  POST /v1/chat/completions\nOpenAI Responses:         POST /v1/responses\nAnthropic Messages:       POST /v1/messages', t('Formats'))}
    <p>{t('IDE agents that take MCP servers (Claude Code, Codex, Cursor…) can also start Pygg’s MCP server over stdio, to hand tasks to Pygg (experimental). It is a program they start, not an address:')}</p>
    {code('Command:    pygg   (or the full path of Pygg.exe)\nArguments:  mcp', 'MCP')}
  </>},
  {id: 'cli', title: 'Command line and npm', body: () => <>
    <p>{t('The pygg command does from a terminal what CLI Tools does with a click. npm installs the command only: the Pygg app still runs the gateway, and your coding tool is installed as before. Install @pygg/cli; run pygg. @dev is the Windows x64 developer preview, not code-signed, so SmartScreen or an antivirus may warn about it.')}</p>
    {code('npm install -g @pygg/cli@dev\npygg doctor\npygg setup claude\npygg run claude', t('Install, check, set up Claude Code, start it through Pygg'))}
    {code('pygg setup codex\npygg run codex', t('Codex'))}
    {code('npx --yes --package=@pygg/cli@dev pygg doctor', t('Try it without installing globally'))}
    <p className="secondary">{t('npx downloads the package into npm’s cache and runs it; setup still makes the same changes to your tool’s configuration. Without Node.js, use the program the installer put next to the app:')}</p>
    {code('& "$env:LOCALAPPDATA\\Pygg\\pygg-cli.exe" doctor', t('Without npm'))}
    <p><b>{t('If pygg is not found.')}</b> {t('Open a new terminal first. Then check where npm put it, and add that folder to your user PATH if it is missing:')}</p>
    {code('where.exe pygg\nnpm prefix -g', t('Where npm put the command'))}
    <p className="secondary">{t('If PowerShell refuses to run scripts, call pygg.cmd instead of pygg. To use another program, set PYGG_CLI to its full path; pygg --launcher-info shows which program the command runs.')}</p>
    <table className="data-table guide-table">
      <thead><tr><th scope="col">{t('Command')}</th><th scope="col">{t('What it does')}</th></tr></thead>
      <tbody>
        {([
          ['pygg', t('Where you are: the gateway, providers, tools set up, and what to type next')],
          ['pygg setup', t('The three tools, whether each is installed and set up, and what to type next')],
          ['pygg setup claude | codex | opencode', t('Route the tool’s requests through Pygg (--dry-run previews, --remove undoes)')],
          ['pygg run claude | codex | opencode', t('Start the tool through Pygg, a routing session per launch (--model before claude picks a model)')],
          ['pygg status', t('Is the gateway answering, and on which store (exit code 3 if not)')],
          ['pygg doctor', t('Status, plus your tools and the providers that need attention')],
          ['pygg providers', t('Saved connections: on, last check, health, models')],
          ['pygg models [CONNECTION] --search TEXT', t('The models to ask for, with billing and price')],
          ['pygg logs --limit 20 --failed', t('Recent request attempts, newest first')],
          ['pygg usage --days 7', t('Requests, known and unknown cost, token mix, by client and model')],
          ['--json', t('On status, doctor, providers, models, logs and usage: the same as JSON for scripts')],
          ['pygg mcp setup claude-code | codex | opencode', t('The experimental MCP task integration, not the gateway: the tool can hand tasks to Pygg')],
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
    <p>{t('From the command line (CONNECTION and MODEL are placeholders). The auto command only shows the choice; test-model sends one short request and may cost a little.')}</p>
    {code('pygg gateway combo --name coding --member CONNECTION/MODEL --member BACKUP_CONNECTION/BACKUP_MODEL\npygg gateway auto --alias auto --client "Claude Code" --tools yes --prompt 2000 --max-tokens 1000\npygg gateway auto --alias auto/cheap --tools yes\npygg gateway test-model --model CONNECTION/MODEL', t('Combos and auto'))}
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
          ['HTTP 401', t('The API key is wrong or revoked. Endpoint → API Keys → Show reveals it again (keys made before this version cannot be shown: create a new one).')],
          [t('HTTP 422 “No connected model is eligible for auto”'), t('No model has every fact auto needs (tier, price, tools, context). Run pygg gateway lookup, set them in Providers → Auto…, lower the key’s minimum tier, or name a model.')],
          [t('HTTP 429 or “cooling down”'), t('A provider’s rate limit or quota. auto and combos move on to the next model; Providers shows when it is tried again.')],
          [t('A cost shows as unknown'), t('The model has no price. Set one in Providers, or run pygg gateway lookup.')],
          [t('Gemini CLI: “client no longer supported”'), t('Google no longer accepts its free Google-account login from Gemini CLI. Use a Gemini API key, or connect Gemini in Providers and use it through Pygg.')],
        ] as const).map(([see, todo]) => <tr key={see}><td>{see}</td><td>{todo}</td></tr>)}
      </tbody>
    </table>
    {code('pygg doctor\npygg diagnostics --out "$env:USERPROFILE\\Desktop\\pygg-diagnostics.zip"', t('Before asking for help'))}
  </>},
  {id: 'faq', title: 'FAQ', body: () => <dl className="guide-faq">
    {([
      [t('Does Pygg provide models?'), t('No. It routes your requests to the providers you connect, with your own keys or local runtimes.')],
      [t('Is free unlimited?'), t('No. Free tiers have their own limits, set by each provider; Pygg moves on to the next model when one is used up.')],
      [t('Why is a cost unknown?'), t('The model has no price, the provider reported no usage, or a failed request may have been billed. Unknown is never counted as $0.')],
      [t('Why did it stop after a server error?'), t('The provider may already have run and billed the request. Pygg tries another model only if the API key allows paid retries.')],
      [t('Does fallback retry a poor answer?'), t('No. Fallback happens on errors before the answer starts; Pygg does not judge answers.')],
      [t('Can another computer use my Pygg?'), t('No. The gateway listens on 127.0.0.1 only.')],
      [t('Does using Claude Code through Pygg use my Claude plan?'), t('No: through Pygg, Claude Code uses your connected providers. Plain claude, started without pygg run, keeps your plan.')],
    ] as const).map(([q, a]) => <div key={q}><dt>{q}</dt><dd>{a}</dd></div>)}
  </dl>},
];

export function Guide({go, base}: {go: Go; /** The gateway's root URL, from its status; the default until it answers. */ base?: string | null}) {
  const addresses: Addresses = {base: base ?? 'http://127.0.0.1:8741'};
  const [open, setOpen] = useState(SECTIONS[0].id);
  const section = SECTIONS.find(s => s.id === open) ?? SECTIONS[0];
  return <div className="page guide">
    <nav className="guide-nav" aria-label={t('Guide sections')}>
      {SECTIONS.map(s => <button key={s.id} type="button" className={s.id === open ? 'guide-nav-item selected' : 'guide-nav-item'}
        aria-current={s.id === open ? 'true' : undefined} onClick={() => setOpen(s.id)}>{t(s.title)}</button>)}
    </nav>
    <article className="guide-body" aria-labelledby="guide-title">
      <h2 id="guide-title"><BookOpen size={16} aria-hidden/>{t(section.title)}</h2>
      {section.body(go, addresses)}
    </article>
  </div>;
}
