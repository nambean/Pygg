| Criterion | X | Y | Z |
|---|---:|---:|---:|
| Spec coverage | 9 | 1 | 7 |
| Correctness | 7 | 1 | 7 |
| Code quality | 6 | 1 | 7 |
| Security | 4 | 1 | 7 |
| Tests | 7 | 1 | 8 |
| UI and UX | 7 | 1 | 5 |
| Accessibility | 6 | 1 | 6 |
| Documentation | 8 | 1 | 8 |
| **Overall** | **7** | **1** | **7** |

## Project X

- **Spec coverage — 9/10.** The app implements the requested pages and API, including search, pagination, CSV, editing and order actions ([app.js](Project-X/source/src/app.js:254), [app.js](Project-X/source/src/app.js:750)). The phone screenshots show the main workflows, though tables become cramped.
- **Correctness — 7/10.** Order creation deducts stock inside a transaction, including when a product occurs on multiple lines; it stores the order-time price ([db.js](Project-X/source/src/db.js:232)). Cancellation restocks and changes status without a transaction, so a failure between writes can leave stock and status inconsistent ([db.js](Project-X/source/src/db.js:275)).
- **Code quality — 6/10.** Database operations are separated into `Store`, and validation is readable ([db.js](Project-X/source/src/db.js:54), [app.js](Project-X/source/src/app.js:14)). The large application file combines routes, HTML, CSS and browser scripts, making changes harder to isolate.
- **Security — 4/10.** Passwords use salted scrypt, queries use parameters, and dynamic HTML is escaped ([db.js](Project-X/source/src/db.js:4), [app.js](Project-X/source/src/app.js:61)). But the known default session signing secret can let an attacker forge an authenticated cookie; the cookie also lacks a `Secure` setting ([app.js](Project-X/source/src/app.js:706)).
- **Tests — 7/10.** Tests cover authentication, CRUD, pagination, CSV, stock, transitions and deletion guards ([app.test.js](Project-X/source/test/app.test.js:49)). They do not demonstrate cancellation atomicity; I did not run them, as requested.
- **UI and UX — 7/10.** Screenshots show consistent navigation, clear forms and readable desktop tables; form errors are displayed inline and delete errors use alerts ([app.js](Project-X/source/src/app.js:316)). On the phone, product and order table headings and values break awkwardly across lines.
- **Accessibility — 6/10.** Fields have visible associated labels and the pages use landmarks and native controls ([app.js](Project-X/source/src/app.js:135), [app.js](Project-X/source/src/app.js:188)). Edit links contain buttons, creating invalid nested interactive controls; dynamic errors are not announced as live alerts ([app.js](Project-X/source/src/app.js:270)).
- **Documentation — 8/10.** The README gives installation, configuration, credentials, tests and features, including the session secret ([README.md](Project-X/source/README.md:9)). It does not explain the security consequences of leaving that secret at its default.

**Three most serious problems**

1. Predictable default session secret undermines authentication.
2. Cancellation is not atomic with stock restoration.
3. Phone tables are difficult to read, especially orders.

## Project Y

- **Spec coverage — 1/10.** Only package manifests are present; `npm start` refers to a missing `index.js` ([package.json](Project-Y/source/package.json:5)). All supplied screenshots show connection refusal.
- **Correctness — 1/10.** No application or database code is present to implement or verify the rules.
- **Code quality — 1/10.** There is no application source to assess; the manifest points to missing entry points.
- **Security — 1/10.** There is no submitted authentication, session handling or route protection to assess.
- **Tests — 1/10.** The test command references a missing `test/index.mjs` ([package.json](Project-Y/source/package.json:8)); no tests were submitted.
- **UI and UX — 1/10.** The four screenshots show the browser’s connection-refused page, not an application.
- **Accessibility — 1/10.** No application markup was submitted or shown, so the requested labels and keyboard experience cannot be assessed.
- **Documentation — 1/10.** No README was submitted.

**Three most serious problems**

1. The server entry point is missing, so the application cannot start as handed in.
2. The specified application and business rules are absent.
3. Tests and README are missing.

## Project Z

- **Spec coverage — 7/10.** The API includes the requested CRUD, CSV, dashboard and order endpoints ([api.js](Project-Z/source/src/routes/api.js:294)). Product and customer **Edit** buttons are rendered, but their click handlers only handle deletion, so browser editing is unavailable ([app.js](Project-Z/source/src/public/app.js:189), [app.js](Project-Z/source/src/public/app.js:318)).
- **Correctness — 7/10.** Creation and cancellation use transactions, preserve order-time prices and guard transitions and deletes ([api.js](Project-Z/source/src/routes/api.js:301), [api.js](Project-Z/source/src/routes/api.js:444)). Validation rejects repeated product lines even when sufficient stock exists, a restriction absent from the specification; prices are also rounded to cents although the spec permits any nonnegative number ([validate.js](Project-Z/source/src/validate.js:20), [validate.js](Project-Z/source/src/validate.js:77)).
- **Code quality — 7/10.** Authentication, validation, routes, database and rendering are separated into modules ([app.js](Project-Z/source/src/app.js:1)). Browser code is lengthy and silently ignores some loading failures, which can leave lists empty without explanation ([app.js](Project-Z/source/src/public/app.js:284)).
- **Security — 7/10.** Salted scrypt, parameterized queries, escaped browser output and server-side session invalidation are present ([auth.js](Project-Z/source/src/auth.js:3), [app.js](Project-Z/source/src/public/app.js:297)). The session cookie is HttpOnly and SameSite=Lax but lacks `Secure`, and stored sessions have no expiry check ([api.js](Project-Z/source/src/routes/api.js:20), [auth.js](Project-Z/source/src/auth.js:36)).
- **Tests — 8/10.** A broad suite exercises CRUD, authentication, workflows, transitions, dashboard and some markup ([orders.test.js](Project-Z/source/tests/orders.test.js:75), [integration.test.js](Project-Z/source/tests/integration.test.js:53)). The markup checks do not catch the inert edit buttons or the repeated-product restriction; tests were not run.
- **UI and UX — 5/10.** The dashboard cards and phone form layout are clear, but screenshots show crowded, largely unstyled tables and tiny action buttons. The nonfunctional edit controls and silently ignored list-loading errors materially affect usability.
- **Accessibility — 6/10.** Pages provide visible field labels and native controls; tests check navigation and labels ([accessibility.test.js](Project-Z/source/tests/accessibility.test.js:122)). The inert edit buttons fail keyboard users too, and the cramped phone tables impair readability.
- **Documentation — 8/10.** The README covers setup, environment variables, tests and status rules ([README.md](Project-Z/source/README.md:10)). Its description of test coverage understates the broader submitted suite.

**Three most serious problems**

1. Product and customer edit buttons do nothing.
2. Valid orders with repeated product lines are rejected.
3. Sessions have no server-side expiry, and cookies lack `Secure`.

## Ranking

**1. X, 2. Z, 3. Y.** X narrowly leads because its browser workflows, including editing, are more complete and its interface is more coherent. Z has stronger separation, broader tests and transactional cancellation, but its broken edit controls and extra order restriction are significant specification gaps. Both have security and correctness issues that deserve attention. Y cannot start from the submitted files.