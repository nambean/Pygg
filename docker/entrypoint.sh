#!/bin/sh
# Starts the session bus and an unlocked keyring (so `docker exec … pygg` reaches the same ones through
# DBUS_SESSION_BUS_ADDRESS), relays 0.0.0.0:8742 to the gateway's 127.0.0.1:8741, then runs pygg.
set -e
mkdir -p "$HOME"
rm -f /tmp/pygg-dbus.sock
dbus-daemon --session --fork --nopidfile --address="$DBUS_SESSION_BUS_ADDRESS"
printf '%s' "${PYGG_KEYRING_PASSWORD:-pygg}" | gnome-keyring-daemon --unlock --components=secrets >/dev/null
socat TCP-LISTEN:8742,fork,reuseaddr,bind=0.0.0.0 TCP:127.0.0.1:8741 &
exec pygg "$@"
