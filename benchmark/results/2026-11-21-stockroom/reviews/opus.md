# Blind review: Stockroom (Projects X, Y and Z)

Reviewer: Claude Opus 5.5, as a fresh agent that saw only the blind folder (the specification, each project's source and screenshots, and the review prompt). Returned as written.

| Criterion | X | Y | Z |
| --- | --- | --- | --- |
| Spec coverage | 9 | 1 | 6 |
| Correctness | 8 | 1 | 7 |
| Code quality | 6 | 1 | 6 |
| Security | 6 | 1 | 8 |
| Tests | 7 | 1 | 7 |
| UI and UX | 7 | 1 | 4 |
| Accessibility | 6 | 1 | 6 |
| Documentation | 8 | 1 | 5 |
| **Overall** | **8** | **1** | **5** |

---

## Project X

**Spec coverage (9).** Every API row in the spec is implemented in `src/app.js:733-907`. That includes `401` versus redirect in `requireAuth` (726-730), CSV with the exact header (759-768), `{items,total,page,pageSize}` (770-776), and `customerName` from a LEFT JOIN (`db.js:199-208`). All pages are there: dashboard with counts, revenue and low stock (220-252); products with 20 per page, Previous/Next links, search, CSV link and edit page (254-399); customers with add, edit, delete and search; orders with filter, multi-line create form and next-status buttons (532-699). The screenshots confirm each of these (`products-1440.png` shows "Page 1 of 2 Next"). One small gap: delete errors appear in `alert()` (346, 479), not next to the action.

**Correctness (8).** Order creation runs in `BEGIN/COMMIT/ROLLBACK`. Inside it, stock is checked again and taken one line at a time (`db.js:243-271`), so two lines for the same product cannot together take more than is in stock. Line prices are captured and the total is rounded (232-239). The transition table and refusal are correct (275-286). Deletes are refused when the record is in use (`app.js:808, 851`). Validation rejects a negative price, a fractional stock and a bad email (14-35). One flaw: cancelling puts stock back and updates the status with no transaction (`db.js:287-295`).

**Code quality (6).** The data layer (`db.js` Store class) is cleanly separated. But `app.js` is a 951-line file that mixes CSS, HTML templates, inline client scripts, validation and routes. The dashboard count queries are duplicated (221-223 and 901-903), and pages reach into `store.db` directly (534-535). Error handling is decent, including a JSON error handler for `/api` (943-948).

**Security (6).** Passwords use scrypt with a salt and `timingSafeEqual` (`db.js:4-17`). All SQL is parameterized. Output is escaped on the server (`escapeHtml`, 61-68) and in the client (644-647), and embedded JSON escapes `<` (585). Every route after `/api/login` is guarded (751). The weak points are sessions. `cookie-session` falls back to a hard-coded secret `'stockroom-secret'` (706), so anyone who knows the default can forge a signed cookie. Sessions are stateless, so signing out cannot revoke a copied cookie. There is no `secure` flag (714-721).

**Tests (7).** `test/app.test.js` has 8 HTTP-level tests with real assertions on the main rules. They cover auth and redirect (49-77), CRUD, duplicates, pagination with 22 items, search and CSV (79-152), stock taken and nothing changed on a refused order (207-218), transitions including 409 and 400 (232-245), delete guards and restock on cancel (259-287), and dashboard revenue (289-311). There are no UI or edge tests, such as duplicate lines or a price rounding case. They look like they pass.

**UI and UX (7).** The screenshots show a clean, consistent layout: a dark header with nav, form cards, tables at full width, and colour-coded action buttons ("Mark paid", "Cancel"). Errors appear in an inline red box (`.error`, 112). At 375 px nothing scrolls sideways, but the table headers break mid-word ("PRIC E", "CUST OMER" in `products-375.png` and `orders-375.png`) because of `word-break: break-word` (115). The status shows as "Pending" or "Paid", but the table has no order id.

**Accessibility (6).** Static fields have `<label for>` (188-191, 289-292). The page has `lang="en"` and real `<table>`/`<thead>`. In the order lines built by JavaScript, the `<label>Product</label>` and `<label>Quantity</label>` are not tied to their controls (637-640), and "Items" is a label with no control (599). There is no `aria-current` in the nav and no `role="alert"` on errors. The browser's default focus outline is kept.

**Documentation (8).** The README gives install and run steps, an env-var table including `SESSION_SECRET`, test instructions with a description of their scope, and a feature list. It is accurate. It does not warn that the session secret must be set in real use.

**Top problems:** (1) a hard-coded fallback session secret and sessions that cannot be revoked (`app.js:706-721`); (2) cancelling and putting stock back is not atomic (`db.js:287-295`); (3) a monolithic `app.js` with inline HTML/CSS/JS, unassociated labels on order lines, and table headers that break mid-word on phones.

---

## Project Y

**All criteria (1).** `source/` contains only `package.json`, `package-lock.json` and `.gitignore`. `package.json` has `"start": "node index.js"` and `"test": "node --experimental-sqlite test/index.mjs"`, but neither `index.js` nor `test/` exists. There is no README. All four screenshots (`*-375.png`) have the same MD5 hash and show the browser error "127.0.0.1 refused to connect" (ERR_CONNECTION_REFUSED). The 1440 px screenshots and the login screenshot are missing. Nothing in the spec is implemented, and there is no code to judge for correctness, quality, security, tests or accessibility.

**Top problems:** (1) no application code at all, so `npm start` would fail with "Cannot find module index.js"; (2) no tests, although `npm test` points at a missing file; (3) no README.

---

## Project Z

**Spec coverage (6).** The API is complete and matches the spec (`src/routes/api.js:294-522`), including the CSV, pagination shape and status codes. The pages fall short in two places:
- **Edit does not work in the UI.** The Edit buttons are rendered (`src/public/app.js:151, 311`), but no code anywhere handles `.edit-btn` or `.edit-customer-btn`, and the client never sends a PUT.
- **Search does nothing in the UI.** Submitting navigates to `/products?q=…` (96-100). On load, `buildUrl` reads `searchInput.value` (121), which the server never fills in (`pages.js:82`), and `parseQueryString().q` is never used (225). The customers page has the same bug (282).

Pagination uses buttons, not links. The dashboard, CSV link, order create, status filter and transitions all work.

**Correctness (7).** Money is stored in integer cents with CHECK constraints (`db.js`, products, orders and order_items). Order creation (api.js:313-363) and cancel with restock (471-511) are both in transactions. Order prices are captured at the time of the order. The 409 and 400 rules match the spec. There are three deviations. An order with the same product on two lines is refused with 400 (`validate.js:76`), which the spec does not ask for. Orders are sorted by millisecond `created_at` with no id tie-break (api.js:394). There is no JSON error middleware, so malformed JSON or a thrown error returns Express's HTML response instead of `{"error"}` (`src/app.js`).

**Code quality (6).** The structure is good: config, db, auth, validate, separate route modules for pages and API, and a view helper. Against that:
- Most of the stylesheet is dead. Selectors like `table #products-table` (`styles.css:120-290`) never match, because the table *is* `#products-table`. This is why the tables in the screenshots are unstyled.
- There is a CSRF-token reader for a meta tag that is never emitted (`app.js:220-223`).
- The nav's active state compares `href === active` (`render.js:20`) with values like `'products'`, so it never matches.
- `escapeHtml` and `showError` are copied three times in `public/app.js`, and the test helpers are copied across all 8 test files.

**Security (8).** Passwords use scrypt with a salt and `timingSafeEqual` (`auth.js:1-15`). Session tokens are 32 random bytes kept in a `sessions` table, and logout deletes the row (`auth.js:29-43`). The cookie is `httpOnly` and `sameSite=lax` (api.js:21). SQL is parameterized throughout, and the client escapes data before inserting it. Auth middleware covers every page and API route except login (`src/app.js:36-45`). On the minus side, sessions never expire, there is no `secure` flag, and `INSERT OR IGNORE` means a changed `ADMIN_PASSWORD` is ignored after the first start (`db.js:56-58`).

**Tests (7).** There are about 100 HTTP tests in 8 files. API coverage is broad: every endpoint's 400, 404 and 409 cases, newest-first ordering, nothing changed after a stock refusal, restock, repeated cancels, and revenue excluding pending and cancelled orders (`orders.test.js`, `products.test.js`, `dashboard.test.js`, `integration.test.js`). The UI tests are shallow. For example, `accessibility.test.js:122-138` only checks that words like "SKU" and "Name" appear in the HTML. No test covers edit or search through the pages, which is exactly where the bugs are. They look like they pass.

**UI and UX (4).** The login and dashboard look tidy (`login-1440.png`, `dashboard-1440.png`). The table pages are poor:
- The products table is unstyled and cramped. "Price Stock" run together, prices have no currency, and the table takes only part of the width (`products-1440.png`).
- The status action buttons read "Paid", "Shipped" and "Cancel", which looks like status labels rather than actions (`orders-1440.png`).
- There is no active-page indicator in the nav.
- A refused status change and a failed delete appear in `alert()` (app.js:618, 213), not next to the action.

At 375 px the stacked forms are usable and nothing scrolls sideways, but the intended card layout for tables never takes effect.

**Accessibility (6).** The good parts: labels wrap their inputs everywhere, including lines added by JavaScript (app.js:460-465); order lines sit in a `<fieldset>`/`<legend>`; buttons carry per-row `aria-label`s; the login error has `role="alert"`; and pagination has a labelled `<nav>`. Against that: the `aria-current` the code meant to add is never emitted (render.js:20), inline form errors have no live region, and the status filter changes the table with no announcement.

**Documentation (5).** The README covers run steps, env vars and the transition table. But it says the tests only cover "sign-in page, auth redirects and login", which is out of date given 8 suites. `docs/plan.md` (392 lines) is a design plan rather than user documentation.

**Top problems:** (1) Edit buttons for products and customers do nothing, so there is no edit in the UI; (2) search on the products and customers pages has no effect; (3) table CSS selectors are broken (`table #id`), which leaves cramped, unstyled tables on desktop and phone, alongside API errors returned as HTML and the refusal of duplicate order lines.

---

## Ranking

**1. X, 2. Z, 3. Y.**

X is the only project where every page in the spec visibly works end to end. Its business rules are right, including the re-check of stock inside the transaction, its tests are focused, and its layout is consistent on both desktop and phone. Its weaknesses are a hard-coded fallback session secret, stateless sessions, a non-atomic cancel, and a monolithic `app.js`. All are real, but none breaks a user-facing feature.

Z has the better architecture, money handling (cents with CHECK constraints), session design and API test suite. A user, though, cannot edit a product or customer and cannot search, both explicit spec requirements, and its tables are visibly broken because of dead CSS. Its large test suite missed all of this because it never exercises the page scripts.

Y handed in no application code, and the server never started.
