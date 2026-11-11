// Basic statistics over arrays of numbers.
function sum(values) {
  return values.reduce((a, b) => a + b, 0);
}

function mean(values) {
  return values.length ? sum(values) / values.length : NaN;
}

function median(values) {
  if (!values.length) return NaN;
  const s = [...values].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}

function range(values, digits = 2) {
  if (!values.length) return null;
  return Number((Math.max(...values) - Math.min(...values)).toFixed(digits));
}

module.exports = { sum, mean, median, range };
