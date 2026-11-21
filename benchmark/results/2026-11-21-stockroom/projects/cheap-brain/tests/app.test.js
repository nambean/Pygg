const test = require('node:test');
const assert = require('node:assert');
const os = require('node:os');
const path = require('node:path');
const fs = require('node:fs');
const { createApp } = require('../src/app');
const { loadConfig } = require('../src/config');
const { initDb } = require('../src/db');
const { verifyPassword } = require('../src/auth');

function startOnEphemeralPort(app) {
  return new Promise((resolve) => {
    const server = app.listen(0, () => {
      resolve({ server, port: server.address().port });
    });
  });
}

async function startApp() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stockroom-'));
  const config = loadConfig({
    PORT: '0',
    DB_PATH: path.join(dir, 'test.db'),
    ADMIN_PASSWORD: 'admin123',
  });
  const app = createApp(config);
  const { server, port } = await startOnEphemeralPort(app);
  return { server, base: `http://127.0.0.1:${port}` };
}

test('GET /login serves the sign-in page on an ephemeral port', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/login`);
    assert.strictEqual(res.status, 200);
    const html = await res.text();
    assert.match(html, /Sign in/);
    assert.match(html, /Username/);
    assert.match(html, /Password/);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test('a page request without a session redirects to /login', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/`, { redirect: 'manual' });
    assert.strictEqual(res.status, 302);
    assert.strictEqual(res.headers.get('location'), '/login');
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test('admin can sign in and reach the dashboard', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/api/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username: 'admin', password: 'admin123' }),
    });
    assert.strictEqual(res.status, 200);
    assert.deepStrictEqual(await res.json(), { ok: true });

    const cookie = res.headers.get('set-cookie').split(';')[0];
    const dash = await fetch(`${base}/`, { headers: { cookie } });
    assert.strictEqual(dash.status, 200);
    assert.match(await dash.text(), /Dashboard/);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test('logout clears the session and returns 200', async () => {
  const { server, base } = await startApp();
  try {
    const login = await fetch(`${base}/api/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username: 'admin', password: 'admin123' }),
    });
    assert.strictEqual(login.status, 200);
    const cookie = login.headers.get('set-cookie').split(';')[0];

    const logout = await fetch(`${base}/api/logout`, {
      method: 'POST',
      headers: { cookie },
    });
    assert.strictEqual(logout.status, 200);
    assert.deepStrictEqual(await logout.json(), { ok: true });

    const after = await fetch(`${base}/api/dashboard`, { headers: { cookie } });
    assert.strictEqual(after.status, 401);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test('a wrong password is refused', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/api/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username: 'admin', password: 'wrong' }),
    });
    assert.strictEqual(res.status, 401);
    assert.deepStrictEqual(await res.json(), { error: 'Wrong username or password' });
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});

test('schema contains all required tables and columns', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stockroom-schema-'));
  const dbPath = path.join(dir, 'schema.db');
  let db;
  try {
    db = initDb(dbPath, 'secret-for-hash-test');

    const tables = db
      .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
      .all()
      .map(r => r.name);

    assert.ok(tables.includes('users'), 'users table must exist');
    assert.ok(tables.includes('sessions'), 'sessions table must exist');
    assert.ok(tables.includes('products'), 'products table must exist');
    assert.ok(tables.includes('customers'), 'customers table must exist');
    assert.ok(tables.includes('orders'), 'orders table must exist');
    assert.ok(tables.includes('order_items'), 'order_items table must exist');

    const colCheck = (table, cols) => {
      const names = db.prepare(`PRAGMA table_info(${table})`).all().map(c => c.name);
      for (const col of cols) {
        assert.ok(names.includes(col), `${table}.${col} column must exist`);
      }
    };

    colCheck('products', ['id', 'sku', 'name', 'price', 'stock']);
    colCheck('customers', ['id', 'name', 'email', 'phone']);
    colCheck('orders', ['id', 'customer_id', 'status', 'total', 'created_at']);
    colCheck('order_items', ['id', 'order_id', 'product_id', 'quantity', 'price']);
  } finally {
    if (db) db.close();
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('seed inserts admin user with hashed password', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stockroom-seed-'));
  const dbPath = path.join(dir, 'seed.db');
  let db;
  try {
    const customPass = 'custom-admin-pass';
    db = initDb(dbPath, customPass);

    const row = db.prepare('SELECT * FROM users WHERE username = ?').get('admin');
    assert.ok(row, 'admin row must exist in users table');
    assert.strictEqual(row.id, 1);

    assert.ok(
      typeof row.password_hash === 'string' && row.password_hash.length > 30,
      'password_hash must be a substantial hashed string'
    );
    assert.ok(
      row.password_hash.includes(':'),
      'hashed password must contain salt:key separator'
    );

    // Verify the hash works — login with custom password verifies correctly
    assert.strictEqual(verifyPassword(customPass, row.password_hash), true);
  } finally {
    if (db) db.close();
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
