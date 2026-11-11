const assert = require('node:assert/strict');
let failed = 0;
const pending = [];
function test(name, fn) {
  pending.push(Promise.resolve().then(fn).catch((e) => {
    failed++;
    console.log(`FAIL ${name}: ${String(e && e.message).split('\n')[0]}`);
  }));
}
function done() {
  Promise.all(pending).then(() => {
    if (failed) { console.log(`${failed} failed`); process.exit(1); }
    console.log('all passed');
  });
}
const users = require('./users');
test('create', () => assert.deepEqual(users.createUser('An', 'An@Example.com'), { name: 'An', email: 'an@example.com' }));
test('update', () => assert.deepEqual(users.updateEmail({ name: 'B', email: 'x@y.z' }, 'New@Mail.io'), { name: 'B', email: 'new@mail.io' }));
test('invite', () => assert.deepEqual(users.inviteUser('C@d.ee'), { invited: 'c@d.ee' }));
for (const bad of ['nope', 'a@b', 'a b@c.d', '@c.d']) {
  test('rejects ' + bad, () => { for (const f of [() => users.createUser('n', bad), () => users.updateEmail({}, bad), () => users.inviteUser(bad)]) assert.throws(f, /invalid email/); });
}
test('validate module', () => { const { isValidEmail } = require('./validate'); assert.equal(isValidEmail('a@b.co'), true); assert.equal(isValidEmail('a@b'), false); });
done();
