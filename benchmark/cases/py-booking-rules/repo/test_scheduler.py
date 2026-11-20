"""Checks scheduler.py against RULES.md."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from scheduler import Conflict, Scheduler  # noqa: E402


def raises(fn, kind, check=None):
    try:
        fn()
    except kind as e:
        if check:
            check(e)
        return
    raise AssertionError(f"no {kind.__name__}")


def names(expected):
    def check(e):
        assert e.booking_id == expected, f"Conflict names {e.booking_id}, wanted {expected}"
    return check


s = Scheduler()

# 1. intervals
for start, end in [(600, 600), (700, 600), (-1, 10), (0, 1441)]:
    raises(lambda: s.book("A", start, end), ValueError, lambda e: e.args == ("bad interval",))
assert s.bookings("A") == []
assert s.book("A", 0, 1440) == "A-1"
s.cancel("A-1")

# 2. touching is not a conflict; other rooms never conflict
assert s.book("A", 540, 600) == "A-2"
assert s.book("A", 600, 660) == "A-3"
assert s.book("A", 480, 540) == "A-4"
assert s.book("B", 540, 660) == "B-1"

# 3. the earliest-starting conflict is named
raises(lambda: s.book("A", 500, 650), Conflict, names("A-4"))
raises(lambda: s.book("A", 610, 615), Conflict, names("A-3"))
assert s.book("A", 900, 960) == "A-5"
assert s.book("A", 1000, 1010) == "A-6"
assert s.book("A", 990, 1000) == "A-7"
raises(lambda: s.book("A", 950, 1005), Conflict, names("A-5"))

# 4. cancel exactly that booking; ids are never reused
s.cancel("A-6")
raises(lambda: s.cancel("A-6"), KeyError, lambda e: e.args == ("A-6",))
raises(lambda: s.cancel("C-1"), KeyError)
assert [b[0] for b in s.bookings("B")] == ["B-1"], "cancelling A-6 left B alone"
assert s.book("A", 1000, 1010) == "A-8", "a cancelled id is not reused"
s.cancel("B-1")
assert s.bookings("B") == []
assert [b[0] for b in s.bookings("A")] == ["A-4", "A-2", "A-3", "A-5", "A-7", "A-8"]

# 5. sorted by start, then end, then id
t = Scheduler()
t.book("R", 300, 400)
t.book("R", 100, 300)
t.book("R", 0, 100)
assert t.bookings("R") == [("R-3", 0, 100), ("R-2", 100, 300), ("R-1", 300, 400)], t.bookings("R")

# 6. free time
assert s.free("A", 480, 1080, 1) == [(660, 900), (960, 990), (1010, 1080)], s.free("A", 480, 1080, 1)
assert s.free("A", 480, 1080, 30) == [(660, 900), (960, 990), (1010, 1080)], "30 minutes is long enough"
assert s.free("A", 480, 1080, 31) == [(660, 900), (1010, 1080)]
assert s.free("A", 550, 950, 0) == [(660, 900)], "bookings across the window's edges take their part"
assert s.free("A", 0, 1440, 60) == [(0, 480), (660, 900), (1010, 1440)]
assert s.free("B", 600, 700, 10) == [(600, 700)]
assert s.free("A", 560, 640, 1) == []

print("booking rules: ok")
