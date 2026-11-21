# Stockroom

A small inventory and order management website for a single shop. One admin user
signs in and manages products, customers and orders through the browser.

## Requirements

- Node.js 22 or later (the app uses the built-in `node:sqlite` module).

## Install

```bash
npm install
```

## Run

```bash
npm start
```

The server listens on the port in the `PORT` environment variable (default `3000`)
and stores its data in the SQLite file named by `DB_PATH` (default `./data.db`).
The database and its tables are created automatically on first start.

| Environment variable | Default      | Purpose                              |
| -------------------- | ------------ | ------------------------------------ |
| `PORT`               | `3000`       | Port the server listens on           |
| `DB_PATH`            | `./data.db`  | SQLite database file                 |
| `ADMIN_PASSWORD`     | `admin123`   | Password for the `admin` user        |

Open `http://localhost:3000` and sign in with user name `admin` and the configured
password (`admin123` by default). The password is stored hashed (scrypt), never in
plain text.

## Test

```bash
npm test
```

The tests use Node's built-in test runner together with Supertest and run against
an in-memory database, so they need no external services.

## Features

- Session-based sign-in for a single `admin` account; every page and API endpoint
  requires a session.
- **Dashboard** (`/`) — product/customer/order counts, revenue (paid + shipped
  orders) and a low-stock list (stock below 5).
- **Products** (`/products`) — searchable, paginated (20 per page) table, add/edit/
  delete, and a CSV export.
- **Customers** (`/customers`) — searchable table, add/edit/delete.
- **Orders** (`/orders`) — create orders from customers and product lines, filter
  by status, and advance/cancel orders through the allowed state transitions.

### Business rules

- Creating an order deducts the ordered quantities from stock; an order that asks
  for more than is in stock is refused and nothing changes.
- Status transitions: `pending → paid` or `cancelled`; `paid → shipped` or
  `cancelled`. Any other change is refused.
- Cancelling an order returns its quantities to stock.
- A product that appears in any order, and a customer with any order, cannot be
  deleted.

## JSON API

All request and response bodies are JSON (except `GET /api/products.csv`, which is
`text/csv`). Errors answer `{ "error": "<message>" }` with the status listed.

| Method & path                     | Description                                  |
| --------------------------------- | -------------------------------------------- |
| `POST /api/login`                 | Sign in (`{username, password}`), sets cookie |
| `POST /api/logout`                | Sign out                                    |
| `GET /api/products?q=&page=`      | Searchable, paginated products (20/page)    |
| `POST /api/products`              | Add a product                               |
| `GET /api/products/:id`           | One product                                 |
| `PUT /api/products/:id`           | Replace a product                           |
| `DELETE /api/products/:id`        | Delete a product (409 if used in an order)  |
| `GET /api/products.csv`           | All products as CSV                         |
| `GET /api/customers?q=`           | Searchable customers                        |
| `POST /api/customers`             | Add a customer                              |
| `GET /api/customers/:id`          | One customer                                |
| `PUT /api/customers/:id`          | Replace a customer                          |
| `DELETE /api/customers/:id`       | Delete a customer (409 if they have orders) |
| `GET /api/orders?status=`         | Orders, newest first, with `customerName`   |
| `POST /api/orders`                | Create a `pending` order                    |
| `GET /api/orders/:id`             | One order with its items                    |
| `PATCH /api/orders/:id`           | Change the status                           |
| `GET /api/dashboard`              | Dashboard figures                           |

## Project layout

- `server.js` — entry point: reads environment, opens the database, starts the app.
- `app.js` — Express app, routes, and authentication guard.
- `db.js` — SQLite schema and data-access functions.
- `auth.js` — password hashing/verification.
- `validation.js` — request field validation.
- `views.js` — server-rendered HTML pages.
- `public/style.css` — stylesheet.
- `test/app.test.js` — automated tests.
