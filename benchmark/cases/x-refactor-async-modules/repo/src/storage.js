'use strict';
// An in-memory key-value store with asynchronous access, like a real one.
const data = new Map();

function later(fn) { setImmediate(fn); }

function get(key, cb) {
  later(() => cb(null, data.has(key) ? JSON.parse(data.get(key)) : null));
}

function set(key, value, cb) {
  if (typeof key !== 'string' || key === '') return later(() => cb(new Error('storage: bad key')));
  later(() => { data.set(key, JSON.stringify(value)); cb(null, value); });
}

function del(key, cb) {
  later(() => cb(null, data.delete(key)));
}

function list(prefix, cb) {
  later(() => cb(null, [...data.keys()].filter((k) => k.startsWith(prefix)).sort()));
}

function reset() { data.clear(); }

module.exports = { get, set, del, list, reset };
