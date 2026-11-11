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
const { Emitter } = require('./emitter');
test('order and args', () => { const e = new Emitter(), seen = []; e.on('x', (a) => seen.push('1' + a)).on('x', (a) => seen.push('2' + a));
  assert.equal(e.emit('x', 'a'), true); assert.deepEqual(seen, ['1a', '2a']); });
test('no listeners', () => assert.equal(new Emitter().emit('nothing'), false));
test('once', () => { const e = new Emitter(); let n = 0; e.once('x', () => n++); e.emit('x'); e.emit('x'); assert.equal(n, 1); assert.equal(e.emit('x'), false); });
test('off', () => { const e = new Emitter(); let n = 0; const f = () => n++; e.on('x', f); e.on('x', f); e.off('x', f); e.emit('x'); assert.equal(n, 1); });
test('off a once listener', () => { const e = new Emitter(); let n = 0; const f = () => n++; e.once('x', f); e.off('x', f); e.emit('x'); assert.equal(n, 0); });
test('added during emit', () => { const e = new Emitter(); let late = 0; e.on('x', () => e.on('x', () => late++)); e.emit('x'); assert.equal(late, 0); e.emit('x'); assert.equal(late, 1); });
test('removed during emit', () => { const e = new Emitter(), seen = []; const b = () => seen.push('b');
  e.on('x', () => { seen.push('a'); e.off('x', b); }); e.on('x', b); e.emit('x'); assert.deepEqual(seen, ['a', 'b']); e.emit('x'); assert.deepEqual(seen, ['a', 'b', 'a']); });
done();
