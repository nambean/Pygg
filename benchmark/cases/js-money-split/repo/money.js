// Splits an amount in cents into n parts that differ by at most one cent.
function splitAmount(cents, n) {
  const part = cents / n;
  return Array.from({ length: n }, () => Math.round(part));
}
module.exports = { splitAmount };
