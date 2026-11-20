'use strict';
const users = require('./users');
const sessions = require('./sessions');

// Signs a user up and opens a first session; a known email only opens a session.
function signIn(name, email, cb) {
  users.find(email, (err) => {
    const go = () => sessions.open(email, cb);
    if (!err) return go();
    if (err.message !== 'no such user') return cb(err);
    users.create(name, email, (e) => (e ? cb(e) : go()));
  });
}

// Closes every open session of a user, then removes the user; resolves to the number of sessions closed.
function leave(email, cb) {
  sessions.openFor(email, (err, ids) => {
    if (err) return cb(err);
    let i = 0;
    (function next() {
      if (i === ids.length) return users.remove(email, (e) => cb(e, e ? undefined : ids.length));
      sessions.close(ids[i++], (e) => (e ? cb(e) : next()));
    })();
  });
}

module.exports = { signIn, leave };
