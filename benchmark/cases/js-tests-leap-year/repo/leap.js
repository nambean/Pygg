// Gregorian leap years. Throws a TypeError for anything but an integer.
function isLeapYear(year) {
  if (!Number.isInteger(year)) throw new TypeError('year must be an integer');
  return (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
}
module.exports = { isLeapYear };
