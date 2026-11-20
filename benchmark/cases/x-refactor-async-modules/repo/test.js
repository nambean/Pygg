'use strict';
// The service's behaviour through its promise API.
const assert = require('assert');
const storage = require('./src/storage');
const audit = require('./src/audit');
const users = require('./src/users');
const sessions = require('./src/sessions');
const app = require('./src/app');

async function rejects(p, message) {
  await assert.rejects(p, (e) => e instanceof Error && e.message === message, `rejects with "${message}"`);
}

(async () => {
  storage.reset(); audit.reset(); sessions.reset();

  // storage
  assert.ok(storage.get('x') instanceof Promise, 'storage.get returns a promise');
  assert.strictEqual(await storage.get('x'), null);
  assert.deepStrictEqual(await storage.set('k:1', { a: 1 }), { a: 1 });
  assert.deepStrictEqual(await storage.get('k:1'), { a: 1 });
  await rejects(storage.set('', 1), 'storage: bad key');
  assert.deepStrictEqual(await storage.list('k:'), ['k:1']);
  assert.strictEqual(await storage.del('k:1'), true);
  assert.strictEqual(await storage.del('k:1'), false);

  // users
  const ada = await users.create('Ada', 'Ada@Example.com');
  assert.deepStrictEqual(ada, { name: 'Ada', email: 'ada@example.com', created: 1 });
  await rejects(users.create('Ada', 'ada@example.com'), 'users: exists');
  await rejects(users.create('X', 'not-an-email'), 'users: bad email');
  assert.deepStrictEqual(await users.find('ADA@example.com'), ada);
  await rejects(users.find('nobody@x.test'), 'no such user');

  // sessions
  const s1 = await sessions.open('ada@example.com');
  const s2 = await sessions.open('ada@example.com');
  assert.deepStrictEqual([s1, s2], ['s1', 's2']);
  await rejects(sessions.open('nobody@x.test'), 'no such user');
  assert.deepStrictEqual(await sessions.openFor('Ada@example.com'), ['s1', 's2']);
  assert.strictEqual(await sessions.close('s1'), true);
  await rejects(sessions.close('s1'), 'sessions: not open');
  await rejects(sessions.close('s9'), 'sessions: not open');
  assert.deepStrictEqual(await sessions.openFor('ada@example.com'), ['s2']);

  // app
  const s3 = await app.signIn('Grace', 'grace@navy.mil');
  assert.strictEqual(s3, 's3');
  assert.strictEqual(await app.signIn('Grace again', 'grace@navy.mil'), 's4');
  await rejects(app.signIn('Bad', 'bad'), 'users: bad email');
  assert.strictEqual(await app.leave('grace@navy.mil'), 2);
  await rejects(users.find('grace@navy.mil'), 'no such user');
  await rejects(app.leave('grace@navy.mil'), 'no such user');
  assert.strictEqual(await app.leave('ada@example.com'), 1);

  // audit, in order
  const log = (await audit.entries()).map((e) => `${e.event} ${e.detail}`);
  assert.deepStrictEqual(log, [
    'user.created ada@example.com',
    'session.opened s1',
    'session.opened s2',
    'session.closed s1',
    'user.created grace@navy.mil',
    'session.opened s3',
    'session.opened s4',
    'session.closed s3',
    'session.closed s4',
    'user.removed grace@navy.mil',
    'session.closed s2',
    'user.removed ada@example.com',
  ]);

  // Concurrent sign-ins of different users interleave but each completes.
  const ids = await Promise.all([app.signIn('A', 'a@x.test'), app.signIn('B', 'b@x.test'), app.signIn('C', 'c@x.test')]);
  assert.deepStrictEqual([...ids].sort(), ['s5', 's6', 's7']);

  console.log('async modules: ok');
})().catch((e) => { console.error(e); process.exit(1); });
