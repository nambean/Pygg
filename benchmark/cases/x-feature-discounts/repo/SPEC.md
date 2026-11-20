# Discounts

All amounts are integer cents.

## Volume discounts

A line's volume discount depends on its product's category and the line's quantity:

| Category | Quantity | Discount |
| --- | --- | --- |
| paper | 10 to 49 | 10% |
| paper | 50 or more | 15% |
| ink | 3 or more | 5% |
| other categories | any | none |

A line's discount is its percentage of the line amount (unit price times quantity), rounded to the cent **half to even** (2.5 cents is 2, 3.5 cents is 4).

## Coupons

`render(cart, { coupon })` takes at most one code. Codes are matched case-insensitively after trimming. Any other code throws an `Error` whose message is `unknown coupon: <code as given>`.

- **INK20**: 20% off every ink line, rounded half to even per line, **instead of** that line's volume discount (a line gets whichever of the two is larger; equal counts as the coupon). It does nothing else.
- **SAVE5**: 500 cents off the order, once, after every line discount, only when the order's amount after line discounts is at least 5000 cents. If it would make the order negative, it takes only what is left.

## Tax and totals

- `subtotalCents` is the sum of line amounts, before discounts (unchanged from today).
- `discountCents` is the sum of every discount.
- Tax is 8% of `subtotalCents - discountCents`, rounded **half up** to the cent.
- `totalCents = subtotalCents - discountCents + taxCents`.

## Discount lines

`discounts` lists, in this order: one entry per line with a volume discount it kept (`{ kind: 'volume', sku, amountCents }`), in the order of the cart's lines; then one per line where INK20 won (`{ kind: 'coupon', code: 'INK20', sku, amountCents }`), in cart order; then SAVE5 when it applied (`{ kind: 'coupon', code: 'SAVE5', amountCents }`). A discount of 0 cents is not listed. With no discount at all, `discounts` is an empty array and `discountCents` is 0.
