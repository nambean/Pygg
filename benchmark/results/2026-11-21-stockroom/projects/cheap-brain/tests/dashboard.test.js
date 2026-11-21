const test = require('node:test');
const assert = require('node:assert');
const os = require('node:os');
const path = require('node:path');
const fs = require('node:fs');
const { createApp } = require('../src/app');
const { loadConfig } = require('../src/config');

function startOnEphemeralPort(app) {
  return new Promise((resolve) => {
    const server = app.listen(0, () => {
      resolve({ server, port: server.address().port });
    });
  });
}

async function startApp() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stockroom-dashboard-'));
  const config = loadConfig({
    PORT: '0',
    DB_PATH: path.join(dir, 'test.db'),
    ADMIN_PASSWORD: 'admin123',
  });
  const app = createApp(config);
  const { server, port } = await startOnEphemeralPort(app);
  const base = `http://127.0.0.1:${port}`;

  const login = await fetch(`${base}/api/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username: 'admin', password: 'admin123' }),
  });
  assert.strictEqual(login.status, 200);
  const cookie = login.headers.get('set-cookie').split(';')[0];

  return { server, base, cookie };
}

async function close(server) {
  await new Promise((resolve) => server.close(resolve));
}

function jsonReq(base, cookie, method, url, body) {
  const headers = { cookie };
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  return fetch(`${base}${url}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
}

async function seedProduct(base, cookie, sku, price, stock) {
  const res = await jsonReq(base, cookie, 'POST', '/api/products', {
    sku,
    name: `Product ${sku}`,
    price,
    stock,
  });
  assert.strictEqual(res.status, 201);
  return res.json();
}

async function seedCustomer(base, cookie, name, email) {
  const res = await jsonReq(base, cookie, 'POST', '/api/customers', { name, email });
  assert.strictEqual(res.status, 201);
  return res.json();
}

async function createOrder(base, cookie, customerId, productId, quantity) {
  const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
    customerId,
    items: [{ productId, quantity }],
  });
  assert.strictEqual(res.status, 201);
  return res.json();
}

test('GET /api/dashboard requires a session', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/api/dashboard`);
    assert.strictEqual(res.status, 401);
  } finally {
    await close(server);
  }
});

test('GET /api/dashboard returns product, customer and order counts', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const p1 = await seedProduct(base, cookie, 'COUNT-P1', 10, 20);
    const p2 = await seedProduct(base, cookie, 'COUNT-P2', 5, 20);
    const p3 = await seedProduct(base, cookie, 'COUNT-P3', 2, 20);
    const c1 = await seedCustomer(base, cookie, 'Alice', 'alice@count.com');
    const c2 = await seedCustomer(base, cookie, 'Bob', 'bob@count.com');

    await createOrder(base, cookie, c1.id, p1.id, 1);
    await createOrder(base, cookie, c2.id, p2.id, 2);

    const res = await jsonReq(base, cookie, 'GET', '/api/dashboard');
    assert.strictEqual(res.status, 200);
    const body = await res.json();
    assert.strictEqual(body.products, 3);
    assert.strictEqual(body.customers, 2);
    assert.strictEqual(body.orders, 2);
  } finally {
    await close(server);
  }
});

test('GET /api/dashboard includes revenue from paid orders', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'REV-PAID', 0.25, 20);
    const customer = await seedCustomer(base, cookie, 'Rev Paid', 'revpaid@example.com');

    const order = await createOrder(base, cookie, customer.id, product.id, 1);
    const paid = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'paid' });
    assert.strictEqual(paid.status, 200);

    const res = await jsonReq(base, cookie, 'GET', '/api/dashboard');
    assert.strictEqual(res.status, 200);
    const body = await res.json();
    assert.strictEqual(body.revenue, 0.25);
  } finally {
    await close(server);
  }
});

test('GET /api/dashboard includes revenue from shipped orders', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'REV-SHIP', 0.6, 20);
    const customer = await seedCustomer(base, cookie, 'Rev Ship', 'revship@example.com');

    const order = await createOrder(base, cookie, customer.id, product.id, 1);
    await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'paid' });
    const shipped = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'shipped' });
    assert.strictEqual(shipped.status, 200);

    const res = await jsonReq(base, cookie, 'GET', '/api/dashboard');
    assert.strictEqual(res.status, 200);
    const body = await res.json();
    assert.strictEqual(body.revenue, 0.6);
  } finally {
    await close(server);
  }
});

test('GET /api/dashboard revenue excludes pending and cancelled orders', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'REV-EXCL', 10, 50);
    const customer = await seedCustomer(base, cookie, 'Rev Excl', 'revexcl@example.com');

    const pendingOrder = await createOrder(base, cookie, customer.id, product.id, 1);
    const cancelledOrder = await createOrder(base, cookie, customer.id, product.id, 1);
    const paidOrder = await createOrder(base, cookie, customer.id, product.id, 1);

    await jsonReq(base, cookie, 'PATCH', `/api/orders/${cancelledOrder.id}`, { status: 'cancelled' });
    await jsonReq(base, cookie, 'PATCH', `/api/orders/${paidOrder.id}`, { status: 'paid' });

    const res = await jsonReq(base, cookie, 'GET', '/api/dashboard');
    assert.strictEqual(res.status, 200);
    const body = await res.json();
    assert.strictEqual(body.revenue, 10);
  } finally {
    await close(server);
  }
});

function statValue(html, name) {
  const re = new RegExp(
    `data-stat="${name}"[\\s\\S]*?<span class="stat-value">([^<]*)</span>`
  );
  const match = html.match(re);
  return match ? match[1] : null;
}

test('GET / dashboard requires a session', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/`, { redirect: 'manual' });
    assert.strictEqual(res.status, 302);
    assert.strictEqual(res.headers.get('location'), '/login');
  } finally {
    await close(server);
  }
});

test('GET / dashboard shows product, customer and order counts', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const p1 = await seedProduct(base, cookie, 'DASH-P1', 10, 20);
    const p2 = await seedProduct(base, cookie, 'DASH-P2', 5, 20);
    await seedProduct(base, cookie, 'DASH-P3', 2, 20);
    const c1 = await seedCustomer(base, cookie, 'Alice', 'alice@dash.com');
    const c2 = await seedCustomer(base, cookie, 'Bob', 'bob@dash.com');

    await createOrder(base, cookie, c1.id, p1.id, 1);
    await createOrder(base, cookie, c2.id, p2.id, 2);

    const res = await fetch(`${base}/`, { headers: { cookie } });
    assert.strictEqual(res.status, 200);
    const html = await res.text();

    assert.strictEqual(statValue(html, 'products'), '3');
    assert.strictEqual(statValue(html, 'customers'), '2');
    assert.strictEqual(statValue(html, 'orders'), '2');
  } finally {
    await close(server);
  }
});

test('GET / dashboard shows revenue from paid and shipped orders', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'DASH-REV', 0.25, 20);
    const customer = await seedCustomer(base, cookie, 'Rev', 'rev@dash.com');
    const order = await createOrder(base, cookie, customer.id, product.id, 1);
    const paid = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'paid' });
    assert.strictEqual(paid.status, 200);

    const res = await fetch(`${base}/`, { headers: { cookie } });
    assert.strictEqual(res.status, 200);
    const html = await res.text();
    assert.strictEqual(statValue(html, 'revenue'), '$0.25');
  } finally {
    await close(server);
  }
});

test('GET / dashboard shows the low-stock list', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const low1 = await seedProduct(base, cookie, 'DASH-LOW-1', 1, 0);
    const low2 = await seedProduct(base, cookie, 'DASH-LOW-2', 2, 4);
    const ok = await seedProduct(base, cookie, 'DASH-OK-1', 3, 5);
    const plenty = await seedProduct(base, cookie, 'DASH-OK-2', 4, 100);

    const res = await fetch(`${base}/`, { headers: { cookie } });
    assert.strictEqual(res.status, 200);
    const html = await res.text();

    assert.ok(html.includes(`data-product-id="${low1.id}"`));
    assert.ok(html.includes(`data-product-id="${low2.id}"`));
    assert.ok(html.includes('DASH-LOW-1'));
    assert.ok(html.includes('DASH-LOW-2'));
    assert.ok(!html.includes(`data-product-id="${ok.id}"`));
    assert.ok(!html.includes(`data-product-id="${plenty.id}"`));
    assert.ok(!html.includes('DASH-OK-1'));
    assert.ok(!html.includes('DASH-OK-2'));
  } finally {
    await close(server);
  }
});

test('GET /api/dashboard lists products with fewer than 5 in stock', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const low1 = await seedProduct(base, cookie, 'LOW-1', 1, 0);
    const low2 = await seedProduct(base, cookie, 'LOW-2', 2, 4);
    const ok = await seedProduct(base, cookie, 'OK-1', 3, 5);
    const plenty = await seedProduct(base, cookie, 'OK-2', 4, 100);

    const res = await jsonReq(base, cookie, 'GET', '/api/dashboard');
    assert.strictEqual(res.status, 200);
    const body = await res.json();

    const ids = body.lowStock.map((p) => p.id).sort((a, b) => a - b);
    assert.deepStrictEqual(ids, [low1.id, low2.id]);

    const byId = new Map(body.lowStock.map((p) => [p.id, p]));
    assert.strictEqual(byId.get(low1.id).stock, 0);
    assert.strictEqual(byId.get(low2.id).stock, 4);
    assert.ok(body.lowStock.every((p) => p.stock < 5));
    assert.ok(!ids.includes(ok.id));
    assert.ok(!ids.includes(plenty.id));

    for (const p of body.lowStock) {
      assert.strictEqual(typeof p.id, 'number');
      assert.strictEqual(typeof p.sku, 'string');
      assert.strictEqual(typeof p.name, 'string');
      assert.strictEqual(typeof p.stock, 'number');
    }
  } finally {
    await close(server);
  }
});
