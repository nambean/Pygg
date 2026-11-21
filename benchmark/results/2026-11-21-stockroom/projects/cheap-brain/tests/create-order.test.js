const test = require('node:test');
const assert = require('node:assert');
const os = require('node:os');
const path = require('node:path');
const fs = require('node:fs');
const { createApp } = require('../src/app');
const { loadConfig } = require('../src/config');
const { DatabaseSync } = require('node:sqlite');

function startOnEphemeralPort(app) {
  return new Promise((resolve) => {
    const server = app.listen(0, () => {
      resolve({ server, port: server.address().port });
    });
  });
}

async function startApp() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stockroom-create-order-'));
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

  return { server, base, cookie, dir };
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

function openDb(dir) {
  return new DatabaseSync(path.join(dir, 'test.db'));
}

test('GET /orders serves the create order page with customer selection and line items', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await fetch(`${base}/orders`, { headers: { cookie } });
    assert.strictEqual(res.status, 200);
    const html = await res.text();

    assert.ok(html.includes('id="create-order-form"'), 'page has a create order form');
    assert.ok(html.includes('id="order-customer"'), 'page has a customer selection');
    assert.ok(html.includes('<select'), 'customer selection is a select element');
    assert.ok(html.includes('id="order-lines"'), 'page has a line items container');
    assert.ok(html.includes('class="order-line-product"'), 'page has a product selector for line items');
    assert.ok(html.includes('class="order-line-quantity"'), 'page has a quantity input for line items');
    assert.ok(html.includes('Create order'), 'page has a Create order button');
    assert.ok(html.includes('id="orders-table"'), 'page has an orders table');
    assert.ok(html.includes('id="orders-body"'), 'page has an orders table body');
  } finally {
    await close(server);
  }
});

test('GET /orders redirects to login without a session', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/orders`, { redirect: 'manual' });
    assert.strictEqual(res.status, 302);
    assert.strictEqual(res.headers.get('location'), '/login');
  } finally {
    await close(server);
  }
});

test('creating an order through the page reduces stock', async () => {
  const { server, base, cookie, dir } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'PAGE-STOCK', 5, 10);
    const customer = await seedCustomer(base, cookie, 'Page Customer', 'page@example.com');

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 4 }],
    });
    assert.strictEqual(res.status, 201);
    const order = await res.json();
    assert.strictEqual(order.status, 'pending');
    assert.strictEqual(order.total, 20);

    const db = openDb(dir);
    try {
      const row = db.prepare('SELECT stock FROM products WHERE id = ?').get(product.id);
      assert.strictEqual(row.stock, 6);
    } finally {
      db.close();
    }
  } finally {
    await close(server);
  }
});

test('creating an order with too little stock returns 409 and changes nothing', async () => {
  const { server, base, cookie, dir } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'PAGE-LOW', 5, 2);
    const customer = await seedCustomer(base, cookie, 'Low Customer', 'low@example.com');

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 3 }],
    });
    assert.strictEqual(res.status, 409);
    const body = await res.json();
    assert.strictEqual(typeof body.error, 'string');

    const db = openDb(dir);
    try {
      const stock = db.prepare('SELECT stock FROM products WHERE id = ?').get(product.id);
      assert.strictEqual(stock.stock, 2);
      const orderCount = Number(db.prepare('SELECT COUNT(*) AS c FROM orders').get().c);
      assert.strictEqual(orderCount, 0);
      const itemCount = Number(db.prepare('SELECT COUNT(*) AS c FROM order_items').get().c);
      assert.strictEqual(itemCount, 0);
    } finally {
      db.close();
    }
  } finally {
    await close(server);
  }
});
