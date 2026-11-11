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
const { wrap } = require('./wrap');
test('greedy', () => assert.equal(wrap('The quick brown fox jumps over the lazy dog', 10), 'The quick\nbrown fox\njumps over\nthe lazy\ndog'));
test('exact width', () => assert.equal(wrap('hello world', 5), 'hello\nworld'));
test('long word', () => assert.equal(wrap('abcdefghij', 4), 'abcd\nefgh\nij'));
test('long word mid line', () => assert.equal(wrap('a verylongword b', 5), 'a\nveryl\nongwo\nrd b'));
test('paragraphs', () => assert.equal(wrap('a b\n\n\nc d', 80), 'a b\n\nc d'));
test('single newline is a space', () => assert.equal(wrap('one\ntwo', 80), 'one two'));
test('trims', () => assert.equal(wrap('  hi  ', 5), 'hi'));
test('empty', () => { assert.equal(wrap('', 5), ''); assert.equal(wrap('  \n ', 5), ''); });
test('bad width', () => assert.throws(() => wrap('x', 0), RangeError));
done();
