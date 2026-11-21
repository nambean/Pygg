Brainstorm b_5c31138417 · revision 1 · open it in Pygg: pygg://brainstorm/b_5c31138417
Planner: mixai-deepseek/deepseek-v4-pro (brain-v2)

No open questions.

Proposed cards (models picked by Pygg's rules; change them in Pygg):
Wave 1:
- c1 [plan] Write design plan for Stockroom — auto/plan -> mixai-deepseek/deepseek-v4-pro, then mixai-glm/glm-5.2, mixai-qwen/qwen3.7-max
  done when: docs/plan.md exists and documents schema, auth, pages, and API routes
Wave 2:
- c2 [code] Scaffold Express app, login page, README, and test harness — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c1)
  done when: `npm install && npm test` passes and a test starts the app on an ephemeral port and gets /login
Wave 3:
- c3 [code] Create SQLite schema and seed admin user — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c2)
  done when: `npm test` passes with tests that verify tables exist and the admin row has a hashed password
Wave 4:
- c4 [code] Add session auth, login/logout API and sign-in page — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c3)
  done when: `npm test` passes with login/logout API tests (200 and cookie, 401 wrong password, 200 logout)
Wave 5:
- c5 [code] Add product API for list, get, create, and update — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c4)
  done when: `npm test` passes with product API tests for list/search/pagination, create, get, update, 400 and 409 cases
Wave 6:
- c6 [code] Add product delete API and CSV export — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c5)
  done when: `npm test` passes with DELETE /api/products/:id (204 and 409) and GET /api/products.csv tests
Wave 7:
- c7 [code] Add customer API CRUD and delete restriction — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c6)
  done when: `npm test` passes with customer API tests for CRUD, duplicate email 409, and delete-with-orders 409
Wave 8:
- c8 [code] Add order creation API with stock transaction — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c7)
  done when: `npm test` passes with order creation tests: 201 pending order, stock reduced, 400 unknown customer/product, 409 insufficient stock
Wave 9:
- c9 [code] Add order list/get/status API with transitions — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c8)
  done when: `npm test` passes with order list/get/status tests including 409 refused transitions and cancel restocks stock
Wave 10:
- c10 [code] Add dashboard API — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c9)
  done when: `npm test` passes with dashboard API tests for counts, revenue, and low stock
Wave 11:
- c11 [test] Add full API workflow integration tests — auto/test -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c10)
  done when: `npm test` passes with an integration test covering login -> product -> customer -> order -> paid -> shipped and cancel/restock
Wave 12:
- c12 [code] Add products page with table, search, pagination, and add form — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c11)
  done when: `npm test` passes with page tests for /products listing, search/pagination, add-product success and duplicate error
Wave 13:
- c13 [code] Add product edit and delete pages — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c12, c6)
  done when: `npm test` passes with product edit/delete page tests including update success and delete confirmation
- c14 [code] Add customers page with add/edit/delete — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c12, c7)
  done when: `npm test` passes with /customers page tests for table, add, edit, delete, and duplicate email error
- c15 [code] Add orders page with list, filter, and status actions — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c12, c9)
  done when: `npm test` passes with /orders page tests for newest-first list, status filter, and allowed status actions
- c17 [code] Add dashboard page — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c12, c10)
  done when: `npm test` passes with dashboard page tests showing counts, revenue, and low stock
Wave 14:
- c16 [code] Add create order page with line items — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c15, c8)
  done when: `npm test` passes with create-order page tests for customer selection, line items, success stock reduction, and too-little-stock error
Wave 15:
- c18 [code] Add responsive CSS and accessibility checks for all pages — auto/code -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c13, c14, c15, c16, c17)
  done when: `npm test` passes with tests asserting viewport meta, visible form labels, and shared nav on authenticated pages
Wave 16:
- c19 [review] Final review and README verification — auto/review -> mixai-qwen/qwen3.6-flash, then mixai-deepseek/deepseek-v4-pro, mixai-glm/glm-5.2 (after c18)
  done when: `npm test` passes and README.md contains run/test instructions, env vars, and the status transition rules

Approve in Pygg; this client cannot approve. Paid cards still wait for approval one by one when they start.
