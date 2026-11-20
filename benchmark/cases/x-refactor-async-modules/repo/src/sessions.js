'use strict';
const storage = require('./storage');
const users = require('./users');
const audit = require('./audit');

let counter = 0;

function open(email, cb) {
  users.find(email, (err, user) => {
    if (err) return cb(err);
    counter += 1;
    const id = `s${counter}`;
    storage.set(`session:${id}`, { id, email: user.email, open: true }, (e) => {
      if (e) return cb(e);
      audit.record('session.opened', id, (e2) => cb(e2, e2 ? undefined : id));
    });
  });
}

function close(id, cb) {
  storage.get(`session:${id}`, (err, s) => {
    if (err) return cb(err);
    if (!s || !s.open) return cb(new Error('sessions: not open'));
    storage.set(`session:${id}`, { ...s, open: false }, (e) => {
      if (e) return cb(e);
      audit.record('session.closed', id, (e2) => cb(e2, e2 ? undefined : true));
    });
  });
}

function openFor(email, cb) {
  storage.list('session:', (err, keys) => {
    if (err) return cb(err);
    const out = [];
    let i = 0;
    (function next() {
      if (i === keys.length) return cb(null, out);
      storage.get(keys[i++], (e, s) => {
        if (e) return cb(e);
        if (s.open && s.email === email.toLowerCase()) out.push(s.id);
        next();
      });
    })();
  });
}

function reset() { counter = 0; }

module.exports = { open, close, openFor, reset };
