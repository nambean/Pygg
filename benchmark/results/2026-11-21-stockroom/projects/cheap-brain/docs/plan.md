# Stockroom — Design Plan

A small inventory and order management website for a single shop. One admin
user signs in through the browser and manages products, customers and orders.

---

## 1. Overview

Stockroom is a self-contained Node.js web application:

- Serves its own pages (server-rendered HTML) and its own JSON API on the same
  HTTP server.
- Persists data in a single SQLite database using Node's built-in
  `node:sqlite` module (no native build step, no external service).
- Uses a small number of npm packages (a web framework and helpers). Nothing
  requires network access at run time.
- `npm install` then `npm start` starts the server on `PORT` (default `3000`).

### Technology choices

| Concern | Choice | Rationale |
| --- | --- | --- |
| Runtime | Node.js 22+ | Required; ships `node:sqlite` and `node:crypto` |
| HTTP server / routing | Express 4 | Small, well-known; keeps routing/JSON parsing concise. Could be replaced by `node:http` but Express keeps the code simpler. |
| Database | `node:sqlite` (`DatabaseSync`) | Required; synchronous API fits a single-process, low-traffic app. |
| Sessions | Random-token cookie backed by a `sessions` table | No persistent signing secret needed; survives restarts; trivially revocable. |
| Password hashing | `crypto.scrypt` (built-in) | Strong KDF with no dependency; store `salt:hash`. |
| Money | Integer cents | Avoids floating-point rounding in prices, line prices, totals and revenue. |
| Tests | `node:test` + built-in `assert` (or `node --test`) | No extra dependency; runs with `npm test`. |

### Configuration (environment variables)

| Variable | Default | Purpose |
| --- | --- | --- |
| `PORT` | `3000` | Port the HTTP server listens on |
| `DB_PATH` | `./data.db` | SQLite database file |
| `ADMIN_PASSWORD` | `admin123` | Password for the single `admin` account |

---

## 2. Project structure

```
server.js            # entrypoint: load config, open DB, start HTTP server
package.json         # scripts: start, test
README.md            # how to run the app and the tests
src/
  config.js          # reads env vars, exports { port, dbPath, adminPassword }
  db.js              # opens DB, runs migrations, seeds admin
  auth.js            # scrypt hash/verify, session create/destroy, middleware
  validate.js        # shared field validators (products, customers, orders)
  routes/
    pages.js         # GET page handlers -> server-rendered HTML
    api.js           # /api/* JSON handlers
  views/             # small HTML template helpers (or template strings)
  public/
    styles.css       # mobile-first CSS (usable at 375px)
    app.js           # progressive enhancement: confirm dialogs, order line rows
tests/
  *.test.js          # node:test suites (API-level, exercised over HTTP)
```

> HTML is server-rendered with a tiny template helper (string interpolation /
> a minimal `renderPage(title, nav, body)` function). No client-side framework.

---

## 3. Data schema

All tables are created on startup if they do not exist (idempotent `CREATE TABLE
IF NOT EXISTS`). IDs are `INTEGER PRIMARY KEY` (auto-incrementing). Foreign keys
are enabled per connection (`PRAGMA foreign_keys = ON`).

Money is stored as **integer cents**. Prices, line prices, order totals and
revenue are all cents in the database; the JSON API and pages convert to
dollars (a plain number) at the boundary.

### `users`

The single admin account. Seeded on startup from `ADMIN_PASSWORD`.

| Column | Type | Constraints |
| --- | --- | --- |
| `id` | INTEGER | PRIMARY KEY |
| `username` | TEXT | NOT NULL, UNIQUE |
| `password_hash` | TEXT | NOT NULL (format `salt:hash` hex) |

Seed: `INSERT OR IGNORE ... ('admin', hash(ADMIN_PASSWORD))`. If the row already
exists it is left unchanged, so a changed `ADMIN_PASSWORD` does not silently
reset an existing password mid-life.

### `sessions`

| Column | Type | Constraints |
| --- | --- | --- |
| `token` | TEXT | PRIMARY KEY (32 random bytes, base64url) |
| `user_id` | INTEGER | NOT NULL, REFERENCES users(id) |
| `created_at` | INTEGER | NOT NULL (epoch seconds) |

### `products`

| Column | Type | Constraints |
| --- | --- | --- |
| `id` | INTEGER | PRIMARY KEY |
| `sku` | TEXT | NOT NULL, UNIQUE |
| `name` | TEXT | NOT NULL |
| `price` | INTEGER | NOT NULL, `price >= 0` (cents) |
| `stock` | INTEGER | NOT NULL, `stock >= 0` |

### `customers`

| Column | Type | Constraints |
| --- | --- | --- |
| `id` | INTEGER | PRIMARY KEY |
| `name` | TEXT | NOT NULL |
| `email` | TEXT | NOT NULL, UNIQUE |
| `phone` | TEXT | NULL allowed |

### `orders`

| Column | Type | Constraints |
| --- | --- | --- |
| `id` | INTEGER | PRIMARY KEY |
| `customer_id` | INTEGER | NOT NULL, REFERENCES customers(id) |
| `status` | TEXT | NOT NULL, one of `pending`, `paid`, `shipped`, `cancelled` |
| `total` | INTEGER | NOT NULL, `total >= 0` (cents, sum of line price × quantity) |
| `created_at` | INTEGER | NOT NULL (epoch milliseconds, for ordering) |

Index on `created_at` (DESC) to serve the newest-first listing.

### `order_items`

One row per order line. The unit price is copied at creation time (snapshot).

| Column | Type | Constraints |
| --- | --- | --- |
| `id` | INTEGER | PRIMARY KEY |
| `order_id` | INTEGER | NOT NULL, REFERENCES orders(id) |
| `product_id` | INTEGER | NOT NULL, REFERENCES products(id) |
| `quantity` | INTEGER | NOT NULL, `quantity >= 1` |
| `price` | INTEGER | NOT NULL, `price >= 0` (cents, price at order time) |

Index on `order_id`.

---

## 4. Auth and sessions

### Password hashing

- `hashPassword(password)` -> `scrypt(password, randomSalt)` encoded as
  `hex(salt):hex(key)`.
- `verifyPassword(password, stored)` -> constant-time compare of the scrypt
  output.

### Session lifecycle

- `POST /api/login` verifies credentials; on success it creates a session row,
  generates a random `token` (`crypto.randomBytes(32).toString('base64url')`),
  and sets an `HttpOnly`, `SameSite=Lax` cookie named `sid`.
- `POST /api/logout` deletes the session row and clears the cookie.
- Every request resolves the session by reading the `sid` cookie and looking it
  up in `sessions`.

### Access control (middleware)

A single `requireAuth` middleware runs for **all** routes except:

- `GET /login` (the sign-in page)
- `POST /api/login`

Behaviour when there is no valid session:

- **Page request** (`text/html` route): redirect (`302`) to `/login`.
- **`/api/*` request**: respond `401` with `{"error": "..."}`.

The navigation and every protected page include a **Sign out** button that
`POST`s to `/api/logout`.

---

## 5. Pages

Every protected page shares the same header navigation:

**Dashboard · Products · Customers · Orders** (with the active page marked),
plus a **Sign out** button. Forms have visible `<label>` elements; CSS is
mobile-first so pages fit a 375 px viewport without horizontal scrolling.

### `/login` (public)

- Form with labelled fields **Username** and **Password** and a **Sign in**
  button.
- Submits to `POST /api/login`. On `401` it re-renders the page with an inline
  error ("Wrong username or password").
- On success the browser is redirected to `/`.

### `/` — Dashboard

Fetches/renders the `GET /api/dashboard` figures:

- Counts: products, customers, orders.
- Revenue: total of `paid` and `shipped` orders (sum of `total`).
- Low-stock list: products with `stock < 5` (id, sku, name, stock).

### `/products`

- Table of products (sku, name, price, stock), **20 per page**, with
  **Previous** / **Next** links driven by `?page=`.
- Search box matching name or SKU (case-insensitive), driven by `?q=`.
- "Add product" form: fields **SKU**, **Name**, **Price**, **Stock**, button
  **Add product**.
- Per-row **Edit** (loads an edit form) and **Delete** button (JavaScript
  `confirm()` before submitting).
- **Download CSV** link to `/api/products.csv`.
- Inline error messages for `400` (invalid fields) and `409` (duplicate SKU).

### `/customers`

- Table of customers (name, email, phone).
- "Add customer" form: **Name**, **Email**, **Phone**, button **Add customer**.
- Per-row Edit and Delete.
- Inline errors for `400` (invalid fields, e.g. bad email) and `409`
  (duplicate email, or delete when the customer has orders).

### `/orders`

- Table of orders (customer name, status, total, date), **newest first**,
  filterable by status via a dropdown / query param.
- "Create order" form: choose a customer (dropdown), add one or more product
  lines (product dropdown + quantity), button **Create order**.
- Per-order buttons to move to the **allowed next statuses** only:
  - `pending` → **Paid** or **Cancel**
  - `paid` → **Shipped** or **Cancel**
- Inline errors for `400` (invalid body/unknown ids), `409` (too little stock,
  refused status change).

---

## 6. API routes

All request/response bodies are JSON unless noted. Every `/api/*` route (except
`POST /api/login`) requires a session, else `401`.

Errors answer `{"error": "<message>"}` with the listed status.

### Auth

| Method & path | Body | Result |
| --- | --- | --- |
| `POST /api/login` | `{username, password}` | `200 {"ok": true}` + `sid` cookie; `401` on wrong user/password |
| `POST /api/logout` | — | `200 {"ok": true}`, clears session |

### Products

| Method & path | Body | Result |
| --- | --- | --- |
| `GET /api/products?q=&page=` | — | `200 {"items":[Product], "total":n, "page":p, "pageSize":20}`; match name or SKU case-insensitive; ordered by `id` |
| `POST /api/products` | `{sku, name, price, stock}` | `201` with product; `400` invalid fields; `409` duplicate SKU |
| `GET /api/products/:id` | — | `200` product; `404` |
| `PUT /api/products/:id` | `{sku, name, price, stock}` | `200` product; `400` invalid; `404`; `409` duplicate SKU |
| `DELETE /api/products/:id` | — | `204`; `404`; `409` when it appears in any order |
| `GET /api/products.csv` | — | `200`, `text/csv`, header `id,sku,name,price,stock` |

Product JSON shape: `{id, sku, name, price, stock}` where `price` is dollars
(a number; cents in DB).

### Customers

| Method & path | Body | Result |
| --- | --- | --- |
| `GET /api/customers?q=` | — | `200 {"items":[Customer], "total":n}`; match name or email |
| `POST /api/customers` | `{name, email, phone}` | `201`; `400` invalid (e.g. bad email); `409` duplicate email |
| `GET /api/customers/:id` | — | `200`; `404` |
| `PUT /api/customers/:id` | `{name, email, phone}` | `200`; `400`; `404`; `409` duplicate email |
| `DELETE /api/customers/:id` | — | `204`; `404`; `409` when the customer has an order |

Customer JSON shape: `{id, name, email, phone}` (`phone` may be empty/null).

### Orders

| Method & path | Body | Result |
| --- | --- | --- |
| `GET /api/orders?status=` | — | `200 {"items":[Order], "total":n}`; newest first; each item includes `customerName` |
| `POST /api/orders` | `{customerId, items:[{productId, quantity}]}` | `201` order; `400` invalid body / unknown customer / unknown product; `409` too little stock |
| `GET /api/orders/:id` | — | `200` order with items; `404` |
| `PATCH /api/orders/:id` | `{status}` | `200` order; `400` unknown status; `404`; `409` refused transition |

Order JSON shape: `{id, customerId, customerName, status, total, createdAt,
items:[{productId, quantity, price}]}`. `total` is dollars.

### Dashboard

| Method & path | Result |
| --- | --- |
| `GET /api/dashboard` | `200 {"products":n, "customers":n, "orders":n, "revenue":x, "lowStock":[{id,sku,name,stock}]}` |

`revenue` is dollars (sum of `total` for `paid` + `shipped` orders).

---

## 7. Business rules (enforced in a transaction)

### Order creation

1. Validate body: `customerId` is a known customer; `items` is a non-empty
   array; every `productId` known; every `quantity` an integer `>= 1`; no
   duplicate `productId` within one order.
2. In a single SQLite transaction:
   - For each line, read the product's current `stock` and `price` (price is
     snapshotted here).
   - If any line asks for more than `stock`, abort with `409` and **change
     nothing** (rollback).
   - Otherwise decrement each product's `stock`, insert the `orders` row with
     `status='pending'` and `total` = Σ(price × quantity), and insert each
     `order_items` row.
3. Return the created order with items including the snapshot prices.

### Status transitions

Allowed transitions only:

```
pending  -> paid | cancelled
paid     -> shipped | cancelled
```

- `PATCH /api/orders/:id {status}`: unknown status -> `400`; unknown order ->
  `404`; any other transition -> `409`.
- **Cancelling** an order restores its quantities to stock (in the same
  transaction): for each `order_items` line, `products.stock += quantity`.

### Deletion guards

- `DELETE /api/products/:id`: if the product has any `order_items` row -> `409`;
  otherwise delete -> `204`.
- `DELETE /api/customers/:id`: if the customer has any `orders` row -> `409`;
  otherwise delete -> `204`.

---

## 8. Validation rules (shared `validate.js`)

- **sku**: non-empty string, unique.
- **name** (product/customer): non-empty string.
- **price**: a number `>= 0`; converted to cents on write.
- **stock**: an integer `>= 0`.
- **email**: non-empty, matches a basic email pattern, unique.
- **phone**: optional; string, trimmed.
- **quantity**: integer `>= 1`.
- **status**: one of the four allowed values.

Invalid fields produce `400 {"error": "<plain-language message>"}`.

---

## 9. Testing strategy

Tests run with `npm test` (using `node --test`). The suite exercises the
application over HTTP (start the server on an ephemeral port with a temp
`DB_PATH` and a known `ADMIN_PASSWORD`), so coverage includes auth, schema and
routing end-to-end.

Planned test areas:

- **Auth**: login success/failure, logout, `401` for API and `302` redirect for
  pages without a session.
- **Products**: CRUD, duplicate SKU `409`, validation `400`, pagination, search,
  CSV export, delete guard when referenced by an order.
- **Customers**: CRUD, duplicate email `409`, email validation, delete guard.
- **Orders**: creation decrements stock, too-little-stock `409` leaves state
  unchanged, status transitions (valid, invalid `409`, unknown `400`),
  cancellation restores stock, order totals and snapshot prices are correct.
- **Dashboard**: counts, revenue (paid+shipped only), low-stock list.

---

## 10. Acceptance checklist

- [ ] `npm install && npm start` serves the app on `PORT` (default 3000).
- [ ] Tables are created on start and admin is seeded from `ADMIN_PASSWORD`.
- [ ] `admin`/`admin123` (or `ADMIN_PASSWORD`) signs in to the dashboard.
- [ ] All pages and `/api/*` require a session (except `/login` and
      `POST /api/login`).
- [ ] All five pages and the full JSON API behave as specified, with inline
      error messages for duplicates, stock shortfalls and refused status
      changes.
- [ ] Pages fit a 375 px viewport without horizontal scrolling; all form
      fields have visible labels.
- [ ] `npm test` passes.
- [ ] `README.md` explains how to run the app and tests.
