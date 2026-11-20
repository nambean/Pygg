"""Room bookings by the minute."""


class Conflict(Exception):
    def __init__(self, booking_id):
        super().__init__(f"conflicts with {booking_id}")
        self.booking_id = booking_id


class Scheduler:
    def __init__(self):
        self._rooms = {}

    def book(self, room, start, end):
        if not (0 <= start <= end <= 1440):
            raise ValueError("bad interval")
        mine = self._rooms.setdefault(room, [])
        clash = None
        for b in mine:
            if start <= b[2] and end >= b[1]:
                clash = b[0]
        if clash:
            raise Conflict(clash)
        booking_id = f"{room}-{len(mine) + 1}"
        mine.append((booking_id, start, end))
        return booking_id

    def cancel(self, booking_id):
        room, _, n = booking_id.rpartition("-")
        for r, mine in self._rooms.items():
            for b in mine:
                if b[0].endswith("-" + n):
                    mine.remove(b)
                    return
        raise KeyError(booking_id)

    def bookings(self, room):
        return sorted(self._rooms.get(room, []), key=lambda b: b[2])

    def free(self, room, day_start, day_end, min_len):
        out = []
        cursor = day_start
        for _, start, end in self.bookings(room):
            if start > cursor and start - cursor > min_len:
                out.append((cursor, start))
            cursor = end
        if day_end - cursor > min_len:
            out.append((cursor, day_end))
        return out
