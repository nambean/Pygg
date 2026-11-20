'use strict';
const TAX_RATE = 0.08;
function subtotal(lines) { return lines.reduce((s, l) => s + l.amountCents, 0); }
function tax(cents) { return Math.round(cents * TAX_RATE); }
module.exports = { subtotal, tax, TAX_RATE };
