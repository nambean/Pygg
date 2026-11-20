'use strict';
const storage = require('./storage');

let seq = 0;

function record(event, detail, cb) {
  seq += 1;
  const key = `audit:${String(seq).padStart(6, '0')}`;
  storage.set(key, { event, detail }, (err) => cb(err, key));
}

function entries(cb) {
  storage.list('audit:', (err, keys) => {
    if (err) return cb(err);
    const out = [];
    let i = 0;
    (function next() {
      if (i === keys.length) return cb(null, out);
      storage.get(keys[i++], (e, v) => {
        if (e) return cb(e);
        out.push(v);
        next();
      });
    })();
  });
}

function reset() { seq = 0; }

module.exports = { record, entries, reset };
