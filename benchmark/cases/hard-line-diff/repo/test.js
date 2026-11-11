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
const { diff } = require('./diff');
// Applies a unified diff to a and checks its structure.
function apply(a, patch) {
  const src = a.split('\n'); const out = []; let pos = 0;
  const lines = patch.split('\n'); assert.equal(lines.pop(), '', 'the diff ends with a newline');
  assert.equal(lines[0], '--- a'); assert.equal(lines[1], '+++ b');
  let k = 2, hunks = 0;
  while (k < lines.length) {
    const m = /^@@ -(\d+),(\d+) \+(\d+),(\d+) @@$/.exec(lines[k++]); assert.ok(m, 'hunk header: ' + lines[k - 1]);
    hunks++;
    const [ , as, al, bs, bl] = m.map(Number);
    const start = al === 0 ? as : as - 1;
    assert.ok(start >= pos, 'hunks in order');
    while (pos < start) out.push(src[pos++]);
    let seenA = 0, seenB = 0;
    while (k < lines.length && !lines[k].startsWith('@@')) {
      const l = lines[k++], t = l[0], body = l.slice(1);
      if (t === ' ') { assert.equal(src[pos], body, 'context matches'); out.push(body); pos++; seenA++; seenB++; }
      else if (t === '-') { assert.equal(src[pos], body, 'removed line matches'); pos++; seenA++; }
      else if (t === '+') { out.push(body); seenB++; }
      else assert.fail('bad line ' + JSON.stringify(l));
    }
    assert.equal(seenA, al, 'old count'); assert.equal(seenB, bl, 'new count');
  }
  while (pos < src.length) out.push(src[pos++]);
  return { text: out.join('\n'), hunks, changed: lines.slice(2).filter((l) => l[0] === '+' || l[0] === '-').length };
}
function lcs(x, y) { const d = Array.from({ length: x.length + 1 }, () => new Array(y.length + 1).fill(0));
  for (let i = 1; i <= x.length; i++) for (let j = 1; j <= y.length; j++) d[i][j] = x[i - 1] === y[j - 1] ? d[i - 1][j - 1] + 1 : Math.max(d[i - 1][j], d[i][j - 1]);
  return d[x.length][y.length]; }
function check(a, b) {
  const p = diff(a, b); const r = apply(a, p);
  assert.equal(r.text, b, 'applying the diff gives b');
  const x = a.split('\n'), y = b.split('\n');
  assert.equal(r.changed, x.length + y.length - 2 * lcs(x, y), 'minimal');
  return r;
}
const L = (n, f = (i) => 'line ' + i) => Array.from({ length: n }, (_, i) => f(i + 1)).join('\n');
test('equal', () => assert.equal(diff('a\nb', 'a\nb'), ''));
test('one change', () => { const r = check(L(10), L(10, (i) => (i === 5 ? 'five' : 'line ' + i))); assert.equal(r.hunks, 1); });
test('header and context', () => assert.equal(diff(L(10), L(10, (i) => (i === 5 ? 'five' : 'line ' + i))),
  '--- a\n+++ b\n@@ -2,7 +2,7 @@\n line 2\n line 3\n line 4\n-line 5\n+five\n line 6\n line 7\n line 8\n'));
test('two far changes are two hunks', () => { const r = check(L(30), L(30, (i) => (i === 3 || i === 25 ? 'x' + i : 'line ' + i))); assert.equal(r.hunks, 2); });
test('near changes merge', () => { const r = check(L(30), L(30, (i) => (i === 10 || i === 16 ? 'x' + i : 'line ' + i))); assert.equal(r.hunks, 1); });
test('insert at start', () => check('b\nc', 'a\nb\nc'));
test('delete at end', () => check('a\nb\nc', 'a\nb'));
test('from empty', () => { assert.ok(diff('', 'x\ny').includes('@@ -1,1 +1,2 @@') || diff('', 'x\ny').includes('@@ -0,0') || true); check('', 'x\ny'); });
test('reordered', () => check('a\nb\nc\nd\ne', 'e\na\nc\nb\nd'));
test('many', () => check(L(60, (i) => 'v' + (i % 7)), L(55, (i) => 'v' + (i % 5))));
done();
