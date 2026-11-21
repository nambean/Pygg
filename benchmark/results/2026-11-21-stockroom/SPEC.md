# Stockroom: a small inventory and order management website

Build a web application for a small shop to manage its products, customers and orders. One admin user signs in and does everything through the browser. Build it in this repository, from an empty start.

## Technology

- Node.js 22 or later. `npm install` then `npm start` must start the server; it listens on the port in the `PORT` environment variable (default 3000).
- Store data in SQLite with Node's built-in `node:sqlite` module, in the file named by the `DB_PATH` environment variable (default `./data.db`). Create the tables on start if they do not exist.
- You may use a small number of npm packages (for example a web framework). No external services, no build step that needs network access at run time.
- The pages are served by the same server. Plain HTML, CSS and JavaScript are fine; so is server-side rendering.
- Include automated tests that run with `npm test`, and a `README.md` saying how to run the app and the tests.

## Sign-in

- One admin account: user name `admin`, password from the `ADMIN_PASSWORD` environment variable (default `admin123`). Store the password hashed.
- Every page and every `/api/*` endpoint needs a signed-in session, except the sign-in page and `POST /api/login`. An API request without a session gets `401`; a page request without one is redirected to `/login`.
- `/login` has a form with fields labelled **Username** and **Password** and a button **Sign in**. A wrong password shows an error message on the page. Signing in goes to the dashboard. Every other page has a **Sign out** button.

## Data

- **Product:** `id`, `sku` (text, required, unique), `name` (required), `price` (number, at least 0), `stock` (whole number, at least 0).
- **Customer:** `id`, `name` (required), `email` (required, a valid address, unique), `phone` (optional).
- **Order:** `id`, `customerId`, `status` (`pending`, `paid`, `shipped` or `cancelled`), `items` (one or more lines of `productId` and `quantity`, a whole number of at least 1, each line with the product's price at the time of the order), `total` (the sum of price × quantity), `createdAt`.

## Rules

- Creating an order takes the quantities out of stock. An order that asks for more of a product than is in stock is refused, and nothing changes.
- Status changes allowed: `pending` → `paid` or `cancelled`; `paid` → `shipped` or `cancelled`. Anything else is refused. Cancelling an order puts its quantities back into stock.
- A product that appears in any order cannot be deleted; a customer with any order cannot be deleted.

## Pages

- **Dashboard** (`/`): the number of products, customers and orders; revenue (the total of paid and shipped orders); and a list of products with fewer than 5 in stock.
- **Products** (`/products`): a table of products, 20 per page with next and previous page links, a search box that matches the name or the SKU, a form to add a product (fields labelled **SKU**, **Name**, **Price**, **Stock**; button **Add product**), and for each product a way to edit it and a **Delete** button that asks for confirmation. A **Download CSV** link exports every product.
- **Customers** (`/customers`): a table of customers, a form to add one (fields **Name**, **Email**, **Phone**; button **Add customer**), edit and delete.
- **Orders** (`/orders`): a table of orders with customer name, status, total and date, newest first, filterable by status; a form to create an order (choose a customer, add one or more product lines with a quantity; button **Create order**); and buttons to move an order to its allowed next statuses.
- Every page has the same navigation (Dashboard, Products, Customers, Orders). Errors from the server (a duplicate SKU, too little stock, a refused status change) are shown to the user in plain words next to the form or action. The pages are usable on a phone (375 px wide) without scrolling sideways, and every form field has a visible label.

## JSON API

All bodies are JSON. Errors answer `{"error": "<message>"}` with the status given.

| Method and path | Does | Answers |
| --- | --- | --- |
| `POST /api/login` `{username, password}` | Signs in, sets a session cookie | `200 {"ok": true}`; `401` on a wrong user name or password |
| `POST /api/logout` | Signs out | `200 {"ok": true}` |
| `GET /api/products?q=&page=` | Products matching `q` (name or SKU, case-insensitive), 20 per page, by `id` | `200 {"items": [...], "total": n, "page": p, "pageSize": 20}` |
| `POST /api/products` | Adds a product | `201` with the product; `400` on invalid fields; `409` on a duplicate SKU |
| `GET /api/products/:id` | One product | `200`; `404` |
| `PUT /api/products/:id` | Replaces its `sku`, `name`, `price`, `stock` | `200` with the product; `400`; `404`; `409` on a duplicate SKU |
| `DELETE /api/products/:id` | Deletes it | `204`; `404`; `409` when it appears in an order |
| `GET /api/products.csv` | Every product as CSV with the header `id,sku,name,price,stock` | `200`, `text/csv` |
| `GET /api/customers?q=` | Customers matching `q` (name or email) | `200 {"items": [...], "total": n}` |
| `POST /api/customers` | Adds a customer | `201`; `400`; `409` on a duplicate email |
| `GET`, `PUT`, `DELETE /api/customers/:id` | As for products | `DELETE` is `409` when the customer has an order |
| `GET /api/orders?status=` | Orders, newest first, each with `customerName` | `200 {"items": [...], "total": n}` |
| `POST /api/orders` `{customerId, items: [{productId, quantity}]}` | Creates a `pending` order | `201` with the order (`id`, `customerId`, `status`, `total`, `items` with `productId`, `quantity`, `price`); `400` on an invalid body or an unknown customer or product; `409` when a product has too little stock |
| `GET /api/orders/:id` | One order with its items | `200`; `404` |
| `PATCH /api/orders/:id` `{status}` | Changes the status | `200` with the order; `400` on an unknown status; `404`; `409` on a change the rules refuse |
| `GET /api/dashboard` | The dashboard's figures | `200 {"products": n, "customers": n, "orders": n, "revenue": x, "lowStock": [{"id", "sku", "name", "stock"}]}` |
