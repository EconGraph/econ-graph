#!/bin/bash

# Make the rootful dockerd start only after the EconGraph subvolumes are
# mounted, so that after a host reboot the kind node econ-graph-worker (which
# Docker restarts by itself) bind-mounts the real subvolumes and not the empty
# directories under them. The fstab entries use `nofail`, which doesn't order
# anything before dockerd. Postgres would refuse to start on the empty
# directories (hostPath type Directory), but would stay down until the worker
# container is restarted by hand.
#
# Writes one systemd drop-in, /etc/systemd/system/docker.service.d/10-econgraph-mounts.conf,
# with Wants= and After= on the two mount units (Wants, not Requires: if a
# subvolume fails to mount, dockerd still starts). Runs `systemctl daemon-reload`.
# Does not restart Docker; it takes effect at the next boot.
#
# Idempotent. Needs sudo, except for --dry-run.
#
# Usage: sudo scripts/host/order-docker-after-subvolumes.sh [--dry-run]

set -euo pipefail

DRY_RUN="false"
case "${1:-}" in
    "") ;;
    --dry-run|-n) DRY_RUN="true" ;;
    *) echo "usage: $0 [--dry-run]" >&2; exit 2 ;;
esac

DROPIN=/etc/systemd/system/docker.service.d/10-econgraph-mounts.conf
units=""
for m in /srv/econgraph/data-plane /srv/econgraph/app-prod; do
    units="${units:+$units }$(systemd-escape -p --suffix=mount "$m")"
done
content="# Written by econ-graph scripts/host/order-docker-after-subvolumes.sh.
# The kind node econ-graph-worker bind-mounts these subvolumes; dockerd must
# not start it before they are mounted.
[Unit]
Wants=${units}
After=${units}"

if [ -f "$DROPIN" ] && [ "$(cat "$DROPIN")" = "$content" ]; then
    echo "$DROPIN is already in place; nothing to do"
    exit 0
fi
echo "+ write $DROPIN:"
printf '%s\n' "$content" | sed 's/^/    /'
echo "+ systemctl daemon-reload"
if [ "$DRY_RUN" = "true" ]; then
    echo "(dry run: nothing was changed)"
    exit 0
fi
[ "$(id -u)" = 0 ] || { echo "error: needs root: sudo $0" >&2; exit 1; }
install -d -m 0755 "$(dirname "$DROPIN")"
printf '%s\n' "$content" >"$DROPIN"
systemctl daemon-reload
echo "docker.service now starts after: $(systemctl show docker.service -p After --value | tr ' ' '\n' | grep -F 'srv-econgraph' | tr '\n' ' ')"
