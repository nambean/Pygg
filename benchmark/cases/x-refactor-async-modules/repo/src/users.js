'use strict';
const storage = require('./storage');
const audit = require('./audit');

function create(name, email, cb) {
  if (!/^[^@\s]+@[^@\s]+$/.test(email)) return cb(new Error('users: bad email'));
  const key = `user:${email.toLowerCase()}`;
  storage.get(key, (err, existing) => {
    if (err) return cb(err);
    if (existing) return cb(new Error('users: exists'));
    const user = { name, email: email.toLowerCase(), created: 1 };
    storage.set(key, user, (e) => {
      if (e) return cb(e);
      audit.record('user.created', user.email, (e2) => cb(e2, e2 ? undefined : user));
    });
  });
}

function find(email, cb) {
  storage.get(`user:${email.toLowerCase()}`, (err, user) => {
    if (err) return cb(err);
    if (!user) return cb(new Error('no such user'));
    cb(null, user);
  });
}

function remove(email, cb) {
  find(email, (err, user) => {
    if (err) return cb(err);
    storage.del(`user:${user.email}`, (e) => {
      if (e) return cb(e);
      audit.record('user.removed', user.email, (e2) => cb(e2, e2 ? undefined : true));
    });
  });
}

module.exports = { create, find, remove };
