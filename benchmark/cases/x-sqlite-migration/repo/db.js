'use strict';
// Opens the shop database and brings it to the latest schema version.
const fs = require('fs');
const path = require('path');
const { DatabaseSync } = require('node:sqlite');

function migrations() {
  const dir = path.join(__dirname, 'migrations');
  return fs.readdirSync(dir)
    .filter((f) => /^\d{3}_.*\.js$/.test(f))
    .sort()
    .map((f) => ({ version: Number(f.slice(0, 3)), up: require(path.join(dir, f)).up }));
}

function open(file) {
  const db = new DatabaseSync(file);
  db.exec('PRAGMA foreign_keys = ON');
  const current = db.prepare('PRAGMA user_version').get().user_version;
  for (const m of migrations()) {
    if (m.version <= current) continue;
    db.exec('BEGIN');
    try {
      m.up(db);
      db.exec(`PRAGMA user_version = ${m.version}`);
      db.exec('COMMIT');
    } catch (e) {
      db.exec('ROLLBACK');
      throw e;
    }
  }
  return db;
}

module.exports = { open, migrations };
