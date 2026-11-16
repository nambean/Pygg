// Layout check for the interface (UI review 2026-11-14, after the Models table grew wider than the page on real data).
// Opens the browser preview with `?stress` data in headless Edge or Chrome at the app's minimum size (1024 × 640), its
// default (1440 × 900) and a wide window (1990 × 1030), on every screen and in the states that broke or could break:
// models tested on a connection of 40 long-named models, a row's menu, every sheet (Connect, Create API Key, Limits,
// Edit Price, the combo editor, a skill, a project's skills, Add Project), combos tested, a failed request expanded, keys shown, every agent's context
// files listed; every screen and the Connect and Create API Key sheets on a first run (`?empty`); and sheets and installs
// whose change failed with a long error (`?fail`). It fails when:
//   - a table or block whose content is wider than its container, unless that container scrolls sideways on purpose;
//   - anything that reaches past the right edge of the window;
//   - a text cell squeezed to one word per line (a cell under 48 px wide holding more than one line of text).
//   - a control a screen reader would announce by its role alone (no accessible name), checked once per state.
// Usage: start the preview (pnpm dev, port 1431), then `node scripts/ui-overflow-check.mjs [--url http://localhost:1431]`.
// Exit code 1 lists every finding; 0 when the layout holds; 2 when the check itself is broken. Needs Node 22+
// (fetch and WebSocket built in, unflagged).
import { execFileSync, spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const args = process.argv.slice(2);
const base = args.includes('--url') ? args[args.indexOf('--url') + 1] : 'http://localhost:1431';
const screens = ['overview', 'connections', 'endpoint', 'tools', 'combos', 'usage', 'policies', 'skills', 'agents', 'guide', 'settings'];
// The app's minimum window is 1024 × 640. Windows' Text size setting (Accessibility) zooms web content, so at 125 % and
// 150 % that window lays out like 819 × 512 and 683 × 427 CSS pixels; those two are checked too.
const sizes = args.includes('--sizes')
  ? args[args.indexOf('--sizes') + 1].split(',').map(s => s.split('x').map(Number))
  : [[1024, 640], [1440, 900], [1990, 1030], [819, 512], [683, 427]];

// States reached by clicking, each from a fresh load of its screen. `steps` runs in the page: `click(text, n)` presses the
// n-th button whose text or label starts with `text` (negative n counts from the end); `wait(ms)`.
const states = [
  {screen: 'connections', name: 'OpenRouter, eight models tested', steps: `
    click('OpenRouter'); await wait(800);
    for (let i = 0; i < 8; i++) { click('Test', i); } await wait(1500);`, expect: '.model-line[role=status]'},
  {screen: 'connections', name: 'a model row menu', steps: `click('OpenRouter'); await wait(800); click('More actions', -1); await wait(300);`, expect: '[role=menu]'},
  {screen: 'connections', name: 'Edit Price sheet', steps: `click('OpenRouter'); await wait(800); click('More actions', 4); await wait(300);
    click('Edit Price'); await wait(500);`, expect: '[role=dialog]'},
  {screen: 'connections', name: 'a web session backed off (DeepSeek)', steps: `click('DeepSeek'); await wait(800);`, expect: '.warn-text'},
  {screen: 'connections', name: 'a key ring and its Add Key sheet', steps: `click('OpenRouter'); await wait(800);
    click('Add Key from Another Account'); await wait(500);`, expect: '[role=dialog]'},
  {screen: 'connections', name: 'Connect sheet', steps: `click('Connect Provider'); await wait(800);`, expect: '[role=dialog]'},
  {screen: 'combos', name: 'combos tested', steps: `for (let i = 0; i < 6; i++) click('Test', i); await wait(1500);`, expect: '.combo-test'},
  {screen: 'combos', name: 'combo editor', steps: `click('Edit…', -1); await wait(600);`, expect: '[role=dialog]'},
  {screen: 'endpoint', name: 'Create API Key sheet', steps: `click('Create API Key'); await wait(600);`, expect: '[role=dialog]'},
  {screen: 'endpoint', name: 'Limits sheet', steps: `click('Limits', -1); await wait(600);`, expect: '[role=dialog]'},
  {screen: 'skills', name: 'every agent’s files listed', steps: `document.querySelectorAll('.context-details').forEach(d => d.open = true); await wait(200);`,
    expect: '.context-details[open] .context-item'},
  {screen: 'skills', name: 'a skill row menu', steps: `click('Actions for', 0); await wait(300);`, expect: '[role=menu]'},
  {screen: 'skills', name: 'a starter skill viewed', steps: `click('Actions for', 0); await wait(300); click('View…'); await wait(500);`, expect: '[role=dialog]'},
  {screen: 'skills', name: 'New Skill sheet', steps: `click('New Skill'); await wait(500);`, expect: '#skill-body'},
  {screen: 'skills', name: 'a project’s skills sheet', steps: `click('Install Skills…', 0); await wait(600);`, expect: '.project-skills'},
  {screen: 'skills', name: 'Add Project sheet', steps: `click('Add Project'); await wait(500);`, expect: '#skill-project-path'},
  {screen: 'endpoint', name: 'keys shown', steps: `for (let i = 0; i < 4; i++) click('Show the key', i); await wait(800);`, expect: '.token-full'},
  // First run: nothing connected, no keys, no requests.
  ...['overview', 'connections', 'endpoint', 'tools', 'combos', 'usage', 'skills'].map(screen => ({screen, query: 'empty', name: 'first run', steps: ''})),
  {screen: 'connections', query: 'empty', name: 'first run, Connect sheet', steps: `click('Connect Provider'); await wait(800);`, expect: '[role=dialog]'},
  {screen: 'endpoint', query: 'empty', name: 'first run, Create API Key sheet', steps: `click('Create API Key'); await wait(600);`, expect: '[role=dialog]'},
  // A change that failed: each sheet shows the error under its form.
  {screen: 'endpoint', query: 'stress&fail', name: 'Gateway Settings failed', steps: `click('Configure…'); await wait(500);
    fill('#gateway-port', '8790'); await wait(100); click('Save', -1); await wait(700);`, expect: '[role=dialog] .error-note'},
  {screen: 'endpoint', query: 'stress&fail', name: 'Create API Key failed', steps: `click('Create API Key'); await wait(600);
    fill('#client-name', 'Visual Studio Code with the Continue extension'); await wait(100); click('Create API Key', -1); await wait(700);`,
    expect: '[role=dialog] .error-note'},
  {screen: 'endpoint', query: 'stress&fail', name: 'Limits failed', steps: `click('Limits', -1); await wait(600);
    click('Save Limits', -1); await wait(700);`, expect: '[role=dialog] .error-note'},
  {screen: 'connections', query: 'stress&fail', name: 'Add Key failed', steps: `click('OpenRouter'); await wait(800);
    click('Add Key from Another Account'); await wait(500); fill('#ring-key-label', 'Second account'); fill('#ring-key', 'sk-or-v1-preview');
    document.querySelector('[role=dialog] input[type=checkbox]')?.click(); await wait(100); click('Add Key', -1); await wait(700);`,
    expect: '[role=dialog] .error-note'},
  {screen: 'skills', query: 'stress&fail', name: 'Save Skill failed', steps: `click('New Skill'); await wait(500);
    fill('#skill-name', 'database-migration-reviewer-for-the-billing-service'); fill('#skill-description', 'Reviews migrations');
    fill('#skill-body', 'Review it.'); await wait(100); click('Save', -1); await wait(700);`, expect: '[role=dialog] .error-note'},
  {screen: 'skills', query: 'stress&fail', name: 'Install failed', steps: `click('Install for All', 0); await wait(700);`, expect: '.page > .group .error-note'},
  {screen: 'skills', query: 'stress&fail', name: 'Add Project failed', steps: `click('Add Project'); await wait(500);
    fill('#skill-project-path', 'D:/work/payments-platform-monorepo-with-a-very-long-folder-name/services/settlement'); await wait(100);
    click('Add Project', -1); await wait(700);`, expect: '[role=dialog] .error-note'},
  {screen: 'skills', query: 'stress&fail', name: 'Install in a project failed', steps: `click('Install Skills…', 0); await wait(600);
    click('Install ', -1); await wait(700);`, expect: '[role=dialog] .error-note'},
  {screen: 'combos', query: 'stress&fail', name: 'Save Combo failed', steps: `click('Edit…', -1); await wait(600);
    click('Save Combo', -1); await wait(700);`, expect: '[role=dialog] .error-note'},
  {screen: 'usage', name: 'a failed request expanded', steps: `click('Requests'); await wait(800);
    const row = [...document.querySelectorAll('tr')].find(r => /429|failed|error/i.test(r.textContent) && r.querySelector('.row-toggle'));
    row?.querySelector('.row-toggle')?.click(); await wait(500);`, expect: '.ledger-detail'},
  {screen: 'usage', query: 'stress', name: 'measured-only panels open', steps: `document.querySelectorAll('details.observation')
    .forEach(d => { d.open = true; }); await wait(400);`, expect: 'details.observation[open] .totals-parts'},
  {screen: 'usage', query: 'stress', name: 'why the cache was missed', steps: `document.querySelectorAll('details.token-mix-details')
    .forEach(d => { d.open = true; }); await wait(400);`, expect: '.token-mix-details table'},
];
const browsers = [
  'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
  'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
  'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
  '/usr/bin/google-chrome', '/usr/bin/chromium', '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
];
const binary = process.env.BROWSER ?? browsers.find(existsSync);
if (!binary) { console.error('No Edge or Chrome found; set BROWSER to one.'); process.exit(2); }

// A run stopped from outside (a timeout, a closed terminal) never reaches its `finally`, and its headless browser keeps
// running; on Windows nothing can catch that. So each run first stops the browsers earlier runs left behind (a
// `pygg-ui-check-` profile whose owning process is gone) and deletes their profiles (found 315 processes on 2026-11-16).
function reapOrphans() {
  let rows = [];
  try {
    if (process.platform === 'win32') {
      const ps = `Get-CimInstance Win32_Process | ? { $_.CommandLine -match 'pygg-ui-check-' -and $_.CommandLine -match '--headless' -and $_.CommandLine -notmatch '--type=' } | % { "$($_.ProcessId) $($_.ParentProcessId) $($_.CommandLine)" }`;
      rows = execFileSync('powershell', ['-NoProfile', '-Command', ps], {encoding: 'utf8'}).split(/\r?\n/);
    } else {
      rows = execFileSync('ps', ['-eo', 'pid=,ppid=,args='], {encoding: 'utf8'}).split('\n')
        .filter(l => l.includes('pygg-ui-check-') && l.includes('--headless') && !l.includes('--type='));
    }
  } catch { return; }
  const alive = pid => { try { process.kill(pid, 0); return true; } catch { return false; } };
  const inUse = new Set();
  for (const row of rows) {
    const [pid, ppid] = row.trim().split(/\s+/).map(Number);
    if (!pid) continue;
    const name = row.match(/pygg-ui-check-[A-Za-z0-9]+/)?.[0];
    if (ppid > 1 && alive(ppid)) { if (name) inUse.add(name); continue; } // another run's browser, still in use
    try { process.kill(pid); } catch { /* already gone */ }
  }
  // Profiles of stopped browsers; not one a live run uses or is about to start a browser in.
  for (const name of readdirSync(tmpdir()).filter(n => n.startsWith('pygg-ui-check-') && !inUse.has(n))) {
    const dir = join(tmpdir(), name);
    try { if (Date.now() - statSync(dir).mtimeMs > 120_000) rmSync(dir, {recursive: true, force: true}); } catch { /* held */ }
  }
}
reapOrphans();

const profile = mkdtempSync(join(tmpdir(), 'pygg-ui-check-'));
const port = 9300 + Math.floor(Math.random() * 500);
const browser = spawn(binary, ['--headless=new', '--disable-gpu', '--hide-scrollbars', `--remote-debugging-port=${port}`,
  `--user-data-dir=${profile}`, '--force-device-scale-factor=1', 'about:blank'], {stdio: 'ignore'});
for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
  process.on(signal, () => { browser.kill(); process.exit(130); });
}

const sleep = ms => new Promise(r => setTimeout(r, ms));
async function json(path, method = 'GET') {
  for (let i = 0; i < 50; i++) {
    try { return await (await fetch(`http://127.0.0.1:${port}${path}`, {method})).json(); } catch { await sleep(200); }
  }
  throw new Error('the browser did not start');
}

/** The check, run in the page: what is wider than the box that holds it. */
function inspect() {
  const findings = [];
  const flagged = [];
  // Only the outermost element of a nest is reported: a table past the edge, not each of its rows and cells.
  const report = (el, text) => { if (!flagged.some(f => f.contains(el))) { flagged.push(el); findings.push(text); } };
  const name = el => {
    const text = (el.getAttribute('aria-label') || el.textContent || '').trim().replace(/\s+/g, ' ').slice(0, 60);
    return `${el.tagName.toLowerCase()}${el.className && typeof el.className === 'string' ? '.' + el.className.trim().split(/\s+/).join('.') : ''}${text ? ` "${text}"` : ''}`;
  };
  const scrollsSideways = el => ['auto', 'scroll'].includes(getComputedStyle(el).overflowX);
  const viewport = document.documentElement.clientWidth;
  for (const el of document.querySelectorAll('.content *, [role="dialog"] *, [role="menu"] *')) {
    const style = getComputedStyle(el);
    if (style.display === 'none' || style.visibility === 'hidden' || style.position === 'fixed') continue;
    const box = el.getBoundingClientRect();
    // Text in a box too narrow to show any of it, as the Usage tables' model names were once (a cell made a block of
    // width 0): nothing overflows, the text is simply gone. A screen-reader-only label (clipped on purpose) is fine.
    const ownText = [...el.childNodes].some(n => n.nodeType === 3 && n.textContent.trim());
    const clippedOnPurpose = style.position === 'absolute' && (style.clip !== 'auto' || box.width <= 1);
    if (ownText && !clippedOnPurpose && box.height > 0 && box.width < 4) {
      report(el, `text hidden in a box ${Math.round(box.width)} px wide: ${name(el)}`);
    }
    if (box.width === 0 || box.height === 0) continue;
    // Inside a box that scrolls sideways on purpose, content may reach past the window: it is scrolled to, not lost.
    let scroller = el.parentElement;
    while (scroller && scroller !== document.body && !scrollsSideways(scroller)) scroller = scroller.parentElement;
    const inScroller = scroller && scroller !== document.body;
    if (box.right > viewport + 1 && !inScroller) report(el, `past the window's right edge by ${Math.round(box.right - viewport)} px: ${name(el)}`);
    // Cut off by a box that clips (overflow hidden), as the Models table was by its rounded group box: what reaches past
    // the clipping box is not seen. A box that scrolls sideways on purpose is fine.
    let clip = el.parentElement;
    while (clip && clip !== document.body && getComputedStyle(clip).overflowX === 'visible') clip = clip.parentElement;
    if (clip && clip !== document.body && !scrollsSideways(clip)) {
      const edge = clip.getBoundingClientRect().right;
      if (box.right > edge + 1) report(el, `cut off ${Math.round(box.right - edge)} px by ${name(clip).split(' "')[0]}: ${name(el)}`);
    }
    // Content wider than its own box, in a box that does not scroll sideways and does not cut it off with an ellipsis.
    if (el.scrollWidth > el.clientWidth + 1 && !scrollsSideways(el) && style.textOverflow !== 'ellipsis'
      && style.overflowX !== 'hidden' && el.clientWidth > 0 && !['svg', 'path', 'input', 'textarea', 'select'].includes(el.tagName.toLowerCase())) {
      report(el, `content ${el.scrollWidth - el.clientWidth} px wider than its box: ${name(el)}`);
    }
    if (el.tagName === 'TD' && box.width < 48 && box.height > 40 && (el.textContent ?? '').trim().includes(' ')) {
      report(el, `squeezed to ${Math.round(box.width)} px: ${name(el)}`);
    }
  }
  // Report each element once, outermost first.
  // The column a person reads to choose a model must stay readable (Codex review: the leftover-width column).
  for (const cell of document.querySelectorAll('.models-table .model-cell')) {
    const width = cell.getBoundingClientRect().width;
    if (width > 0 && width < 260) { findings.push(`the model column is only ${Math.round(width)} px wide`); break; }
  }
  return findings.slice(0, 25);
}

let failures = 0;

// Controls a screen reader announces by role alone ("button", "edit"): no text, label, aria-label or title. Read from
// the browser's accessibility tree, the same tree Narrator reads through UI Automation.
const NAMED_ROLES = new Set(['button', 'link', 'textbox', 'searchbox', 'combobox', 'listbox', 'checkbox', 'radio', 'switch',
  'menuitem', 'menuitemcheckbox', 'menuitemradio', 'tab', 'slider', 'spinbutton', 'option', 'dialog', 'img']);
async function unnamedControls(send) {
  const tree = await send('Accessibility.getFullAXTree');
  const out = [];
  for (const node of tree.result?.nodes ?? []) {
    const role = node.role?.value;
    if (node.ignored || !NAMED_ROLES.has(role) || String(node.name?.value ?? '').trim()) continue;
    const described = node.backendDOMNodeId
      ? await send('DOM.resolveNode', {backendNodeId: node.backendDOMNodeId}).then(r => r.result?.object?.objectId
        ? send('Runtime.callFunctionOn', {objectId: r.result.object.objectId, returnByValue: true,
            functionDeclaration: 'function () { return this.outerHTML.slice(0, 160); }'}) : null)
      : null;
    out.push(`a ${role} with no accessible name: ${described?.result?.result?.value ?? '(no element)'}`);
  }
  return out;
}
try {
  // A page target: Edge also lists its extensions' background pages.
  let target;
  for (let i = 0; i < 25 && !target; i++) {
    target = (await json('/json/list')).find(t => t.type === 'page');
    if (!target) await sleep(200);
  }
  if (!target) throw new Error('the browser opened no page');
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
  let id = 0;
  const pending = new Map();
  ws.onmessage = event => {
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) { pending.get(message.id)(message); pending.delete(message.id); }
  };
  const send = (method, params = {}) => new Promise(resolve => { const n = ++id; pending.set(n, resolve); ws.send(JSON.stringify({id: n, method, params})); });
  await send('DOM.enable');
  await send('Accessibility.enable');
  // Waits until the screen has drawn: its content is there and nothing in it is still loading (up to 10 s).
  const settle = async () => {
    for (let i = 0; i < 40; i++) {
      await sleep(250);
      const r = await send('Runtime.evaluate', {returnByValue: true,
        expression: `!!document.querySelector('.content > *') && !document.querySelector('.content .spinner-label')`});
      if (r.result?.result?.value) break;
    }
    await sleep(500);
  };
  // First, the check must see the bug it was written for: the old test result, unbroken, in the Models table's actions.
  await send('Emulation.setDeviceMetricsOverride', {width: 1990, height: 1030, deviceScaleFactor: 1, mobile: false});
  await send('Page.navigate', {url: `${base}/?screen=connections&stress`});
  await settle();
  const planted = await send('Runtime.evaluate', {returnByValue: true, expression: `(() => {
    const cell = document.querySelector('table td:last-child');
    if (!cell) return ['no table to plant the bug in: ' + location.href];
    const s = document.createElement('small'); s.style.whiteSpace = 'nowrap';
    s.textContent = 'The model answered with HTTP 429 Too Many Requests: Rate limit exceeded: free-models-per-day. Add 10 credits to unlock 1000 free model requests per day.'.repeat(2);
    cell.appendChild(s);
    return (${inspect.toString()})();
  })()`});
  if (!(planted.result?.result?.value ?? []).some(f => f.includes('cut off') || f.includes('past the window') || f.includes('wider than its box'))) {
    console.error(JSON.stringify(planted.result?.result?.value ?? planted.result));
    console.error('Self-test failed: a planted overflow in the Models table was not found. The check is blind; fix it first.');
    browser.kill();
    process.exit(2);
  }
  console.log('self-test: a planted overflow is found');
  const helpers = `const wait = ms => new Promise(r => setTimeout(r, ms));
    window.confirm = () => true;
    const click = (text, n = 0) => {
      const all = [...document.querySelectorAll('button, [role=menuitem], [role=tab], [role=radio]')].filter(b =>
        (b.getAttribute('aria-label') ?? '').startsWith(text) || (b.textContent ?? '').trim().startsWith(text));
      const b = all.at(n); if (b) b.click(); return !!b;
    };
    // Types into a React input or text area: the native setter, then an input event React listens for.
    const fill = (selector, value) => {
      const el = document.querySelector(selector); if (!el) return false;
      const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
      Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, value);
      el.dispatchEvent(new Event('input', {bubbles: true})); return true;
    };`;
  const runs = [...screens.map(screen => ({screen, name: 'as it opens', steps: ''})), ...states];
  for (const [width, height] of sizes) {
    await send('Emulation.setDeviceMetricsOverride', {width, height, deviceScaleFactor: 1, mobile: false});
    for (const run of runs) {
      await send('Page.navigate', {url: `${base}/?screen=${run.screen}&${run.query ?? 'stress'}`});
      await settle();
      if (run.steps) {
        const done = await send('Runtime.evaluate', {awaitPromise: true, returnByValue: true,
          expression: `(async () => { ${helpers} ${run.steps} return ${run.expect ? `!!document.querySelector(${JSON.stringify(run.expect)})` : 'true'}; })()`});
        if (done.result?.exceptionDetails) { failures++; console.log(`
${run.screen}, ${run.name}: the steps failed: ${done.result.exceptionDetails.exception?.description ?? ''}`); continue; }
        // A state the steps did not reach is a failure of the check, not a pass.
        if (done.result?.result?.value !== true) { failures++; console.log(`
${run.screen}, ${run.name}, ${width} × ${height}: not reached (no ${run.expect})`); continue; }
      }
      const result = await send('Runtime.evaluate', {expression: `(${inspect.toString()})()`, returnByValue: true});
      const found = result.result?.result?.value ?? [`could not inspect: ${JSON.stringify(result.result?.exceptionDetails ?? result.error)}`];
      // Once per state (at the first size): what a screen reader such as Narrator gets from the accessibility tree.
      if (width === sizes[0][0] && height === sizes[0][1]) found.push(...await unnamedControls(send));
      const label = `${run.screen}, ${run.name}, ${width} × ${height}`;
      if (found.length) {
        failures += found.length;
        console.log(`
${label}:`);
        for (const f of found) console.log(`  ${f}`);
      } else {
        console.log(`${label}: ok`);
      }
    }
  }
  ws.close();
} finally {
  browser.kill();
  await sleep(300);
  try { rmSync(profile, {recursive: true, force: true}); } catch { /* the browser may still hold it */ }
}
console.log(failures ? `\n${failures} layout or naming problems.` : '\nThe layout holds on every screen, and every control has a name.');
process.exit(failures ? 1 : 0);
