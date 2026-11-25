#!/bin/sh
# Builds pygg:local from this repository and runs the gateway on 127.0.0.1:8741 (interface preview on :1431).
#   ./run.sh build   build the image
#   ./run.sh up      (re)start the container
#   ./run.sh token   make a Pygg API key for this Mac, saved to docker/token
#   ./run.sh pygg …  run a pygg command inside it (e.g. ./run.sh pygg status)
set -e
here=$(cd "$(dirname "$0")" && pwd)
repo=${PYGG_REPO:-$here/..}
case "${1:-up}" in
  build)
    docker build -f "$here/Dockerfile" --build-context docker="$here" -t pygg:local "$repo"
    docker build -f "$here/Dockerfile.ui" -t pygg-ui:local "$repo" ;;
  up)
    docker rm -f pygg >/dev/null 2>&1 || true
    docker run -d --name pygg --restart unless-stopped \
      -p 127.0.0.1:8741:8742 -p "[::1]:8741:8742" -v pygg-data:/data pygg:local
    docker rm -f pygg-ui >/dev/null 2>&1 || true
    docker run -d --name pygg-ui --restart unless-stopped -p 127.0.0.1:1431:80 pygg-ui:local >/dev/null
    sleep 2
    curl -fsS http://127.0.0.1:8741/health && echo
    echo "Gateway API: http://127.0.0.1:8741/v1  ·  Interface (demo data): http://127.0.0.1:1431" ;;
  pygg)
    shift; docker exec -it pygg pygg "$@" ;;
  token)
    # A Pygg API key for a tool on this Mac, saved to docker/token (git-ignored) and never printed.
    (umask 077; docker exec pygg pygg gateway create-client --name "${2:-mac}" | grep -oE 'str-[0-9a-f]+' > "$here/token")
    echo "Saved to $here/token" ;;
  logs)
    docker logs -f pygg ;;
  down)
    docker rm -f pygg pygg-ui ;;
  *)
    echo "usage: $0 build|up|token [NAME]|pygg …|logs|down" >&2; exit 2 ;;
esac
