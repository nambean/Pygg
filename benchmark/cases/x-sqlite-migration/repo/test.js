'use strict';
// Checks migration 002 and store.js against a version-1 database built from fixtures/v1.sql.
const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { DatabaseSync } = require('node:sqlite');

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'shop-'));
const v1 = path.join(tmp, 'v1.sqlite');
{
  const db = new DatabaseSync(v1);
  require('./migrations/001_init.js').up(db);
  db.exec('PRAGMA user_version = 1');
  db.exec(fs.readFileSync(path.join(__dirname, 'fixtures', 'v1.sql'), 'utf8'));
  db.close();
}

const { open, migrations } = require('./db.js');
const store = require('./store.js');
const latest = Math.max(...migrations().map((m) => m.version));
assert.ok(latest >= 2, 'a migration 002 exists');

// Upgrade in place.
const db = open(v1);
assert.strictEqual(db.prepare('PRAGMA user_version').get().user_version, latest, 'upgraded to the latest version');
const cols = db.prepare('PRAGMA table_info(customers)').all().map((c) => c.name);
assert.ok(cols.includes('first_name') && cols.includes('last_name'), 'first_name and last_name exist');
assert.ok(!cols.includes('name'), 'name is gone');

const rows = db.prepare('SELECT id, first_name, last_name, email FROM customers ORDER BY id').all()
  .map((r) => ({ ...r }));
assert.deepStrictEqual(rows, [
  { id: 1, first_name: 'Ada', last_name: 'Lovelace', email: 'ada@example.com' },
  { id: 2, first_name: 'Grace Brewster', last_name: 'Hopper', email: 'grace@navy.mil' },
  { id: 3, first_name: 'Cher', last_name: '', email: 'cher@music.test' },
  { id: 5, first_name: 'Alan', last_name: 'Turing', email: 'alan@bletchley.uk' },
], 'names split, emails lower-cased and trimmed, duplicates merged into the lowest id');

const orders = db.prepare('SELECT id, customer_id, total_cents FROM orders ORDER BY id').all().map((r) => ({ ...r }));
assert.deepStrictEqual(orders, [
  { id: 1, customer_id: 1, total_cents: 1200 },
  { id: 2, customer_id: 1, total_cents: 800 },
  { id: 3, customer_id: 1, total_cents: 50 },
  { id: 4, customer_id: 2, total_cents: 9900 },
  { id: 5, customer_id: 2, total_cents: 100 },
  { id: 6, customer_id: 3, total_cents: 4500 },
  { id: 7, customer_id: 5, total_cents: 0 },
], 'every order kept, moved to the merged customer');

const unique = db.prepare("SELECT name FROM pragma_index_list('customers') WHERE \"unique\" = 1").all()
  .some((ix) => db.prepare(`PRAGMA index_info('${ix.name}')`).all().some((c) => c.name === 'email'));
assert.ok(unique, 'a unique index on email');
assert.strictEqual(db.prepare('PRAGMA foreign_key_check').all().length, 0, 'no order points at a missing customer');

// The store on the upgraded database.
assert.deepStrictEqual({ ...store.customerSummary(db, 1) },
  { firstName: 'Ada', lastName: 'Lovelace', email: 'ada@example.com', orders: 3, totalCents: 2050 });
assert.deepStrictEqual({ ...store.customerSummary(db, 3) },
  { firstName: 'Cher', lastName: '', email: 'cher@music.test', orders: 1, totalCents: 4500 });
assert.strictEqual(store.customerSummary(db, 4), null, 'a merged customer is gone');
const id = store.addCustomer(db, { firstName: 'Katherine', lastName: 'Johnson', email: '  KJ@NASA.gov ' });
assert.deepStrictEqual({ ...store.customerSummary(db, id) },
  { firstName: 'Katherine', lastName: 'Johnson', email: 'kj@nasa.gov', orders: 0, totalCents: 0 });
assert.throws(() => store.addCustomer(db, { firstName: 'K', lastName: 'J', email: 'kj@nasa.GOV' }), /duplicate email/);
assert.throws(() => store.addCustomer(db, { firstName: 'A', lastName: 'B', email: 'Ada@example.com' }), /duplicate email/);
store.addOrder(db, id, 300, '2026-04-01');
assert.strictEqual(store.customerSummary(db, id).totalCents, 300);
db.close();

// Opening again changes nothing.
const before = fs.readFileSync(v1);
const again = open(v1);
assert.strictEqual(again.prepare('PRAGMA user_version').get().user_version, latest);
assert.strictEqual(again.prepare('SELECT COUNT(*) AS n FROM customers').get().n, 5);
again.close();
assert.ok(before.equals(fs.readFileSync(v1)), 'an up-to-date database is left as it is');

// A new database.
const fresh = open(path.join(tmp, 'new.sqlite'));
assert.strictEqual(fresh.prepare('PRAGMA user_version').get().user_version, latest);
const fid = store.addCustomer(fresh, { firstName: 'Mary', lastName: 'Jackson', email: 'mary@nasa.gov' });
assert.strictEqual(store.customerSummary(fresh, fid).lastName, 'Jackson');
fresh.close();

console.log('sqlite migration: ok');
