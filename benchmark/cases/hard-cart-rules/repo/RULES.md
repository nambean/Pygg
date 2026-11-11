# Cart rules

1. A line's amount is its unit price times its quantity, in cents.
2. Percentage coupons apply before fixed-amount coupons. Each coupon applies to the result of the one before it.
   A percentage discount is rounded half up to the cent. The total never goes below zero.
3. Shipping is 499 cents, free when the subtotal after coupons is 5000 cents or more.
4. Tax is computed per line, on the line's share of the discounted subtotal, rounded half up to the cent:
   a line's share is its amount times (discounted subtotal / subtotal before coupons). Shipping is not taxed.
5. The total is the discounted subtotal plus shipping plus the sum of the per-line taxes.
