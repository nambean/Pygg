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
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stockroom-customers-'));
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

test('GET /api/customers requires a session', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/api/customers`);
    assert.strictEqual(res.status, 401);
  } finally {
    await close(server);
  }
});

test('POST /api/customers creates a customer and returns 201', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Alice Smith',
      email: 'alice@example.com',
      phone: '555-0100',
    });
    assert.strictEqual(res.status, 201);
    const body = await res.json();
    assert.strictEqual(body.name, 'Alice Smith');
    assert.strictEqual(body.email, 'alice@example.com');
    assert.strictEqual(body.phone, '555-0100');
    assert.strictEqual(typeof body.id, 'number');
  } finally {
    await close(server);
  }
});

test('POST /api/customers omits phone when not provided', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Bob Jones',
      email: 'bob@example.com',
    });
    assert.strictEqual(res.status, 201);
    const body = await res.json();
    assert.strictEqual(body.name, 'Bob Jones');
    assert.strictEqual(body.email, 'bob@example.com');
    assert.strictEqual(body.phone, null);
  } finally {
    await close(server);
  }
});

test('POST /api/customers trims whitespace on name, email and phone', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: '  Carol White  ',
      email: '  carol@example.com  ',
      phone: '  555-0200  ',
    });
    assert.strictEqual(res.status, 201);
    const body = await res.json();
    assert.strictEqual(body.name, 'Carol White');
    assert.strictEqual(body.email, 'carol@example.com');
    assert.strictEqual(body.phone, '555-0200');
  } finally {
    await close(server);
  }
});

test('POST /api/customers rejects missing name with 400', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'POST', '/api/customers', {
      email: 'test@example.com',
    });
    assert.strictEqual(res.status, 400);
    const body = await res.json();
    assert.strictEqual(typeof body.error, 'string');
  } finally {
    await close(server);
  }
});

test('POST /api/customers rejects missing email with 400', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Test User',
    });
    assert.strictEqual(res.status, 400);
    const body = await res.json();
    assert.strictEqual(typeof body.error, 'string');
  } finally {
    await close(server);
  }
});

test('POST /api/customers rejects invalid email with 400', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Test User',
      email: 'not-an-email',
    });
    assert.strictEqual(res.status, 400);
    const body = await res.json();
    assert.strictEqual(typeof body.error, 'string');
  } finally {
    await close(server);
  }
});

test('POST /api/customers returns 409 on duplicate email', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const first = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'First',
      email: 'dup@example.com',
    });
    assert.strictEqual(first.status, 201);

    const second = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Second',
      email: 'dup@example.com',
    });
    assert.strictEqual(second.status, 409);
    const body = await second.json();
    assert.strictEqual(typeof body.error, 'string');
  } finally {
    await close(server);
  }
});

test('GET /api/customers lists customers with total count', async () => {
  const { server, base, cookie } = await startApp();
  try {
    await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Customer A',
      email: 'a@test.com',
      phone: '111',
    });
    await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Customer B',
      email: 'b@test.com',
    });
    await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Customer C',
      email: 'c@test.com',
      phone: '333',
    });

    const res = await jsonReq(base, cookie, 'GET', '/api/customers');
    assert.strictEqual(res.status, 200);
    const body = await res.json();
    assert.strictEqual(body.total, 3);
    assert.strictEqual(body.items.length, 3);
    assert.strictEqual(body.items[0].name, 'Customer A');
    assert.strictEqual(body.items[2].name, 'Customer C');
  } finally {
    await close(server);
  }
});

test('GET /api/customers?q filters by name and email', async () => {
  const { server, base, cookie } = await startApp();
  try {
    await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Charlie Brown',
      email: 'charlie@test.com',
    });
    await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Diana Prince',
      email: 'diana@test.com',
    });

    const byName = await jsonReq(base, cookie, 'GET', '/api/customers?q=charlie');
    const byNameBody = await byName.json();
    assert.strictEqual(byNameBody.total, 1);
    assert.strictEqual(byNameBody.items[0].name, 'Charlie Brown');

    const byEmail = await jsonReq(base, cookie, 'GET', '/api/customers?q=diana');
    const byEmailBody = await byEmail.json();
    assert.strictEqual(byEmailBody.total, 1);
    assert.strictEqual(byEmailBody.items[0].email, 'diana@test.com');

    const none = await jsonReq(base, cookie, 'GET', '/api/customers?q=nothing');
    const noneBody = await none.json();
    assert.strictEqual(noneBody.total, 0);
    assert.deepStrictEqual(noneBody.items, []);
  } finally {
    await close(server);
  }
});

test('GET /api/customers/:id returns a customer and 404 when missing', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const created = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Get Me',
      email: 'getme@example.com',
    });
    const createdBody = await created.json();

    const found = await jsonReq(base, cookie, 'GET', `/api/customers/${createdBody.id}`);
    assert.strictEqual(found.status, 200);
    const foundBody = await found.json();
    assert.deepStrictEqual(foundBody, createdBody);

    const missing = await jsonReq(base, cookie, 'GET', '/api/customers/99999');
    assert.strictEqual(missing.status, 404);
  } finally {
    await close(server);
  }
});

test('PUT /api/customers/:id replaces the customer fields', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const created = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Old Name',
      email: 'old@example.com',
      phone: '000',
    });
    const createdBody = await created.json();

    const updated = await jsonReq(base, cookie, 'PUT', `/api/customers/${createdBody.id}`, {
      name: 'New Name',
      email: 'new@example.com',
      phone: '999',
    });
    assert.strictEqual(updated.status, 200);
    const body = await updated.json();
    assert.strictEqual(body.id, createdBody.id);
    assert.strictEqual(body.name, 'New Name');
    assert.strictEqual(body.email, 'new@example.com');
    assert.strictEqual(body.phone, '999');

    // Verify persistence
    const fetched = await jsonReq(base, cookie, 'GET', `/api/customers/${createdBody.id}`);
    assert.deepStrictEqual(await fetched.json(), body);
  } finally {
    await close(server);
  }
});

test('PUT /api/customers/:id returns 400 on validation failure', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const created = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Valid',
      email: 'valid@example.com',
    });
    const createdBody = await created.json();

    const res = await jsonReq(base, cookie, 'PUT', `/api/customers/${createdBody.id}`, {
      email: 'not-an-email',
    });
    assert.strictEqual(res.status, 400);
  } finally {
    await close(server);
  }
});

test('PUT /api/customers/:id returns 404 when missing', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'PUT', '/api/customers/99999', {
      name: 'Nobody',
      email: 'nobody@example.com',
    });
    assert.strictEqual(res.status, 404);
  } finally {
    await close(server);
  }
});

test('PUT /api/customers/:id returns 409 when email conflicts with another customer', async () => {
  const { server, base, cookie } = await startApp();
  try {
    await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'A',
      email: 'conflict@example.com',
    });
    const b = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'B',
      email: 'b@example.com',
    });
    const bBody = await b.json();

    const res = await jsonReq(base, cookie, 'PUT', `/api/customers/${bBody.id}`, {
      name: 'B Updated',
      email: 'conflict@example.com',
    });
    assert.strictEqual(res.status, 409);

    // Same email update should still work
    const same = await jsonReq(base, cookie, 'PUT', `/api/customers/${bBody.id}`, {
      name: 'B Renamed',
      email: 'b@example.com',
    });
    assert.strictEqual(same.status, 200);
  } finally {
    await close(server);
  }
});

test('DELETE /api/customers/:id deletes a customer and returns 204', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const created = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Delete Me',
      email: 'delete@example.com',
    });
    const body = await created.json();

    const del = await jsonReq(base, cookie, 'DELETE', `/api/customers/${body.id}`);
    assert.strictEqual(del.status, 204);

    const fetched = await jsonReq(base, cookie, 'GET', `/api/customers/${body.id}`);
    assert.strictEqual(fetched.status, 404);
  } finally {
    await close(server);
  }
});

test('DELETE /api/customers/:id returns 404 when missing', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'DELETE', '/api/customers/99999');
    assert.strictEqual(res.status, 404);
  } finally {
    await close(server);
  }
});

test('DELETE /api/customers/:id returns 409 when customer has orders', async () => {
  const { server, base, cookie, dir } = await startApp();
  try {
    const customerRes = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Ordered Customer',
      email: 'ordered@example.com',
    });
    const customer = await customerRes.json();

    // Create an order for this customer to link them
    const dbPath = path.join(dir, 'test.db');
    const db = new DatabaseSync(dbPath);
    try {
      db.prepare("INSERT INTO products (sku, name, price, stock) VALUES (?, ?, ?, ?)")
        .run('PRO-1', 'Product', 100, 50);
      db.prepare("INSERT INTO orders (customer_id, status, total, created_at) VALUES (?, ?, ?, ?)")
        .run(customer.id, 'pending', 100, Date.now());
      db.prepare("INSERT INTO order_items (order_id, product_id, quantity, price) VALUES (?, ?, ?, ?)")
        .run(1, 1, 1, 100);
    } finally {
      db.close();
    }

    const del = await jsonReq(base, cookie, 'DELETE', `/api/customers/${customer.id}`);
    assert.strictEqual(del.status, 409);
    const delBody = await del.json();
    assert.strictEqual(typeof delBody.error, 'string');
  } finally {
    await close(server);
  }
});
