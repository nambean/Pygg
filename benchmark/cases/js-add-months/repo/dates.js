// Adds n months to a Date (UTC) and returns a new Date.
function addMonths(date, n) {
  const d = new Date(date.getTime());
  d.setUTCMonth(d.getUTCMonth() + n);
  return d;
}
module.exports = { addMonths };
