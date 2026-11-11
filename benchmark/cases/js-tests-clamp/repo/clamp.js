// Limits x to the range [lo, hi]. NaN stays NaN. Throws a RangeError when lo > hi.
function clamp(x, lo, hi) {
  if (lo > hi) throw new RangeError('lo must not be above hi');
  if (Number.isNaN(x)) return NaN;
  return Math.min(Math.max(x, lo), hi);
}
module.exports = { clamp };
