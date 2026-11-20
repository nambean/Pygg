"""Checks config.py against SPEC.md and the files in fixtures/."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from config import ConfigError, load  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))


def fx(name):
    return os.path.join(HERE, "fixtures", name)


def raises(name, message, env=None):
    try:
        load(fx(name), env)
    except ConfigError as e:
        assert str(e) == message, f"{name}: got {str(e)!r}, wanted {message!r}"
        return
    raise AssertionError(f"{name}: no ConfigError, wanted {message!r}")


# Sections, comments and plain values still work on their own.
assert load(fx("base.cfg"), {}) == {
    "name": "shop", "host": "localhost", "log.level": "info", "log.dir": "/var/log/shop",
}, load(fx("base.cfg"), {})

# Includes, overrides wherever they were read, sections reset and restored, interpolation after reading.
got = load(fx("app.cfg"), {"PREFIX": "api", "DB_USER": "ada"})
assert got == {
    "name": "shop",
    "host": "127.0.0.1",
    "log.level": "debug",
    "log.dir": "/var/log/shop",
    "server.port": "8080",
    "server.url": "http://127.0.0.1:8080/api",
    "db.url": "postgres://ada@127.0.0.1/shop",
    "db.user": "ada",
    "db.pool": "5",
    "server.timeout": "30",
}, got
# host is overridden after base.cfg and db.cfg uses the overriding value; after db.cfg the section is [server] again,
# so timeout is server.timeout, until [log] overrides log.level.

# Dollars.
assert load(fx("dollars.cfg")) == {"price": "$5 and ${not.a.key}", "cost": "$5 and ${not.a.key}!"}, load(fx("dollars.cfg"))

# Errors, with exact messages.
raises("app.cfg", "undefined: env:PREFIX", {"DB_USER": "ada"})
raises("app.cfg", "undefined: env:PREFIX")
raises("missing.cfg", "missing include: nowhere/else.cfg")
raises("cycle_a.cfg", "include cycle: cycle_a.cfg -> cycle_b.cfg -> cycle_a.cfg")
raises("cycle_b.cfg", "include cycle: cycle_b.cfg -> cycle_a.cfg -> cycle_b.cfg")
raises("undefined.cfg", "undefined: b")
raises("interp_cycle.cfg", "interpolation cycle: x -> y -> z -> x")

print("config includes: ok")
