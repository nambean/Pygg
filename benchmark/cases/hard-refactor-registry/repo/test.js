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
const { render } = require('./report');
const { exportRows } = require('./export');
const rows = [{ name: 'a', n: 1 }, { name: 'b,c', n: 2 }];
test('csv', () => assert.equal(render(rows, 'csv'), 'name,n\na,1\n"b,c",2'));
test('tsv', () => assert.equal(render(rows, 'tsv'), 'name\tn\na\t1\nb,c\t2'));
test('json', () => assert.equal(render(rows, 'json'), JSON.stringify(rows)));
test('unknown', () => assert.throws(() => render(rows, 'xml'), /unknown format xml/));
test('export uses the same formats', () => { assert.equal(exportRows(rows, 'tsv').body, render(rows, 'tsv')); assert.equal(exportRows(rows, 'csv').type, 'text/csv'); assert.equal(exportRows(rows, 'json').type, 'application/json'); });
test('registry', () => { const reg = require('./formats'); reg.register('pipe', { type: 'text/plain', render: (rs) => rs.map((r) => r.name).join('|') });
  assert.equal(render(rows, 'pipe'), 'a|b,c'); assert.equal(exportRows(rows, 'pipe').type, 'text/plain'); assert.deepEqual(reg.names().slice(0, 3).sort(), ['csv', 'json', 'tsv']); });
done();
