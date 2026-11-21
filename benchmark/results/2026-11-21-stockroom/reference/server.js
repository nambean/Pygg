// Reference implementation of SPEC.md, written only to check that the acceptance suites can be passed.
const http = require('node:http');
const crypto = require('node:crypto');
const { DatabaseSync } = require('node:sqlite');

const db = new DatabaseSync(process.env.DB_PATH || './data.db');
db.exec(`CREATE TABLE IF NOT EXISTS users (name TEXT PRIMARY KEY, salt TEXT, hash TEXT);
CREATE TABLE IF NOT EXISTS products (id INTEGER PRIMARY KEY, sku TEXT UNIQUE NOT NULL, name TEXT NOT NULL, price REAL NOT NULL, stock INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT UNIQUE NOT NULL, phone TEXT);
CREATE TABLE IF NOT EXISTS orders (id INTEGER PRIMARY KEY, customer_id INTEGER NOT NULL, status TEXT NOT NULL, total REAL NOT NULL, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS order_items (order_id INTEGER, product_id INTEGER, quantity INTEGER, price REAL);`);
const hash = (pw, salt) => crypto.scryptSync(pw, salt, 32).toString('hex');
{
  const salt = crypto.randomBytes(8).toString('hex');
  db.prepare('INSERT OR REPLACE INTO users VALUES (?, ?, ?)').run('admin', salt, hash(process.env.ADMIN_PASSWORD || 'admin123', salt));
}
const sessions = new Set();
const json = (res, status, body) => { res.writeHead(status, { 'content-type': 'application/json' }); res.end(body === undefined ? '' : JSON.stringify(body)); };
const err = (res, status, message) => json(res, status, { error: message });
const body = req => new Promise(r => { let d = ''; req.on('data', c => d += c); req.on('end', () => { try { r(JSON.parse(d || '{}')); } catch { r(null); } }); });
const sid = req => (req.headers.cookie || '').split(/;\s*/).map(c => c.split('=')).find(c => c[0] === 'sid')?.[1];
const product = id => db.prepare('SELECT * FROM products WHERE id = ?').get(id);
const validProduct = b => b && typeof b.sku === 'string' && b.sku.trim() && typeof b.name === 'string' && b.name.trim()
  && typeof b.price === 'number' && b.price >= 0 && Number.isInteger(b.stock) && b.stock >= 0;
const validCustomer = b => b && typeof b.name === 'string' && b.name.trim() && typeof b.email === 'string' && /^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(b.email);
const orderOf = id => {
  const o = db.prepare('SELECT o.*, c.name AS customerName FROM orders o JOIN customers c ON c.id = o.customer_id WHERE o.id = ?').get(id);
  if (!o) return null;
  const items = db.prepare('SELECT product_id AS productId, quantity, price FROM order_items WHERE order_id = ?').all(id);
  return { id: o.id, customerId: o.customer_id, customerName: o.customerName, status: o.status, total: o.total, createdAt: o.created_at, items };
};
const NEXT = { pending: ['paid', 'cancelled'], paid: ['shipped', 'cancelled'], shipped: [], cancelled: [] };

async function api(req, res, url) {
  const m = req.method, p = url.pathname;
  if (m === 'POST' && p === '/api/login') {
    const b = await body(req);
    const u = b && db.prepare('SELECT * FROM users WHERE name = ?').get(b.username);
    if (!u || hash(String(b.password), u.salt) !== u.hash) return err(res, 401, 'Wrong user name or password');
    const s = crypto.randomBytes(16).toString('hex'); sessions.add(s);
    res.setHeader('set-cookie', `sid=${s}; HttpOnly; Path=/; SameSite=Lax`);
    return json(res, 200, { ok: true });
  }
  if (!sessions.has(sid(req))) return err(res, 401, 'Sign in first');
  if (m === 'POST' && p === '/api/logout') { sessions.delete(sid(req)); return json(res, 200, { ok: true }); }
  if (m === 'GET' && p === '/api/products.csv') {
    const rows = db.prepare('SELECT * FROM products ORDER BY id').all();
    res.writeHead(200, { 'content-type': 'text/csv' });
    return res.end(['id,sku,name,price,stock', ...rows.map(r => [r.id, r.sku, `"${r.name.replace(/"/g, '""')}"`, r.price, r.stock].join(','))].join('\n'));
  }
  if (m === 'GET' && p === '/api/products') {
    const q = `%${(url.searchParams.get('q') || '').toLowerCase()}%`, page = Math.max(1, +url.searchParams.get('page') || 1);
    const where = 'WHERE lower(name) LIKE ? OR lower(sku) LIKE ?';
    const total = db.prepare(`SELECT count(*) n FROM products ${where}`).get(q, q).n;
    const items = db.prepare(`SELECT * FROM products ${where} ORDER BY id LIMIT 20 OFFSET ?`).all(q, q, (page - 1) * 20);
    return json(res, 200, { items, total, page, pageSize: 20 });
  }
  if (m === 'POST' && p === '/api/products') {
    const b = await body(req); if (!validProduct(b)) return err(res, 400, 'SKU, name, a price of at least 0 and a whole stock of at least 0 are needed');
    try { const r = db.prepare('INSERT INTO products (sku, name, price, stock) VALUES (?, ?, ?, ?)').run(b.sku.trim(), b.name.trim(), b.price, b.stock); return json(res, 201, product(r.lastInsertRowid)); }
    catch { return err(res, 409, `SKU ${b.sku} already exists`); }
  }
  let r = p.match(/^\/api\/products\/(\d+)$/);
  if (r) {
    const pr = product(+r[1]); if (!pr) return err(res, 404, 'No such product');
    if (m === 'GET') return json(res, 200, pr);
    if (m === 'PUT') {
      const b = await body(req); if (!validProduct(b)) return err(res, 400, 'Invalid product');
      try { db.prepare('UPDATE products SET sku = ?, name = ?, price = ?, stock = ? WHERE id = ?').run(b.sku.trim(), b.name.trim(), b.price, b.stock, pr.id); return json(res, 200, product(pr.id)); }
      catch { return err(res, 409, `SKU ${b.sku} already exists`); }
    }
    if (m === 'DELETE') {
      if (db.prepare('SELECT 1 FROM order_items WHERE product_id = ?').get(pr.id)) return err(res, 409, 'This product is in an order and cannot be deleted');
      db.prepare('DELETE FROM products WHERE id = ?').run(pr.id); res.writeHead(204); return res.end();
    }
  }
  if (m === 'GET' && p === '/api/customers') {
    const q = `%${(url.searchParams.get('q') || '').toLowerCase()}%`;
    const items = db.prepare('SELECT * FROM customers WHERE lower(name) LIKE ? OR lower(email) LIKE ? ORDER BY id').all(q, q);
    return json(res, 200, { items, total: items.length });
  }
  if (m === 'POST' && p === '/api/customers') {
    const b = await body(req); if (!validCustomer(b)) return err(res, 400, 'A name and a valid email are needed');
    try { const x = db.prepare('INSERT INTO customers (name, email, phone) VALUES (?, ?, ?)').run(b.name.trim(), b.email.trim(), b.phone || ''); return json(res, 201, db.prepare('SELECT * FROM customers WHERE id = ?').get(x.lastInsertRowid)); }
    catch { return err(res, 409, `The email ${b.email} is already in use`); }
  }
  r = p.match(/^\/api\/customers\/(\d+)$/);
  if (r) {
    const c = db.prepare('SELECT * FROM customers WHERE id = ?').get(+r[1]); if (!c) return err(res, 404, 'No such customer');
    if (m === 'GET') return json(res, 200, c);
    if (m === 'PUT') {
      const b = await body(req); if (!validCustomer(b)) return err(res, 400, 'Invalid customer');
      try { db.prepare('UPDATE customers SET name = ?, email = ?, phone = ? WHERE id = ?').run(b.name, b.email, b.phone || '', c.id); return json(res, 200, db.prepare('SELECT * FROM customers WHERE id = ?').get(c.id)); }
      catch { return err(res, 409, 'That email is already in use'); }
    }
    if (m === 'DELETE') {
      if (db.prepare('SELECT 1 FROM orders WHERE customer_id = ?').get(c.id)) return err(res, 409, 'This customer has orders and cannot be deleted');
      db.prepare('DELETE FROM customers WHERE id = ?').run(c.id); res.writeHead(204); return res.end();
    }
  }
  if (m === 'GET' && p === '/api/orders') {
    const s = url.searchParams.get('status');
    const ids = db.prepare(`SELECT id FROM orders ${s ? 'WHERE status = ?' : ''} ORDER BY id DESC`).all(...(s ? [s] : []));
    const items = ids.map(o => orderOf(o.id)); return json(res, 200, { items, total: items.length });
  }
  if (m === 'POST' && p === '/api/orders') {
    const b = await body(req);
    if (!b || !Number.isInteger(b.customerId) || !Array.isArray(b.items) || !b.items.length || b.items.some(i => !Number.isInteger(i.productId) || !Number.isInteger(i.quantity) || i.quantity < 1)) return err(res, 400, 'A customer and at least one product line are needed');
    if (!db.prepare('SELECT 1 FROM customers WHERE id = ?').get(b.customerId)) return err(res, 400, 'Unknown customer');
    const lines = b.items.map(i => ({ ...i, p: product(i.productId) }));
    if (lines.some(l => !l.p)) return err(res, 400, 'Unknown product');
    const want = {}; for (const l of lines) want[l.productId] = (want[l.productId] || 0) + l.quantity;
    for (const [id, q] of Object.entries(want)) { const pr = product(+id); if (pr.stock < q) return err(res, 409, `Not enough stock of ${pr.name}: ${pr.stock} left`); }
    db.exec('BEGIN');
    const total = lines.reduce((s, l) => s + l.p.price * l.quantity, 0);
    const o = db.prepare("INSERT INTO orders (customer_id, status, total, created_at) VALUES (?, 'pending', ?, ?)").run(b.customerId, total, new Date().toISOString());
    for (const l of lines) {
      db.prepare('INSERT INTO order_items VALUES (?, ?, ?, ?)').run(o.lastInsertRowid, l.productId, l.quantity, l.p.price);
      db.prepare('UPDATE products SET stock = stock - ? WHERE id = ?').run(l.quantity, l.productId);
    }
    db.exec('COMMIT');
    return json(res, 201, orderOf(o.lastInsertRowid));
  }
  r = p.match(/^\/api\/orders\/(\d+)$/);
  if (r) {
    const o = orderOf(+r[1]); if (!o) return err(res, 404, 'No such order');
    if (m === 'GET') return json(res, 200, o);
    if (m === 'PATCH') {
      const b = await body(req); if (!b || !(b.status in NEXT)) return err(res, 400, 'Unknown status');
      if (!NEXT[o.status].includes(b.status)) return err(res, 409, `An order that is ${o.status} cannot become ${b.status}`);
      if (b.status === 'cancelled') for (const i of o.items) db.prepare('UPDATE products SET stock = stock + ? WHERE id = ?').run(i.quantity, i.productId);
      db.prepare('UPDATE orders SET status = ? WHERE id = ?').run(b.status, o.id); return json(res, 200, orderOf(o.id));
    }
  }
  if (m === 'GET' && p === '/api/dashboard') {
    const n = t => db.prepare(`SELECT count(*) n FROM ${t}`).get().n;
    return json(res, 200, { products: n('products'), customers: n('customers'), orders: n('orders'),
      revenue: db.prepare("SELECT coalesce(sum(total), 0) s FROM orders WHERE status IN ('paid', 'shipped')").get().s,
      lowStock: db.prepare('SELECT id, sku, name, stock FROM products WHERE stock < 5 ORDER BY stock').all() });
  }
  return err(res, 404, 'Not found');
}

const CSS = `body{font:15px system-ui;margin:0;color:#222}nav{display:flex;flex-wrap:wrap;gap:12px;padding:12px;background:#f2f2f2;align-items:center}
main{padding:12px;max-width:960px}table{width:100%;border-collapse:collapse}td,th{border-bottom:1px solid #ddd;padding:6px;text-align:left}
form{display:flex;flex-wrap:wrap;gap:8px;margin:12px 0;align-items:end}label{display:flex;flex-direction:column;font-size:13px}
input,select{padding:6px;max-width:100%}.err{color:#b00020}.wrap{overflow-x:auto}`;
const page = (title, body, script = '') => `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${title}</title><style>${CSS}</style></head>
<body>${title === 'Sign in' ? '' : `<nav><a href="/">Dashboard</a><a href="/products">Products</a><a href="/customers">Customers</a><a href="/orders">Orders</a><button id="out">Sign out</button></nav>`}
<main><h1>${title}</h1>${body}<p class="err" id="err" role="alert"></p></main><script>
const $ = s => document.querySelector(s); const esc = s => String(s ?? '').replace(/[&<>"]/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'})[c]);
async function call(m, u, b) { const r = await fetch(u, {method: m, headers: b ? {'content-type': 'application/json'} : {}, body: b ? JSON.stringify(b) : undefined}); const j = r.status === 204 ? {} : await r.json(); if (!r.ok) { $('#err').textContent = j.error; throw j; } $('#err').textContent = ''; return j; }
if ($('#out')) $('#out').onclick = async () => { await call('POST', '/api/logout'); location = '/login'; };
${script}</script></body></html>`;
const PAGES = {
  '/login': page('Sign in', `<form id="f"><label>Username<input id="u" name="username"></label><label>Password<input id="p" type="password" name="password"></label><button>Sign in</button></form>`,
    `$('#f').onsubmit = async e => { e.preventDefault(); try { await call('POST', '/api/login', {username: $('#u').value, password: $('#p').value}); location = '/'; } catch {} };`),
  '/': page('Dashboard', `<div id="d"></div>`, `call('GET', '/api/dashboard').then(d => $('#d').innerHTML = '<p>Products: ' + d.products + ' · Customers: ' + d.customers + ' · Orders: ' + d.orders + ' · Revenue: ' + d.revenue.toFixed(2) + '</p><h2>Low stock</h2><ul>' + d.lowStock.map(p => '<li>' + esc(p.name) + ' (' + p.stock + ')</li>').join('') + '</ul>');`),
  '/products': page('Products', `<form id="s"><label>Search<input type="search" name="q" id="q"></label><button>Search</button></form>
<form id="f"><label>SKU<input id="sku"></label><label>Name<input id="name"></label><label>Price<input id="price" type="number" step="0.01"></label><label>Stock<input id="stock" type="number"></label><button>Add product</button></form>
<a href="/api/products.csv">Download CSV</a><div class="wrap"><table><thead><tr><th>SKU</th><th>Name</th><th>Price</th><th>Stock</th><th></th></tr></thead><tbody id="t"></tbody></table></div><p id="pg"></p>`,
    `const qs = new URLSearchParams(location.search); $('#q').value = qs.get('q') || '';
async function load() { const page = +(qs.get('page') || 1); const d = await call('GET', '/api/products?q=' + encodeURIComponent($('#q').value) + '&page=' + page);
  $('#t').innerHTML = d.items.map(p => '<tr><td>' + esc(p.sku) + '</td><td>' + esc(p.name) + '</td><td>' + p.price + '</td><td>' + p.stock + '</td><td><button data-e="' + p.id + '">Edit</button> <button data-d="' + p.id + '">Delete</button></td></tr>').join('');
  const q = '&q=' + encodeURIComponent($('#q').value);
  $('#pg').innerHTML = (page > 1 ? '<a href="?page=' + (page - 1) + q + '">Previous</a> ' : '') + (page * 20 < d.total ? '<a rel="next" href="?page=' + (page + 1) + q + '">Next</a>' : ''); }
$('#t').onclick = async e => { const d = e.target.dataset; if (d.d && confirm('Delete this product?')) { try { await call('DELETE', '/api/products/' + d.d); load(); } catch {} }
  if (d.e) { const p = await call('GET', '/api/products/' + d.e); const name = prompt('Name', p.name); if (name) { try { await call('PUT', '/api/products/' + d.e, {...p, name}); load(); } catch {} } } };
$('#f').onsubmit = async e => { e.preventDefault(); try { await call('POST', '/api/products', {sku: $('#sku').value, name: $('#name').value, price: +$('#price').value, stock: +$('#stock').value}); $('#f').reset(); load(); } catch {} };
load();`),
  '/customers': page('Customers', `<form id="f"><label>Name<input id="name"></label><label>Email<input id="email" type="email"></label><label>Phone<input id="phone"></label><button>Add customer</button></form><div class="wrap"><table><thead><tr><th>Name</th><th>Email</th><th>Phone</th><th></th></tr></thead><tbody id="t"></tbody></table></div>`,
    `async function load() { const d = await call('GET', '/api/customers'); $('#t').innerHTML = d.items.map(c => '<tr><td>' + esc(c.name) + '</td><td>' + esc(c.email) + '</td><td>' + esc(c.phone) + '</td><td><button data-d="' + c.id + '">Delete</button></td></tr>').join(''); }
$('#t').onclick = async e => { if (e.target.dataset.d && confirm('Delete?')) { try { await call('DELETE', '/api/customers/' + e.target.dataset.d); load(); } catch {} } };
$('#f').onsubmit = async e => { e.preventDefault(); try { await call('POST', '/api/customers', {name: $('#name').value, email: $('#email').value, phone: $('#phone').value}); $('#f').reset(); load(); } catch {} };
load();`),
  '/orders': page('Orders', `<form id="f"><label>Customer<select id="c"></select></label><label>Product<select id="p"></select></label><label>Quantity<input id="q" type="number" min="1" value="1"></label><button>Create order</button></form>
<form id="fl"><label>Status<select id="st"><option value="">All</option><option>pending</option><option>paid</option><option>shipped</option><option>cancelled</option></select></label></form><div class="wrap"><table><thead><tr><th>#</th><th>Customer</th><th>Status</th><th>Total</th><th>Date</th><th></th></tr></thead><tbody id="t"></tbody></table></div>`,
    `const NEXT = {pending: ['paid', 'cancelled'], paid: ['shipped', 'cancelled'], shipped: [], cancelled: []};
async function load() { const d = await call('GET', '/api/orders' + ($('#st').value ? '?status=' + $('#st').value : ''));
  $('#t').innerHTML = d.items.map(o => '<tr><td>' + o.id + '</td><td>' + esc(o.customerName) + '</td><td>' + o.status + '</td><td>' + o.total.toFixed(2) + '</td><td>' + o.createdAt.slice(0, 10) + '</td><td>' + NEXT[o.status].map(s => '<button data-o="' + o.id + '" data-s="' + s + '">' + (s === 'paid' ? 'Paid' : s === 'shipped' ? 'Shipped' : 'Cancel') + '</button>').join(' ') + '</td></tr>').join(''); }
(async () => { const c = await call('GET', '/api/customers'); $('#c').innerHTML = c.items.map(x => '<option value="' + x.id + '">' + esc(x.name) + '</option>').join('');
  const p = await call('GET', '/api/products?page=1&q='); let all = p.items; for (let i = 2; (i - 1) * 20 < p.total; i++) all = all.concat((await call('GET', '/api/products?page=' + i)).items);
  $('#p').innerHTML = all.map(x => '<option value="' + x.id + '">' + esc(x.name) + '</option>').join(''); load(); })();
$('#st').onchange = load;
$('#t').onclick = async e => { const d = e.target.dataset; if (d.o) { try { await call('PATCH', '/api/orders/' + d.o, {status: d.s}); load(); } catch {} } };
$('#f').onsubmit = async e => { e.preventDefault(); try { await call('POST', '/api/orders', {customerId: +$('#c').value, items: [{productId: +$('#p').value, quantity: +$('#q').value}]}); load(); } catch {} };`),
};

http.createServer(async (req, res) => {
  const url = new URL(req.url, 'http://x');
  if (url.pathname.startsWith('/api/')) return api(req, res, url).catch(e => err(res, 500, String(e)));
  if (url.pathname !== '/login' && !sessions.has(sid(req))) { res.writeHead(302, { location: '/login' }); return res.end(); }
  const html = PAGES[url.pathname]; if (!html) { res.writeHead(404); return res.end('Not found'); }
  res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); res.end(html);
}).listen(+process.env.PORT || 3000);
