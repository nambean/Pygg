// Layout check for the interface (UI review 2026-11-14, after the Models table grew wider than the page on real data).
// Opens the browser preview with `?stress` data in headless Edge or Chrome at the app's minimum size (1024 × 640), its
// default (1440 × 900) and a wide window (1990 × 1030), on every screen and in the states that broke or could break:
// models tested on a connection of 40 long-named models, a row's menu, every sheet (Connect, Create API Key, Limits,
// Edit Price, the combo editor), combos tested, a failed request expanded, keys shown. It fails when:
//   - a table or block whose content is wider than its container, unless that container scrolls sideways on purpose;
//   - anything that reaches past the right edge of the window;
//   - a text cell squeezed to one word per line (a cell under 48 px wide holding more than one line of text).
// Usage: start the preview (pnpm dev, port 1431), then `node scripts/ui-overflow-check.mjs [--url http://localhost:1431]`.
// Exit code 1 lists every finding; 0 when the layout holds; 2 when the check itself is broken. Needs Node 22+
// (fetch and WebSocket built in, unflagged).
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const args = process.argv.slice(2);
const base = args.includes('--url') ? args[args.indexOf('--url') + 1] : 'http://localhost:1431';
const screens = ['overview', 'connections', 'endpoint', 'tools', 'combos', 'usage', 'policies', 'agents', 'guide', 'settings'];
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
  {screen: 'endpoint', name: 'keys shown', steps: `for (let i = 0; i < 4; i++) click('Show the key', i); await wait(800);`, expect: '.token-full'},
  {screen: 'usage', name: 'a failed request expanded', steps: `click('Requests'); await wait(800);
    const row = [...document.querySelectorAll('tr')].find(r => /429|failed|error/i.test(r.textContent) && r.querySelector('.row-toggle'));
    row?.querySelector('.row-toggle')?.click(); await wait(500);`, expect: '.ledger-detail'},
];
const browsers = [
  'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
  'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
  'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
  '/usr/bin/google-chrome', '/usr/bin/chromium', '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
];
const binary = process.env.BROWSER ?? browsers.find(existsSync);
if (!binary) { console.error('No Edge or Chrome found; set BROWSER to one.'); process.exit(2); }

const profile = mkdtempSync(join(tmpdir(), 'pygg-ui-check-'));
const port = 9300 + Math.floor(Math.random() * 500);
const browser = spawn(binary, ['--headless=new', '--disable-gpu', '--hide-scrollbars', `--remote-debugging-port=${port}`,
  `--user-data-dir=${profile}`, '--force-device-scale-factor=1', 'about:blank'], {stdio: 'ignore'});

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
    };`;
  const runs = [...screens.map(screen => ({screen, name: 'as it opens', steps: ''})), ...states];
  for (const [width, height] of sizes) {
    await send('Emulation.setDeviceMetricsOverride', {width, height, deviceScaleFactor: 1, mobile: false});
    for (const run of runs) {
      await send('Page.navigate', {url: `${base}/?screen=${run.screen}&stress`});
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
console.log(failures ? `\n${failures} layout problems.` : '\nThe layout holds on every screen.');
process.exit(failures ? 1 : 0);
