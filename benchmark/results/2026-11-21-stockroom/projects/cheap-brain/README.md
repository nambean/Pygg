# Stockroom

A small inventory and order management website for a single shop. One admin
user signs in through the browser and manages products, customers and orders.

## Requirements

- Node.js 22 or later

## Running the app

```sh
npm install
npm start
```

The server listens on the port in the `PORT` environment variable (default
`3000`). Open <http://localhost:3000> and sign in with user name `admin` and
the password from `ADMIN_PASSWORD` (default `admin123`).

### Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `PORT` | `3000` | Port the HTTP server listens on |
| `DB_PATH` | `./data.db` | SQLite database file |
| `ADMIN_PASSWORD` | `admin123` | Password for the `admin` user |

The SQLite database is created on first start if it does not exist.

## Running the tests

```sh
npm test
```

The tests start the server on an ephemeral port against a temporary database
and exercise the app over HTTP (sign-in page, auth redirects and login).

## Status transition rules

Orders move through four statuses (`pending`, `paid`, `shipped`, `cancelled`).
Allowed transitions:

| From     | To         | Effect                          |
| -------- | ---------- | ------------------------------- |
| `pending` | `paid`     | Marks order as paid             |
| `pending` | `cancelled` | Restores stock quantities       |
| `paid`    | `shipped`  | Marks order as shipped           |
| `paid`    | `cancelled` | Restores stock quantities       |

Any other transition is refused. Revenue counts only orders with
status `paid` or `shipped`.
