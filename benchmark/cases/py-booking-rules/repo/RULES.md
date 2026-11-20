# Room bookings

1. A time is an integer number of minutes since midnight. A booking is the half-open interval `[start, end)` with `0 <= start < end <= 1440`; anything else raises `ValueError("bad interval")`.
2. Two bookings of one room conflict when they overlap. Touching is not overlapping: a booking that ends at 600 and one that starts at 600 do not conflict. Bookings of different rooms never conflict.
3. `book(room, start, end)` returns an id `"<room>-<n>"`, `n` counting from 1 for each room and never reused, not even after a cancel. A conflicting booking raises `Conflict`, whose `.booking_id` is the id of the conflicting booking that starts earliest (the lower `n` on a tie).
4. `cancel(booking_id)` removes exactly that booking. An unknown id raises `KeyError(booking_id)`.
5. `bookings(room)` returns `(id, start, end)` tuples sorted by start, then end, then id.
6. `free(room, day_start, day_end, min_len)` returns the free `(start, end)` intervals of the room within `[day_start, day_end)`, sorted, each at least `min_len` minutes long. Bookings that touch or overlap leave no gap between them; a booking that begins before `day_start` or ends after `day_end` still takes its part of the window.
