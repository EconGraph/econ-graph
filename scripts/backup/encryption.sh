# shellcheck shell=bash
# Public-key-only backup storage. Never source or store an age identity here.

backup_encryption_preflight() {
    local config="${ECONGRAPH_CONFIG_DIR:-$HOME/.config/econgraph}/backup.env"
    if [[ -z "${BACKUP_AGE_RECIPIENT:-}" && -f "$config" ]]; then
        BACKUP_AGE_RECIPIENT="$(
            # shellcheck disable=SC1090
            source "$config"
            printf '%s' "${BACKUP_AGE_RECIPIENT:-}"
        )"
    fi
    [[ "${BACKUP_AGE_RECIPIENT:-}" =~ ^age1[0-9a-z]+$ ]] ||
        die "set BACKUP_AGE_RECIPIENT to the maintainer's public age recipient; no plaintext fallback"
    command -v age >/dev/null || die "age is required for encrypted backups"
    # age-keygen makes the throwaway key that backup_seal_dir decrypts with.
    command -v age-keygen >/dev/null || die "age-keygen is required to check that encrypted backups decrypt"
    command -v python3 >/dev/null || die "python3 is required to check that encrypted backups decrypt"
    # Exercise recipient parsing before any dump or downtime. No identity is used.
    printf '' | age -r "$BACKUP_AGE_RECIPIENT" >/dev/null || die "invalid age recipient"
    backup_recipient_proven "$BACKUP_AGE_RECIPIENT" ||
        die "BACKUP_AGE_RECIPIENT has not been proven to belong to a held identity; no backup was taken"
    BACKUP_PLAINTEXT_ROOT="${BACKUP_PLAINTEXT_ROOT:-/dev/shm}"
    [[ "$(stat -f -c %T -- "$BACKUP_PLAINTEXT_ROOT")" == tmpfs ]] ||
        die "BACKUP_PLAINTEXT_ROOT must be tmpfs; plaintext staging on disk is refused"
}

# backup_recipient_proven RECIPIENT: whether the maintainer has shown that they
# hold the identity for RECIPIENT. This is the hook for the key-custody
# decision in https://linear.app/econgraph/issue/ECO-718 (shared with PR #543):
# a one-time challenge decrypted with the private identity, recorded as proven.
# Until that check is chosen every well-formed recipient is accepted, so the
# throwaway-key check in backup_seal_dir proves the ciphertext decrypts but
# not that the maintainer can decrypt it.
backup_recipient_proven() {
    return 0
}

# python3 -c "$BACKUP_HASH_THROUGH" FILE: copy stdin to stdout and write the
# stream's SHA-256 to FILE.
BACKUP_HASH_THROUGH='
import hashlib, sys
digest = hashlib.sha256()
for block in iter(lambda: sys.stdin.buffer.read(1 << 20), b""):
    digest.update(block)
    sys.stdout.buffer.write(block)
sys.stdout.buffer.flush()
with open(sys.argv[1], "w") as out:
    out.write(digest.hexdigest() + "\n")
'

# backup_staging_lock: serialize every writer that stages plaintext in
# BACKUP_PLAINTEXT_ROOT, whatever its destination root, so one writer's
# tmpfs room check is not spent by another. Call after
# backup_encryption_preflight. Taken once per process (later calls return at
# once) and held until backup_staging_unlock or exit.
# BACKUP_LOCK_NONBLOCK=true fails at once if another writer holds it;
# otherwise this says who holds it and waits up to BACKUP_LOCK_TIMEOUT
# seconds (default 3600), then refuses.
# A script run by a holder that exported BACKUP_STAGING_LOCK_FD (and so
# inherited that open lock) reuses it: open-backup.sh around a restore, a
# Keycloak restore around its safety backup.
backup_staging_lock() {
    local lock fd timeout="${BACKUP_LOCK_TIMEOUT:-3600}"
    lock="${BACKUP_PLAINTEXT_ROOT:-/dev/shm}/.econgraph-backup-$(id -u).lock"
    if [[ -n "${BACKUP_STAGING_LOCK_FD:-}" ]]; then
        [[ "$BACKUP_STAGING_LOCK_FD" =~ ^[0-9]+$ && /dev/fd/$BACKUP_STAGING_LOCK_FD -ef "$lock" ]] && return 0
        # Not the open lock of this staging root (stale or foreign): ignore it.
        BACKUP_STAGING_LOCK_FD=""
    fi
    [[ ! -L "$lock" ]] || die "$lock is a symlink; refusing to use it as the backup lock"
    exec {fd}>>"$lock" || die "could not open the backup lock $lock"
    [[ -O "$lock" && ! -L "$lock" ]] || die "$lock is not a lock file owned by $(id -un); refusing it"
    if ! flock -n "$fd"; then
        if [[ "${BACKUP_LOCK_NONBLOCK:-false}" == true ]]; then
            die "another backup is staging plaintext in ${BACKUP_PLAINTEXT_ROOT:-/dev/shm} ($(tr '\n' ' ' <"$lock"))"
        fi
        echo "Waiting up to ${timeout}s for another backup staging plaintext in ${BACKUP_PLAINTEXT_ROOT:-/dev/shm} to finish: $(tr '\n' ' ' <"$lock")" >&2
        flock -w "$timeout" "$fd" ||
            die "another backup still holds $lock after ${timeout}s ($(tr '\n' ' ' <"$lock")); nothing was stopped or changed"
    fi
    # Only the holder writes here; waiters read it to say whom they wait for.
    : >"$lock" && printf 'pid=%s command=%s since=%s\n' "$$" "${0##*/}" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"$lock" || true
    BACKUP_STAGING_LOCK_FD="$fd"
}

backup_staging_unlock() {
    [[ -n "${BACKUP_STAGING_LOCK_FD:-}" ]] || return 0
    exec {BACKUP_STAGING_LOCK_FD}>&-
    BACKUP_STAGING_LOCK_FD=""
}

backup_plaintext_dir() {
    (umask 077; mktemp -d "${BACKUP_PLAINTEXT_ROOT:-/dev/shm}/econgraph-backup.XXXXXXXX")
}

# The source is temporary staging, never an existing backup. Publish only after
# every write succeeds. Failed ciphertext is kept without a VERIFIED marker.
#
# The bundle is also encrypted to a throwaway age key generated in tmpfs. The
# published file is decrypted with that key and the decrypted tar stream must
# hash the same as the stream that was encrypted, so VERIFIED means the
# ciphertext on disk is complete and decrypts. The key is destroyed before
# returning; it cannot open the bundle later.
backup_seal_dir() {
    local source="$1" output="$2" partial="${2}.partial" recipient="${BACKUP_AGE_RECIPIENT:-}"
    [[ "$recipient" =~ ^age1[0-9a-z]+$ ]] || die "no valid BACKUP_AGE_RECIPIENT; run backup_encryption_preflight first"
    [[ ! -e "$output" && ! -e "$partial" ]] || die "backup output already exists: $output"
    # Every step below checks its own status; errexit is off so that a failed
    # pipeline stage is reported here rather than ending the subshell early.
    (umask 077
        set +e -o pipefail
        keydir=""
        # An interrupted/failed final rename must not leave an unpublished
        # directory eligible for off-site selection. The throwaway key never
        # outlives this subshell.
        trap 'rm -f -- "$partial/VERIFIED"; [[ -z "$keydir" ]] || rm -rf -- "$keydir"' EXIT
        keydir="$(mktemp -d "${BACKUP_PLAINTEXT_ROOT:-/dev/shm}/econgraph-verify-key.XXXXXXXX")" || exit
        [[ "$(stat -f -c %T -- "$keydir")" == tmpfs ]] || {
            echo "error: $keydir is not on tmpfs; refusing to write the throwaway key there" >&2
            exit 1
        }
        age-keygen -o "$keydir/key" 2>/dev/null || exit
        check_recipient="$(age-keygen -y "$keydir/key")" || exit
        [[ "$check_recipient" =~ ^age1[0-9a-z]+$ ]] || exit 1
        # The .partial directory is made last, once the key is ready, so a
        # failed setup leaves nothing that would block the next run.
        mkdir -p -- "$(dirname "$output")" || exit
        mkdir -- "$partial" || exit
        # Hash the tar stream on its way to age. A plain pipeline stage: if
        # either neighbour fails it fails too, and nothing can block on it.
        # Each stage's status is checked: a file tar could not read (tar
        # exits 2 but still writes the rest) must never be sealed.
        tar -C "$source" -cf - . | python3 -I -c "$BACKUP_HASH_THROUGH" "$keydir/encrypted.sha256" |
            age -r "$recipient" -r "$check_recipient" -o "$partial/backup.tar.age"
        stages=("${PIPESTATUS[@]}")
        [[ "${stages[*]}" == "0 0 0" ]] || {
            echo "error: sealing $source failed (tar ${stages[0]}, hash ${stages[1]}, age ${stages[2]}); not publishing it" >&2
            exit 1
        }
        sync -- "$partial/backup.tar.age" || exit
        age -d -i "$keydir/key" "$partial/backup.tar.age" | sha256sum > "$keydir/decrypted.sha256" || {
            echo "error: $partial/backup.tar.age does not decrypt; not marking it VERIFIED" >&2
            exit 1
        }
        read -r encrypted_hash _ < "$keydir/encrypted.sha256" || exit
        read -r decrypted_hash _ < "$keydir/decrypted.sha256" || exit
        [[ "$encrypted_hash" =~ ^[0-9a-f]{64}$ && "$decrypted_hash" == "$encrypted_hash" ]] || {
            echo "error: $partial/backup.tar.age decrypts to a different stream than was encrypted; not marking it VERIFIED" >&2
            exit 1
        }
        rm -rf -- "$keydir" || exit
        (cd "$partial" || exit; sha256sum backup.tar.age > SHA256SUMS) || exit
        date -u +%Y-%m-%dT%H:%M:%SZ > "$partial/VERIFIED" || exit
        # The ciphertext, its checksum and the marker reach the disk before
        # the bundle is published, and the rename before anything relies on it.
        sync -- "$partial/SHA256SUMS" "$partial/VERIFIED" "$partial" || exit
        mv -T -n -- "$partial" "$output" || exit
        # mv -n exits 0 without moving anything when the output appeared
        # meanwhile (coreutils 9.7), so check that the rename happened.
        [[ ! -e "$partial" ]] || {
            echo "error: $output appeared before $partial could be published; nothing was replaced" >&2
            exit 1
        }
        sync -- "$(dirname "$output")" || exit
    )
}

# In-process migration consumers can use the same verified tmpfs staging. On a
# later invocation the public bundle must first be opened by its operator.
backup_source_dir() {
    if [[ "$1" == "${BACKUP_ACTIVE_OUTPUT:-}" && -n "${BACKUP_ACTIVE_DIR:-}" ]]; then
        printf '%s\n' "$BACKUP_ACTIVE_DIR"
    else
        [[ ! -f "$1/backup.tar.age" ]] || die "encrypted bundle: use scripts/backup/open-backup.sh $1 -- COMMAND @BACKUP@ at an operator terminal"
        printf '%s\n' "$1"
    fi
}

# Keep resume-state identity stable while the opened payload changes tmpfs path.
backup_original_dir() {
    if [[ -n "${BACKUP_ACTIVE_OUTPUT:-}" && "$1" == "${BACKUP_ACTIVE_DIR:-}" ]]; then
        printf '%s\n' "$BACKUP_ACTIVE_OUTPUT"
    else
        printf '%s\n' "$1"
    fi
}
