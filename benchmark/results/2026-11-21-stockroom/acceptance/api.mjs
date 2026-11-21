// Hidden acceptance tests for SPEC.md's JSON API and rules. Run against a project folder:
//   node api.mjs <project dir> <out.json>
// Installs the project's packages if it has any, starts it with a fresh database on a free port, runs every check
// (each independent: one failure does not stop the rest), stops it and writes the results.
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import net from 'node:net';

const dir = resolve(process.argv[2]);
const out = process.argv[3] ?? 'api-results.json';
const results = [];
const check = async (id, fn) => {
  try {
    const detail = await fn();
    results.push({ id, pass: true, detail: detail ?? '' });
  } catch (e) {
    results.push({ id, pass: false, detail: String(e?.message ?? e).slice(0, 300) });
  }
};
const expect = (cond, msg) => { if (!cond) throw new Error(msg); };

const freePort = () => new Promise(r => { const s = net.createServer().listen(0, () => { const p = s.address().port; s.close(() => r(p)); }); });
const port = await freePort();
const base = `http://127.0.0.1:${port}`;
const data = mkdtempSync(join(tmpdir(), 'stockroom-'));
const password = 'acc-Pa55-word';

if (existsSync(join(dir, 'package.json'))) {
  const i = spawnSync('npm', ['install', '--no-audit', '--no-fund'], { cwd: dir, shell: true, encoding: 'utf8', timeout: 300_000 });
  if (i.status !== 0) console.error('npm install failed:', (i.stderr || '').slice(-500));
}
const server = spawn('npm', ['start'], {
  cwd: dir, shell: true,
  env: { ...process.env, PORT: String(port), DB_PATH: join(data, 'db.sqlite'), ADMIN_PASSWORD: password, NODE_ENV: 'production' },
});
let log = '';
server.stdout.on('data', d => { log += d; });
server.stderr.on('data', d => { log += d; });
const stop = () => { try { if (process.platform === 'win32') spawnSync('taskkill', ['/pid', String(server.pid), '/t', '/f']); else server.kill(); } catch {} };

let up = false;
for (let i = 0; i < 60 && !up; i++) {
  try { await fetch(base + '/login', { redirect: 'manual' }); up = true; } catch { await new Promise(r => setTimeout(r, 500)); }
}

let cookie = '';
const call = async (method, path, body, opts = {}) => {
  const headers = { ...(body !== undefined ? { 'content-type': 'application/json' } : {}), ...(opts.noCookie ? {} : cookie ? { cookie } : {}) };
  const r = await fetch(base + path, { method, headers, body: body !== undefined ? JSON.stringify(body) : undefined, redirect: 'manual' });
  const text = await r.text();
  let json; try { json = JSON.parse(text); } catch {}
  return { status: r.status, json, text, headers: r.headers };
};

await check('server starts with npm start', () => { expect(up, 'no answer on ' + base + ' within 30 s: ' + log.slice(-300)); });

// ---- sign-in ----
await check('API without a session is 401', async () => { const r = await call('GET', '/api/products', undefined, { noCookie: true }); expect(r.status === 401, 'got ' + r.status); });
await check('page without a session redirects to /login', async () => {
  const r = await call('GET', '/products', undefined, { noCookie: true });
  expect([301, 302, 303, 307, 308].includes(r.status) && (r.headers.get('location') || '').includes('/login'), 'got ' + r.status + ' ' + r.headers.get('location'));
});
await check('wrong password is 401', async () => { const r = await call('POST', '/api/login', { username: 'admin', password: 'nope' }); expect(r.status === 401, 'got ' + r.status); });
await check('ADMIN_PASSWORD signs in', async () => {
  const r = await call('POST', '/api/login', { username: 'admin', password });
  expect(r.status === 200, 'got ' + r.status + ' ' + r.text.slice(0, 100));
  const set = r.headers.get('set-cookie');
  expect(set, 'no session cookie');
  cookie = set.split(/,(?=[^;]+=)/).map(c => c.split(';')[0]).join('; ');
});
await check('the password is not stored in plain text', async () => {
  const db = join(data, 'db.sqlite');
  expect(existsSync(db), 'no database at DB_PATH');
  const { readFileSync } = await import('node:fs');
  expect(!readFileSync(db).includes(Buffer.from(password)), 'the password is in the database file as written');
});

// ---- products ----
const P = {};
await check('add a product', async () => {
  const r = await call('POST', '/api/products', { sku: 'A-100', name: 'Blue mug', price: 4.5, stock: 10 });
  expect(r.status === 201, 'got ' + r.status + ' ' + r.text.slice(0, 120));
  expect(r.json?.id && r.json.sku === 'A-100' && r.json.price === 4.5 && r.json.stock === 10, 'body ' + r.text.slice(0, 120));
  P.mug = r.json.id;
});
await check('duplicate SKU is 409', async () => { const r = await call('POST', '/api/products', { sku: 'A-100', name: 'Other', price: 1, stock: 1 }); expect(r.status === 409 && r.json?.error, 'got ' + r.status + ' ' + r.text.slice(0, 80)); });
for (const [id, body] of [
  ['missing name is 400', { sku: 'B-1', price: 1, stock: 1 }],
  ['negative price is 400', { sku: 'B-2', name: 'x', price: -1, stock: 1 }],
  ['fractional stock is 400', { sku: 'B-3', name: 'x', price: 1, stock: 1.5 }],
  ['negative stock is 400', { sku: 'B-4', name: 'x', price: 1, stock: -2 }],
]) {
  await check('product: ' + id, async () => { const r = await call('POST', '/api/products', body); expect(r.status === 400 && r.json?.error, 'got ' + r.status + ' ' + r.text.slice(0, 80)); });
}
await check('get a product, 404 for an unknown one', async () => {
  const r = await call('GET', '/api/products/' + P.mug); expect(r.status === 200 && r.json?.name === 'Blue mug', 'got ' + r.status);
  const n = await call('GET', '/api/products/999999'); expect(n.status === 404, 'unknown got ' + n.status);
});
await check('edit a product', async () => {
  const r = await call('PUT', '/api/products/' + P.mug, { sku: 'A-100', name: 'Blue mug XL', price: 5, stock: 10 });
  expect(r.status === 200 && r.json?.name === 'Blue mug XL' && r.json.price === 5, 'got ' + r.status + ' ' + r.text.slice(0, 100));
});
await check('edit to a duplicate SKU is 409', async () => {
  const o = await call('POST', '/api/products', { sku: 'A-200', name: 'Red mug', price: 4, stock: 3 });
  P.red = o.json?.id;
  const r = await call('PUT', '/api/products/' + P.red, { sku: 'A-100', name: 'Red mug', price: 4, stock: 3 });
  expect(r.status === 409, 'got ' + r.status);
});
await check('25 more products page 20 at a time', async () => {
  for (let i = 0; i < 25; i++) await call('POST', '/api/products', { sku: `P-${String(i).padStart(3, '0')}`, name: `Plate ${i}`, price: 2, stock: 20 });
  const p1 = await call('GET', '/api/products?page=1');
  expect(p1.json?.total === 27 && p1.json.items?.length === 20 && p1.json.pageSize === 20 && p1.json.page === 1, 'page 1: ' + JSON.stringify({ total: p1.json?.total, n: p1.json?.items?.length, pageSize: p1.json?.pageSize }));
  const p2 = await call('GET', '/api/products?page=2');
  expect(p2.json?.items?.length === 7, 'page 2 has ' + p2.json?.items?.length);
});
await check('search matches name or SKU, case-insensitive', async () => {
  const a = await call('GET', '/api/products?q=MUG'); expect(a.json?.total === 2, 'q=MUG total ' + a.json?.total);
  const b = await call('GET', '/api/products?q=a-200'); expect(b.json?.total === 1 && b.json.items[0].name === 'Red mug', 'q=a-200 ' + b.text.slice(0, 80));
});
await check('CSV export', async () => {
  const r = await call('GET', '/api/products.csv');
  expect(r.status === 200 && (r.headers.get('content-type') || '').includes('text/csv'), 'got ' + r.status + ' ' + r.headers.get('content-type'));
  const lines = r.text.trim().split(/\r?\n/);
  expect(lines[0].replace(/"/g, '') === 'id,sku,name,price,stock', 'header ' + lines[0]);
  expect(lines.length === 28, 'lines ' + lines.length);
});

// ---- customers ----
const C = {};
await check('add a customer', async () => {
  const r = await call('POST', '/api/customers', { name: 'Lan Pham', email: 'lan@example.com', phone: '0912345678' });
  expect(r.status === 201 && r.json?.id, 'got ' + r.status + ' ' + r.text.slice(0, 100)); C.lan = r.json.id;
  const b = await call('POST', '/api/customers', { name: 'Minh Do', email: 'minh@example.com' }); C.minh = b.json?.id;
  expect(b.status === 201, 'without a phone got ' + b.status);
});
await check('duplicate email is 409', async () => { const r = await call('POST', '/api/customers', { name: 'X', email: 'lan@example.com' }); expect(r.status === 409, 'got ' + r.status); });
await check('invalid email is 400', async () => { const r = await call('POST', '/api/customers', { name: 'X', email: 'not-an-email' }); expect(r.status === 400, 'got ' + r.status); });
await check('customer search and edit', async () => {
  const s = await call('GET', '/api/customers?q=minh'); expect(s.json?.total === 1, 'search total ' + s.json?.total);
  const r = await call('PUT', '/api/customers/' + C.minh, { name: 'Minh Do', email: 'minh.do@example.com', phone: '' });
  expect(r.status === 200 && r.json?.email === 'minh.do@example.com', 'edit got ' + r.status);
});

// ---- orders ----
const O = {};
await check('create an order: stock taken, total and prices recorded', async () => {
  const r = await call('POST', '/api/orders', { customerId: C.lan, items: [{ productId: P.mug, quantity: 3 }, { productId: P.red, quantity: 1 }] });
  expect(r.status === 201, 'got ' + r.status + ' ' + r.text.slice(0, 120));
  expect(r.json?.status === 'pending' && Math.abs(r.json.total - 19) < 1e-9, 'body ' + r.text.slice(0, 160));
  expect(Array.isArray(r.json.items) && r.json.items.some(i => i.productId === P.mug && i.quantity === 3 && i.price === 5), 'items ' + JSON.stringify(r.json.items));
  O.first = r.json.id;
  const m = await call('GET', '/api/products/' + P.mug); expect(m.json?.stock === 7, 'mug stock ' + m.json?.stock);
});
await check('too little stock is 409 and changes nothing', async () => {
  const r = await call('POST', '/api/orders', { customerId: C.lan, items: [{ productId: P.mug, quantity: 2 }, { productId: P.red, quantity: 50 }] });
  expect(r.status === 409, 'got ' + r.status);
  const m = await call('GET', '/api/products/' + P.mug); expect(m.json?.stock === 7, 'mug stock changed to ' + m.json?.stock);
});
for (const [id, body] of [
  ['no items is 400', () => ({ customerId: C.lan, items: [] })],
  ['quantity 0 is 400', () => ({ customerId: C.lan, items: [{ productId: P.mug, quantity: 0 }] })],
  ['unknown customer is 400', () => ({ customerId: 999999, items: [{ productId: P.mug, quantity: 1 }] })],
  ['unknown product is 400', () => ({ customerId: C.lan, items: [{ productId: 999999, quantity: 1 }] })],
]) {
  await check('order: ' + id, async () => { const r = await call('POST', '/api/orders', body()); expect(r.status === 400, 'got ' + r.status); });
}
await check('get an order with its items, 404 for an unknown one', async () => {
  const r = await call('GET', '/api/orders/' + O.first); expect(r.status === 200 && r.json?.items?.length === 2, 'got ' + r.status);
  const n = await call('GET', '/api/orders/999999'); expect(n.status === 404, 'unknown got ' + n.status);
});
await check('status: pending to paid to shipped', async () => {
  let r = await call('PATCH', '/api/orders/' + O.first, { status: 'paid' }); expect(r.status === 200 && r.json?.status === 'paid', 'paid got ' + r.status);
  r = await call('PATCH', '/api/orders/' + O.first, { status: 'shipped' }); expect(r.status === 200 && r.json?.status === 'shipped', 'shipped got ' + r.status);
});
await check('refused status changes are 409, unknown status 400', async () => {
  const r = await call('PATCH', '/api/orders/' + O.first, { status: 'pending' }); expect(r.status === 409, 'shipped to pending got ' + r.status);
  const c = await call('PATCH', '/api/orders/' + O.first, { status: 'cancelled' }); expect(c.status === 409, 'shipped to cancelled got ' + c.status);
  const u = await call('PATCH', '/api/orders/' + O.first, { status: 'lost' }); expect(u.status === 400, 'unknown status got ' + u.status);
});
await check('cancelling puts stock back', async () => {
  const o = await call('POST', '/api/orders', { customerId: C.minh, items: [{ productId: P.red, quantity: 2 }] });
  expect(o.status === 201, 'create got ' + o.status); O.second = o.json.id;
  let s = await call('GET', '/api/products/' + P.red); expect(s.json?.stock === 0, 'red stock after order ' + s.json?.stock);
  const c = await call('PATCH', '/api/orders/' + O.second, { status: 'cancelled' }); expect(c.status === 200, 'cancel got ' + c.status);
  s = await call('GET', '/api/products/' + P.red); expect(s.json?.stock === 2, 'red stock after cancel ' + s.json?.stock);
  const again = await call('PATCH', '/api/orders/' + O.second, { status: 'paid' }); expect(again.status === 409, 'cancelled to paid got ' + again.status);
});
await check('orders newest first, filter by status, with customer names', async () => {
  const r = await call('GET', '/api/orders'); expect(r.json?.total === 2 && r.json.items[0].id === O.second, 'order ' + JSON.stringify(r.json?.items?.map(i => i.id)));
  expect(r.json.items.some(i => i.customerName === 'Lan Pham'), 'no customerName');
  const f = await call('GET', '/api/orders?status=shipped'); expect(f.json?.total === 1 && f.json.items[0].id === O.first, 'filter ' + f.json?.total);
});
await check('dashboard figures', async () => {
  const r = await call('GET', '/api/dashboard');
  expect(r.json?.products === 27 && r.json.customers === 2 && r.json.orders === 2, 'counts ' + r.text.slice(0, 120));
  expect(Math.abs(r.json.revenue - 19) < 1e-9, 'revenue ' + r.json.revenue);
  const low = (r.json.lowStock || []).map(p => p.sku).sort();
  expect(JSON.stringify(low) === JSON.stringify(['A-200']), 'lowStock ' + JSON.stringify(low));
});
await check('a product or customer in an order cannot be deleted', async () => {
  const p = await call('DELETE', '/api/products/' + P.mug); expect(p.status === 409, 'product got ' + p.status);
  const c = await call('DELETE', '/api/customers/' + C.lan); expect(c.status === 409, 'customer got ' + c.status);
});
await check('delete a product and a customer with no orders', async () => {
  const n = await call('POST', '/api/products', { sku: 'Z-1', name: 'Spare', price: 1, stock: 1 });
  const d = await call('DELETE', '/api/products/' + n.json?.id); expect(d.status === 204, 'product got ' + d.status);
  const g = await call('GET', '/api/products/' + n.json?.id); expect(g.status === 404, 'still there: ' + g.status);
  const c = await call('POST', '/api/customers', { name: 'Gone', email: 'gone@example.com' });
  const e = await call('DELETE', '/api/customers/' + c.json?.id); expect(e.status === 204, 'customer got ' + e.status);
});
await check('sign out ends the session', async () => {
  const r = await call('POST', '/api/logout'); expect(r.status === 200, 'logout got ' + r.status);
  const a = await call('GET', '/api/products'); expect(a.status === 401, 'after logout got ' + a.status);
});

stop();
const passed = results.filter(r => r.pass).length;
writeFileSync(out, JSON.stringify({ project: dir, passed, total: results.length, results, log: log.slice(-2000) }, null, 1));
console.log(`${passed}/${results.length} API checks passed`);
for (const r of results.filter(r => !r.pass)) console.log('FAIL', r.id, '-', r.detail);
process.exit(0);
