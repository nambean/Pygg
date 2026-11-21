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
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stockroom-products-'));
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

test('GET /api/products requires a session', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/api/products`);
    assert.strictEqual(res.status, 401);
  } finally {
    await close(server);
  }
});

test('POST /api/products creates a product and returns 201', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'SKU-1',
      name: 'Widget',
      price: 9.99,
      stock: 12,
    });
    assert.strictEqual(res.status, 201);
    const body = await res.json();
    assert.strictEqual(body.sku, 'SKU-1');
    assert.strictEqual(body.name, 'Widget');
    assert.strictEqual(body.price, 9.99);
    assert.strictEqual(body.stock, 12);
    assert.strictEqual(typeof body.id, 'number');
  } finally {
    await close(server);
  }
});

test('POST /api/products trims whitespace and rejects empty fields', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const good = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: '  SKU-2  ',
      name: '  Gadget  ',
      price: 5,
      stock: 0,
    });
    assert.strictEqual(good.status, 201);
    assert.strictEqual((await good.json()).sku, 'SKU-2');

    const bad = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: '   ',
      name: 'Widget',
      price: 5,
      stock: 1,
    });
    assert.strictEqual(bad.status, 400);
  } finally {
    await close(server);
  }
});

test('POST /api/products rejects invalid fields with 400', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const cases = [
      { sku: 'A', name: '', price: 5, stock: 1 },
      { sku: 'A', name: 'Widget', price: -1, stock: 1 },
      { sku: 'A', name: 'Widget', price: 'x', stock: 1 },
      { sku: 'A', name: 'Widget', price: 5, stock: -1 },
      { sku: 'A', name: 'Widget', price: 5, stock: 1.5 },
      { sku: '', name: 'Widget', price: 5, stock: 1 },
    ];
    for (const payload of cases) {
      const res = await jsonReq(base, cookie, 'POST', '/api/products', payload);
      assert.strictEqual(res.status, 400, JSON.stringify(payload));
      const body = await res.json();
      assert.strictEqual(typeof body.error, 'string');
    }
  } finally {
    await close(server);
  }
});

test('POST /api/products returns 409 on a duplicate SKU', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const first = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'DUP',
      name: 'First',
      price: 1,
      stock: 1,
    });
    assert.strictEqual(first.status, 201);

    const second = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'DUP',
      name: 'Second',
      price: 2,
      stock: 2,
    });
    assert.strictEqual(second.status, 409);
    assert.strictEqual(typeof (await second.json()).error, 'string');
  } finally {
    await close(server);
  }
});

test('GET /api/products lists products with pagination and total', async () => {
  const { server, base, cookie } = await startApp();
  try {
    for (let i = 1; i <= 25; i += 1) {
      const res = await jsonReq(base, cookie, 'POST', '/api/products', {
        sku: `SKU-${i}`,
        name: `Product ${i}`,
        price: i,
        stock: i,
      });
      assert.strictEqual(res.status, 201);
    }

    const page1 = await jsonReq(base, cookie, 'GET', '/api/products?page=1');
    assert.strictEqual(page1.status, 200);
    const body1 = await page1.json();
    assert.strictEqual(body1.total, 25);
    assert.strictEqual(body1.page, 1);
    assert.strictEqual(body1.pageSize, 20);
    assert.strictEqual(body1.items.length, 20);
    assert.strictEqual(body1.items[0].id, 1);
    assert.strictEqual(body1.items[19].id, 20);

    const page2 = await jsonReq(base, cookie, 'GET', '/api/products?page=2');
    const body2 = await page2.json();
    assert.strictEqual(body2.total, 25);
    assert.strictEqual(body2.items.length, 5);
    assert.strictEqual(body2.items[0].id, 21);
  } finally {
    await close(server);
  }
});

test('GET /api/products defaults page to 1 for invalid page values', async () => {
  const { server, base, cookie } = await startApp();
  try {
    await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'P',
      name: 'Product',
      price: 1,
      stock: 1,
    });
    const res = await jsonReq(base, cookie, 'GET', '/api/products?page=abc');
    assert.strictEqual(res.status, 200);
    const body = await res.json();
    assert.strictEqual(body.page, 1);
    assert.strictEqual(body.items.length, 1);
  } finally {
    await close(server);
  }
});

test('GET /api/products search matches name or SKU case-insensitively', async () => {
  const { server, base, cookie } = await startApp();
  try {
    await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'AAA-111',
      name: 'Hammer',
      price: 5,
      stock: 10,
    });
    await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'BBB-222',
      name: 'Wrench',
      price: 7,
      stock: 3,
    });
    await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'CCC-333',
      name: 'Drill',
      price: 20,
      stock: 1,
    });

    const bySku = await jsonReq(base, cookie, 'GET', '/api/products?q=bbb');
    const bySkuBody = await bySku.json();
    assert.strictEqual(bySkuBody.total, 1);
    assert.strictEqual(bySkuBody.items[0].sku, 'BBB-222');

    const byName = await jsonReq(base, cookie, 'GET', '/api/products?q=HAM');
    const byNameBody = await byName.json();
    assert.strictEqual(byNameBody.total, 1);
    assert.strictEqual(byNameBody.items[0].name, 'Hammer');

    const none = await jsonReq(base, cookie, 'GET', '/api/products?q=nothing');
    const noneBody = await none.json();
    assert.strictEqual(noneBody.total, 0);
    assert.deepStrictEqual(noneBody.items, []);
  } finally {
    await close(server);
  }
});

test('GET /api/products/:id returns one product and 404 when missing', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const created = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'GET-1',
      name: 'Single',
      price: 3.5,
      stock: 4,
    });
    const createdBody = await created.json();

    const found = await jsonReq(base, cookie, 'GET', `/api/products/${createdBody.id}`);
    assert.strictEqual(found.status, 200);
    assert.deepStrictEqual(await found.json(), createdBody);

    const missing = await jsonReq(base, cookie, 'GET', '/api/products/99999');
    assert.strictEqual(missing.status, 404);

    const notANumber = await jsonReq(base, cookie, 'GET', '/api/products/abc');
    assert.strictEqual(notANumber.status, 404);
  } finally {
    await close(server);
  }
});

test('PUT /api/products/:id replaces the product fields', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const created = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'UP-1',
      name: 'Old',
      price: 1,
      stock: 1,
    });
    const createdBody = await created.json();

    const updated = await jsonReq(base, cookie, 'PUT', `/api/products/${createdBody.id}`, {
      sku: 'UP-2',
      name: 'New',
      price: 2.5,
      stock: 9,
    });
    assert.strictEqual(updated.status, 200);
    const body = await updated.json();
    assert.strictEqual(body.id, createdBody.id);
    assert.strictEqual(body.sku, 'UP-2');
    assert.strictEqual(body.name, 'New');
    assert.strictEqual(body.price, 2.5);
    assert.strictEqual(body.stock, 9);

    const fetched = await jsonReq(base, cookie, 'GET', `/api/products/${createdBody.id}`);
    assert.deepStrictEqual(await fetched.json(), body);
  } finally {
    await close(server);
  }
});

test('PUT /api/products/:id returns 400 on invalid fields', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const created = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'UP-3',
      name: 'Valid',
      price: 1,
      stock: 1,
    });
    const createdBody = await created.json();

    const res = await jsonReq(base, cookie, 'PUT', `/api/products/${createdBody.id}`, {
      sku: 'UP-3',
      name: 'Valid',
      price: -1,
      stock: 1,
    });
    assert.strictEqual(res.status, 400);
  } finally {
    await close(server);
  }
});

test('PUT /api/products/:id returns 404 when missing', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'PUT', '/api/products/99999', {
      sku: 'X',
      name: 'Y',
      price: 1,
      stock: 1,
    });
    assert.strictEqual(res.status, 404);
  } finally {
    await close(server);
  }
});

test('PUT /api/products/:id returns 409 when SKU conflicts with another product', async () => {
  const { server, base, cookie } = await startApp();
  try {
    await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'A',
      name: 'A',
      price: 1,
      stock: 1,
    });
    const b = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'B',
      name: 'B',
      price: 1,
      stock: 1,
    });
    const bBody = await b.json();

    const res = await jsonReq(base, cookie, 'PUT', `/api/products/${bBody.id}`, {
      sku: 'A',
      name: 'B',
      price: 1,
      stock: 1,
    });
    assert.strictEqual(res.status, 409);

    const sameSku = await jsonReq(base, cookie, 'PUT', `/api/products/${bBody.id}`, {
      sku: 'B',
      name: 'B updated',
      price: 2,
      stock: 2,
    });
    assert.strictEqual(sameSku.status, 200);
  } finally {
    await close(server);
  }
});

test('DELETE /api/products/:id deletes a product and returns 204', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const created = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'DEL-1',
      name: 'Destroyable',
      price: 10,
      stock: 5,
    });
    const body = await created.json();

    const del = await jsonReq(base, cookie, 'DELETE', `/api/products/${body.id}`);
    assert.strictEqual(del.status, 204);

    const fetched = await jsonReq(base, cookie, 'GET', `/api/products/${body.id}`);
    assert.strictEqual(fetched.status, 404);
  } finally {
    await close(server);
  }
});

test('DELETE /api/products/:id returns 404 when missing', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'DELETE', '/api/products/99999');
    assert.strictEqual(res.status, 404);
  } finally {
    await close(server);
  }
});

test('DELETE /api/products/:id returns 409 when product is in an order', async () => {
  const { server, base, cookie, dir } = await startApp();
  try {
    const productRes = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'ORDERED',
      name: 'Ordered Product',
      price: 100,
      stock: 50,
    });
    const product = await productRes.json();

    const dbPath = path.join(dir, 'test.db');
    const db = new DatabaseSync(dbPath);
    try {
      db.prepare("INSERT INTO customers (name, email) VALUES (?, ?)").run('Customer', 'customer@example.com');
      db.prepare("INSERT INTO orders (customer_id, status, total, created_at) VALUES (?, ?, ?, ?)")
        .run(1, 'pending', 100, Date.now());
      db.prepare("INSERT INTO order_items (order_id, product_id, quantity, price) VALUES (?, ?, ?, ?)")
        .run(1, product.id, 2, 100);
    } finally {
      db.close();
    }

    const del = await jsonReq(base, cookie, 'DELETE', `/api/products/${product.id}`);
    assert.strictEqual(del.status, 409);
    const delBody = await del.json();
    assert.strictEqual(typeof delBody.error, 'string');
  } finally {
    await close(server);
  }
});

test('GET /api/products.csv returns valid CSV', async () => {
  const { server, base, cookie } = await startApp();
  try {
    await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'CSV-1',
      name: 'Alpha',
      price: 100,
      stock: 10,
    });
    await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'CSV-2',
      name: 'Beta',
      price: 250,
      stock: 5,
    });

    const res = await fetch(`${base}/api/products.csv`, {
      headers: { cookie },
    });
    assert.strictEqual(res.status, 200);
    assert.strictEqual(res.headers.get('content-type').split(';')[0], 'text/csv');
    const text = await res.text();
    const lines = text.trim().split('\n');
    assert.strictEqual(lines.length, 3);
    assert.strictEqual(lines[0], 'id,sku,name,price,stock');
    const dataLines = lines.slice(1).sort((a, b) => {
      return Number(a.split(',')[0]) - Number(b.split(',')[0]);
    });
    const fields0 = dataLines[0].split(',');
    const fields1 = dataLines[1].split(',');
    assert.strictEqual(fields0[0], '1');
    assert.strictEqual(fields0[1], 'CSV-1');
    assert.strictEqual(fields0[2], 'Alpha');
    assert.strictEqual(fields0[3], '100');
    assert.strictEqual(fields0[4], '10');
    assert.strictEqual(fields1[0], '2');
    assert.strictEqual(fields1[1], 'CSV-2');
    assert.strictEqual(fields1[2], 'Beta');
    assert.strictEqual(fields1[3], '250');
    assert.strictEqual(fields1[4], '5');
  } finally {
    await close(server);
  }
});

test('GET /api/products.csv requires a session', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/api/products.csv`);
    assert.strictEqual(res.status, 401);
  } finally {
    await close(server);
  }
});

test('GET /products serves the products page with table', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await fetch(`${base}/products`, {
      headers: { cookie },
    });
    assert.strictEqual(res.status, 200);
    const html = await res.text();
    assert.ok(html.includes('<main'), 'page must contain a main element');
    assert.ok(html.includes('id="products-body"'), 'page must have products body table');
    assert.ok(html.includes('<table'), 'page must contain a table');
  } finally {
    await close(server);
  }
});

test('page request without session redirects to login', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/products`, { redirect: 'manual' });
    assert.strictEqual(res.status, 302);
    assert.strictEqual(res.headers.get('location'), '/login');
  } finally {
    await close(server);
  }
});

test('POST /api/products returns 201 on success', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'P-1',
      name: 'Tester',
      price: 10,
      stock: 5,
    });
    assert.strictEqual(res.status, 201);
  } finally {
    await close(server);
  }
});

test('POST /api/products returns 409 duplicate SKU', async () => {
  const { server, base, cookie } = await startApp();
  try {
    await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'SKU-100',
      name: 'Widget',
      price: 1,
      stock: 1,
    });
    const dup = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'SKU-100',
      name: 'Other',
      price: 2,
      stock: 2,
    });
    assert.strictEqual(dup.status, 409);
    const body = await dup.json();
    assert.ok(body.error);
  } finally {
    await close(server);
  }
});
