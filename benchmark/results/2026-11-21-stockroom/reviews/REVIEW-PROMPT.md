# Blind review: three implementations of one specification

You are reviewing three web applications, **Project X**, **Project Y** and **Project Z**, each built by a different AI coding setup from the same specification, `SPEC.md` (in this folder). You do not know which setup built which, and it does not matter: judge only what is in front of you. Do not guess the setup.

Each project folder holds the source as it was handed in (no Git history, no `node_modules`). Next to each are screenshots of its pages taken by an automated browser after a scripted session, at 1440 px (`*-1440.png`) and at 375 px phone width (`*-375.png`): sign-in, dashboard, products, customers, orders. A missing screenshot means that page could not be reached.

## What to do

Read `SPEC.md`, then each project's code, and look at its screenshots. You may read any file. Do not modify anything, and do not run servers or install packages.

Score each project from 1 to 10 on each criterion, with **concrete evidence** for every score (file and line, or what a screenshot shows). A score without evidence does not count.

1. **Spec coverage:** how much of `SPEC.md` is implemented as written (pages, API, rules, sign-in, CSV, pagination, search, error messages).
2. **Correctness:** the rules (stock taken and returned, refused status changes, refused deletes, validation, totals at the order's prices) and edge cases; transactions where several writes must succeed together.
3. **Code quality:** structure, separation of concerns, duplication, naming, readability, error handling.
4. **Security:** password hashing, session handling and cookie flags, SQL injection, output escaping (XSS), access control on every route.
5. **Tests:** what the project's own tests cover and how well; whether they look like they pass.
6. **UI and UX:** from the screenshots and the page code: layout, clarity, feedback after actions and errors, consistency between pages, the phone layout.
7. **Accessibility:** labels, semantic HTML, focus and keyboard use, contrast.
8. **Documentation:** the README.

Then give each project an **overall score from 1 to 10** (your judgement, not an average) and **rank the three**. List the three most serious problems of each project.

## Answer format

Markdown, in this order:

1. A table: rows are the eight criteria and Overall, columns are X, Y, Z.
2. For each project: the evidence for each score, in one or two sentences per criterion, then its three most serious problems.
3. The ranking, with one paragraph on why.

Keep it under 1,800 words.
