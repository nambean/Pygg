// Hidden acceptance tests for SPEC.md's pages, in headless Microsoft Edge over the DevTools protocol:
//   node ui.mjs <project dir> <out dir>
// Starts the project with a fresh database, drives it as a user would (fields found by their visible label, buttons
// by their name as SPEC.md gives them), checks every page at 375 px for sideways scrolling, restarts the server to
// check the data stayed, and saves screenshots of every page at 1440 px and 375 px for the reviewers.
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import net from 'node:net';

const dir = resolve(process.argv[2]);
const outDir = resolve(process.argv[3] ?? 'ui-out');
mkdirSync(outDir, { recursive: true });
const EDGE = ['C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe', 'C:/Program Files/Microsoft/Edge/Application/msedge.exe'].find(existsSync);
const results = [];
const check = async (id, fn) => {
  try { const d = await fn(); results.push({ id, pass: true, detail: d ?? '' }); }
  catch (e) { results.push({ id, pass: false, detail: String(e?.message ?? e).slice(0, 300) }); }
};
const expect = (c, m) => { if (!c) throw new Error(m); };
const sleep = ms => new Promise(r => setTimeout(r, ms));
const freePort = () => new Promise(r => { const s = net.createServer().listen(0, () => { const p = s.address().port; s.close(() => r(p)); }); });

const port = await freePort();
const base = `http://127.0.0.1:${port}`;
const data = mkdtempSync(join(tmpdir(), 'stockroom-ui-'));
const password = 'acc-Pa55-word';
const env = { ...process.env, PORT: String(port), DB_PATH: join(data, 'db.sqlite'), ADMIN_PASSWORD: password, NODE_ENV: 'production' };
if (existsSync(join(dir, 'package.json')) && !existsSync(join(dir, 'node_modules'))) {
  spawnSync('npm', ['install', '--no-audit', '--no-fund'], { cwd: dir, shell: true, timeout: 300_000 });
}
let server;
const kill = p => { try { if (process.platform === 'win32') spawnSync('taskkill', ['/pid', String(p.pid), '/t', '/f']); else p.kill(); } catch {} };
const start = async () => {
  server = spawn('npm', ['start'], { cwd: dir, shell: true, env });
  server.stdout.on('data', () => {}); server.stderr.on('data', () => {});
  for (let i = 0; i < 60; i++) { try { await fetch(base + '/login', { redirect: 'manual' }); return true; } catch { await sleep(500); } }
  return false;
};
const up = await start();

const cdpPort = await freePort();
const edge = spawn(EDGE, ['--headless=new', `--remote-debugging-port=${cdpPort}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), 'edge-acc-'))}`,
  '--no-first-run', '--disable-gpu', '--window-size=1440,900', 'about:blank']);
let targets = [];
for (let i = 0; i < 40 && !targets.length; i++) {
  try { targets = (await (await fetch(`http://127.0.0.1:${cdpPort}/json`)).json()).filter(t => t.type === 'page'); } catch { await sleep(250); }
}
const ws = new WebSocket(targets[0].webSocketDebuggerUrl);
await new Promise(r => ws.addEventListener('open', r));
let seq = 0; const pending = new Map();
ws.addEventListener('message', e => {
  const m = JSON.parse(e.data);
  if (m.id && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
  if (m.method === 'Page.javascriptDialogOpening') send('Page.handleJavaScriptDialog', { accept: true });
});
const send = (method, params = {}) => new Promise(r => { const i = ++seq; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
await send('Page.enable');
await send('Runtime.enable');
const evaluate = async expr => {
  const r = await send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.result?.exceptionDetails) throw new Error('page script: ' + (r.result.exceptionDetails.exception?.description ?? '').slice(0, 200));
  return r.result?.result?.value;
};
const settle = async () => { await sleep(700); for (let i = 0; i < 20; i++) { if (await evaluate('document.readyState') === 'complete') break; await sleep(150); } await sleep(300); };
const go = async path => { await send('Page.navigate', { url: base + path }); await settle(); };
const size = async (w, h, mobile) => send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile });
const shot = async name => { const r = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true }); writeFileSync(join(outDir, name + '.png'), Buffer.from(r.result.data, 'base64')); };
const text = () => evaluate('document.body ? document.body.innerText : ""');
const path = () => evaluate('location.pathname');

// In-page helpers: a control by its visible label, a clickable by its name.
const HELPERS = `
  window.__norm = s => (s || '').replace(/[*:]/g, '').replace(/\\s+/g, ' ').trim().toLowerCase();
  window.__visible = el => !!(el && (el.offsetWidth || el.offsetHeight || el.getClientRects().length));
  window.__field = (label, scope) => {
    const root = scope || document;
    const want = __norm(label);
    for (const l of root.querySelectorAll('label')) {
      if (__norm(l.textContent) === want || __norm(l.textContent).startsWith(want + ' ')) {
        const c = l.control || l.querySelector('input,select,textarea');
        if (c && __visible(c)) return c;
      }
    }
    for (const c of root.querySelectorAll('input,select,textarea')) {
      if (!__visible(c)) continue;
      const by = c.getAttribute('aria-labelledby');
      const named = __norm(c.getAttribute('aria-label')) || (by ? __norm(document.getElementById(by)?.textContent) : '');
      if (named === want) return c;
    }
    return null;
  };
  window.__set = (el, v) => {
    const proto = el.tagName === 'SELECT' ? HTMLSelectElement.prototype : el.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, v);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  };
  window.__button = (name, scope) => {
    const want = __norm(name);
    const all = [...(scope || document).querySelectorAll('button,input[type=submit],input[type=button],a,[role=button]')].filter(__visible);
    return all.find(b => __norm(b.textContent || b.value || b.getAttribute('aria-label')) === want)
      || all.find(b => __norm(b.textContent || b.value || b.getAttribute('aria-label')).startsWith(want));
  };
  window.__formOf = el => el.closest('form') || el.parentElement;
  true;`;
const helpers = () => evaluate(HELPERS);
const fill = async (label, value) => {
  await helpers();
  const ok = await evaluate(`(() => { const f = __field(${JSON.stringify(label)}); if (!f) return false; __set(f, ${JSON.stringify(String(value))}); return true; })()`);
  if (!ok) throw new Error(`no visible field labelled "${label}"`);
};
const click = async name => {
  await helpers();
  const ok = await evaluate(`(() => { const b = __button(${JSON.stringify(name)}); if (!b) return false; b.click(); return true; })()`);
  if (!ok) throw new Error(`no visible button or link named "${name}"`);
  await settle();
};
const hasError = async before => {
  const t = (await text()).toLowerCase();
  return t !== before.toLowerCase() && /(exist|duplicate|already|taken|unique|in use|409|conflict)/.test(t);
};
const login = async (pw = password) => { await go('/login'); await fill('Username', 'admin'); await fill('Password', pw); await click('Sign in'); };

await check('server starts with npm start', () => expect(up, 'no answer within 30 s'));

await check('a page without a session shows the sign-in page', async () => { await go('/products'); expect((await path()).startsWith('/login'), 'at ' + await path()); });
await check('a wrong password shows an error on the page', async () => {
  await login('wrong');
  const t = (await text()).toLowerCase();
  expect((await path()).startsWith('/login') && /(invalid|incorrect|wrong|fail|error|not)/.test(t), 'at ' + await path() + ': ' + t.slice(0, 120));
});
await check('signing in goes to the dashboard', async () => {
  await login();
  expect(['/', '/dashboard'].includes(await path()), 'at ' + await path());
  await shot('dashboard-1440');
});
await check('the navigation links every page', async () => {
  await helpers();
  const missing = await evaluate(`['Dashboard','Products','Customers','Orders'].filter(n => !__button(n))`);
  expect(missing.length === 0, 'missing ' + missing.join(', '));
});
await check('the dashboard shows the counts and revenue', async () => {
  const t = (await text()).toLowerCase();
  expect(/product/.test(t) && /customer/.test(t) && /order/.test(t) && /revenue/.test(t), t.slice(0, 160));
});

// ---- products ----
await check('add a product through the form', async () => {
  await go('/products');
  await fill('SKU', 'UI-1'); await fill('Name', 'Green teapot'); await fill('Price', '12.5'); await fill('Stock', '3');
  await click('Add product');
  await go('/products');
  expect((await text()).includes('Green teapot'), 'not listed after adding');
});
await check('a duplicate SKU shows an error in plain words', async () => {
  await go('/products');
  const before = await text();
  await fill('SKU', 'UI-1'); await fill('Name', 'Copy'); await fill('Price', '1'); await fill('Stock', '1');
  await click('Add product');
  expect(await hasError(before), 'no visible error: ' + (await text()).slice(0, 200));
});
await check('delete asks for confirmation and removes the product', async () => {
  await evaluate(`fetch('/api/products', {method:'POST', headers:{'content-type':'application/json'}, body: JSON.stringify({sku:'DEL-1', name:'Doomed vase', price:1, stock:1})})`);
  await go('/products');
  await helpers();
  const clicked = await evaluate(`(() => {
    const row = [...document.querySelectorAll('tr, li, .card, article, div')].filter(e => e.innerText && e.innerText.includes('Doomed vase') && __button('Delete', e)).sort((a, b) => a.innerText.length - b.innerText.length)[0];
    if (!row) return false; __button('Delete', row).click(); return true; })()`);
  expect(clicked, 'no Delete button next to the product');
  await settle(); await sleep(500);
  const left = await evaluate(`fetch('/api/products?q=Doomed').then(r => r.json()).then(j => j.total)`);
  expect(left === 0, 'still there after confirming');
});

await check('products page 20 at a time with next and previous links', async () => {
  for (let i = 0; i < 24; i++) {
    const body = JSON.stringify({ sku: 'BULK-' + String(i).padStart(2, '0'), name: 'Bulk item ' + i, price: 1, stock: 9 });
    await evaluate(`fetch('/api/products', {method:'POST', headers:{'content-type':'application/json'}, body: ${JSON.stringify(body)}}).then(r => r.status)`);
  }
  await go('/products');
  await helpers();
  const rows = await evaluate(`document.querySelectorAll('table tbody tr').length`);
  expect(rows === 20, 'rows on page 1: ' + rows);
  const next = await evaluate(`!!__button('Next') || !!document.querySelector('a[rel=next]')`);
  expect(next, 'no Next link');
  await shot('products-1440');
});
await check('the search box finds by name or SKU', async () => {
  await go('/products');
  await helpers();
  const ok = await evaluate(`(() => {
    const box = __field('Search') || document.querySelector('input[type=search]') || document.querySelector('input[name=q]') || [...document.querySelectorAll('input')].find(i => /search/i.test(i.placeholder || ''));
    if (!box) return false;
    __set(box, 'teapot');
    const form = box.closest('form');
    if (form) { form.requestSubmit ? form.requestSubmit() : form.submit(); }
    else box.dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter', bubbles: true}));
    return true; })()`);
  expect(ok, 'no search box');
  await settle();
  const t = await text();
  expect(t.includes('Green teapot') && !t.includes('Bulk item 1'), 'results: ' + t.slice(0, 200));
});
await check('the CSV link downloads every product', async () => {
  await go('/products');
  await helpers();
  const got = await evaluate(`(async () => { const a = __button('Download CSV'); if (!a) return null; const r = await fetch(a.href || a.getAttribute('formaction') || '/api/products.csv'); return (await r.text()).split(String.fromCharCode(10))[0].trim(); })()`);
  expect(got !== null, 'no Download CSV link');
  expect(got.replace(/"/g, '') === 'id,sku,name,price,stock', 'header ' + got);
});
// ---- customers ----
await check('add a customer through the form', async () => {
  await go('/customers');
  await fill('Name', 'Hoa Tran'); await fill('Email', 'hoa@example.com'); await fill('Phone', '0987654321');
  await click('Add customer');
  await go('/customers');
  expect((await text()).includes('Hoa Tran'), 'not listed');
  await shot('customers-1440');
});
await check('a duplicate email shows an error', async () => {
  await go('/customers');
  const before = await text();
  await fill('Name', 'Other'); await fill('Email', 'hoa@example.com');
  await click('Add customer');
  expect(await hasError(before), 'no visible error');
});

// ---- orders ----
await check('create an order through the form', async () => {
  await go('/orders');
  await helpers();
  const chosen = await evaluate(`(() => {
    const selects = [...document.querySelectorAll('select')].filter(__visible);
    const cust = selects.find(s => [...s.options].some(o => o.text.includes('Hoa Tran')));
    const prod = selects.find(s => [...s.options].some(o => o.text.includes('Green teapot')));
    if (!cust || !prod) return 'selects: ' + selects.length;
    __set(cust, [...cust.options].find(o => o.text.includes('Hoa Tran')).value);
    __set(prod, [...prod.options].find(o => o.text.includes('Green teapot')).value);
    const qty = __field('Quantity') || [...document.querySelectorAll('input[type=number]')].filter(__visible).find(i => /qty|quantity/i.test(i.name + i.id + i.placeholder)) || [...document.querySelectorAll('input[type=number]')].filter(__visible)[0];
    if (qty) __set(qty, '2');
    return 'ok'; })()`);
  expect(chosen === 'ok', 'could not choose a customer and a product: ' + chosen);
  await click('Create order');
  const orders = await evaluate(`fetch('/api/orders').then(r => r.json())`);
  expect(orders.total === 1 && orders.items[0].customerName === 'Hoa Tran', 'orders: ' + JSON.stringify(orders).slice(0, 160));
  await go('/orders');
  expect((await text()).includes('Hoa Tran'), 'not listed');
  await shot('orders-1440');
});
await check('an order moves to paid with a button', async () => {
  await go('/orders');
  await helpers();
  const ok = await evaluate(`(() => {
    const b = [...document.querySelectorAll('button,input[type=submit],a')].filter(__visible).find(x => /^(mark (as )?)?paid$|^pay$|→ ?paid|to paid/i.test(__norm(x.textContent || x.value)));
    if (!b) return false; b.click(); return true; })()`);
  expect(ok, 'no button to move it to paid');
  await settle();
  const o = await evaluate(`fetch('/api/orders').then(r => r.json()).then(j => j.items[0].status)`);
  expect(o === 'paid', 'status ' + o);
});
await check('too little stock shows an error', async () => {
  await go('/orders');
  await helpers();
  const before = await text();
  await evaluate(`(() => {
    const selects = [...document.querySelectorAll('select')].filter(__visible);
    const cust = selects.find(s => [...s.options].some(o => o.text.includes('Hoa Tran')));
    const prod = selects.find(s => [...s.options].some(o => o.text.includes('Green teapot')));
    __set(cust, [...cust.options].find(o => o.text.includes('Hoa Tran')).value);
    __set(prod, [...prod.options].find(o => o.text.includes('Green teapot')).value);
    const qty = __field('Quantity') || [...document.querySelectorAll('input[type=number]')].filter(__visible)[0];
    __set(qty, '99'); })()`);
  await click('Create order');
  const t = (await text()).toLowerCase();
  expect(t !== before.toLowerCase() && /(stock|enough|insufficient|available)/.test(t), 'no visible error');
});

// ---- accessibility and layout ----
await check('every form field has a label', async () => {
  const bad = [];
  for (const p of ['/', '/products', '/customers', '/orders']) {
    await go(p);
    const b = await evaluate(`[...document.querySelectorAll('input,select,textarea')].filter(c => c.type !== 'hidden' && c.type !== 'submit' && c.type !== 'button' && c.offsetParent !== null && !(c.labels && c.labels.length) && !c.getAttribute('aria-label') && !c.getAttribute('aria-labelledby') && !c.closest('label')).map(c => c.name || c.id || c.type)`);
    for (const x of b) bad.push(p + ':' + x);
  }
  expect(bad.length === 0, 'unlabelled: ' + bad.join(', '));
});
await check('no sideways scrolling at 375 px', async () => {
  await size(375, 812, true);
  const wide = [];
  for (const [p, name] of [['/', 'dashboard'], ['/products', 'products'], ['/customers', 'customers'], ['/orders', 'orders']]) {
    await go(p);
    const w = await evaluate('Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) - window.innerWidth');
    if (w > 1) wide.push(`${p} +${w}px`);
    await shot(name + '-375');
  }
  await size(1440, 900, false);
  expect(wide.length === 0, wide.join(', '));
});
await check('sign out returns to the sign-in page', async () => {
  await go('/');
  await click('Sign out');
  await go('/products');
  expect((await path()).startsWith('/login'), 'at ' + await path());
  await shot('login-1440');
});
await check('the data stays after a restart', async () => {
  kill(server); await sleep(1500);
  expect(await start(), 'did not start again');
  await login();
  await go('/products?q=teapot');
  expect((await text()).includes('Green teapot'), 'product gone after restart');
});

kill(server);
ws.close(); kill(edge);
const passed = results.filter(r => r.pass).length;
writeFileSync(join(outDir, 'ui-results.json'), JSON.stringify({ project: dir, passed, total: results.length, results }, null, 1));
console.log(`${passed}/${results.length} UI checks passed`);
for (const r of results.filter(r => !r.pass)) console.log('FAIL', r.id, '-', r.detail);
process.exit(0);
