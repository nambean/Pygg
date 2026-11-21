# Stockroom

A small inventory and order management website for a single shop admin. It manages products, customers and orders, and is served entirely by one Node.js process backed by SQLite.

## Requirements

- Node.js 22.13 or later (uses the built-in `node:sqlite` module).

## Install and run

```bash
npm install
npm start
```

The server listens on the port in the `PORT` environment variable (default `3000`). Open `http://localhost:3000` and sign in.

### Environment variables

| Variable          | Default      | Purpose                                  |
| ----------------- | ------------ | ---------------------------------------- |
| `PORT`            | `3000`       | Port the server listens on               |
| `DB_PATH`         | `./data.db`  | SQLite database file                     |
| `ADMIN_PASSWORD`  | `admin123`   | Password for the `admin` account         |
| `SESSION_SECRET`  | built-in     | Secret used to sign session cookies      |

The single admin user name is `admin`. The password is stored hashed in the database.

## Tests

```bash
npm test
```

The tests use Node's built-in test runner and exercise the sign-in flow, the JSON API, and the business rules (stock deduction, status transitions, deletion guards, CSV export and the dashboard figures).

## Features

- Products: add, edit, delete, search, paginate (20 per page) and export to CSV.
- Customers: add, edit, delete and search.
- Orders: create from existing customers and products, filter by status, and move through the allowed statuses.
- Dashboard: counts, revenue from paid/shipped orders, and a low-stock list.
