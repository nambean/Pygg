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
const { nextRun } = require('./cron');
const at = (s) => new Date(s);
const iso = (d) => d.toISOString();
test('every minute', () => assert.equal(iso(nextRun('* * * * *', at('2026-01-01T00:00:30Z'))), '2026-01-01T00:01:00.000Z'));
test('strictly after', () => assert.equal(iso(nextRun('* * * * *', at('2026-01-01T00:00:00Z'))), '2026-01-01T00:01:00.000Z'));
test('step', () => assert.equal(iso(nextRun('*/15 * * * *', at('2026-01-01T10:07:00Z'))), '2026-01-01T10:15:00.000Z'));
test('hour rollover', () => assert.equal(iso(nextRun('*/15 * * * *', at('2026-01-01T10:50:00Z'))), '2026-01-01T11:00:00.000Z'));
test('list and range', () => assert.equal(iso(nextRun('0 9-17/4,22 * * *', at('2026-01-01T14:00:00Z'))), '2026-01-01T17:00:00.000Z'));
test('day rollover', () => assert.equal(iso(nextRun('30 6 * * *', at('2026-01-01T07:00:00Z'))), '2026-01-02T06:30:00.000Z'));
test('month end', () => assert.equal(iso(nextRun('0 0 1 * *', at('2026-01-31T12:00:00Z'))), '2026-02-01T00:00:00.000Z'));
test('the 31st skips short months', () => assert.equal(iso(nextRun('0 0 31 * *', at('2026-01-31T12:00:00Z'))), '2026-03-31T00:00:00.000Z'));
test('leap day', () => assert.equal(iso(nextRun('0 12 29 2 *', at('2026-03-01T00:00:00Z'))), '2028-02-29T12:00:00.000Z'));
test('weekday (2026-01-01 is a Thursday)', () => assert.equal(iso(nextRun('0 8 * * 1-5', at('2026-01-02T09:00:00Z'))), '2026-01-05T08:00:00.000Z'));
test('7 is Sunday', () => assert.equal(iso(nextRun('0 0 * * 7', at('2026-01-01T00:00:00Z'))), '2026-01-04T00:00:00.000Z'));
test('day of month OR day of week', () => assert.equal(iso(nextRun('0 0 13 * 5', at('2026-01-01T00:00:00Z'))), '2026-01-02T00:00:00.000Z'));
test('day of week alone', () => assert.equal(iso(nextRun('0 0 * * 5', at('2026-01-03T00:00:00Z'))), '2026-01-09T00:00:00.000Z'));
test('year rollover', () => assert.equal(iso(nextRun('0 0 1 1 *', at('2026-06-01T00:00:00Z'))), '2027-01-01T00:00:00.000Z'));
test('seconds dropped', () => assert.equal(iso(nextRun('5 * * * *', at('2026-01-01T00:04:59.999Z'))), '2026-01-01T00:05:00.000Z'));
test('invalid', () => { for (const bad of ['* * * *', '60 * * * *', '* 24 * * *', '* * 0 * *', '* * * 13 *', '* * * * 8', '*/0 * * * *', 'a * * * *', '5-1 * * * *'])
  assert.throws(() => nextRun(bad, at('2026-01-01T00:00:00Z')), Error, bad); });
done();
