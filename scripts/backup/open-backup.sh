#!/bin/bash
# Open an encrypted bundle in private tmpfs for one operator command.
# Usage: open-backup.sh BUNDLE_DIR -- COMMAND [ARGS containing @BACKUP@]
# Example: open-backup.sh ~/econgraph-backups/STAMP -- \
#   scripts/deploy/restore-kind-databases.sh --from @BACKUP@ --dry-run
# The private identity path is requested on the terminal, never configured for
# a timer. Existing backups are never removed. Disposable plaintext is removed
# when the command exits; tmpfs can still reach swap (use encrypted swap).
set -euo pipefail
umask 077
script_dir="$(dirname "$(readlink -f "$0")")"
die() { echo "error: $*" >&2; exit 1; }
# shellcheck source=scripts/backup/encryption.sh
source "$script_dir/encryption.sh"
[[ $# -ge 3 && "$2" == -- ]] || die "usage: $0 BUNDLE_DIR -- COMMAND [ARGS containing @BACKUP@]"
bundle="$(readlink -e -- "$1")" || die "bundle directory not found"
shift 2
[[ -t 0 ]] || die "opening a backup requires an operator terminal; unattended decryption is refused"
command -v age >/dev/null || die "age is required"
[[ -f "$bundle/VERIFIED" && -s "$bundle/backup.tar.age" ]] || die "not a verified encrypted bundle"
read -r expected filename < "$bundle/SHA256SUMS"
[[ "$expected" =~ ^[0-9a-f]{64}$ && "$filename" == backup.tar.age ]] || die "invalid ciphertext checksum record"
actual="$(sha256sum "$bundle/backup.tar.age")"
[[ "${actual%% *}" == "$expected" ]] || die "ciphertext checksum mismatch"
BACKUP_PLAINTEXT_ROOT="${BACKUP_PLAINTEXT_ROOT:-/dev/shm}"
[[ "$(stat -f -c %T -- "$BACKUP_PLAINTEXT_ROOT")" == tmpfs ]] || die "plaintext staging must be tmpfs"
scratch="$(backup_plaintext_dir)"
trap 'rm -rf -- "$scratch"' EXIT
read -r -p 'Path to your age identity (kept outside agent access): ' identity </dev/tty
[[ -n "$identity" ]] || die "identity path is required"
# Only regular files and directories are accepted, with no absolute paths,
# traversal, symlinks, hard links, devices or duplicate files.
age -d -i "$identity" "$bundle/backup.tar.age" | python3 "$script_dir/unpack-backup.py" "$scratch"
args=()
for arg in "$@"; do args+=("${arg//@BACKUP@/$scratch}"); done
"${args[@]}"
