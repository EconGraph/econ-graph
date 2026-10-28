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
    (umask 077
        set -o pipefail
        mkdir -p -- "$(dirname "$output")" || exit
        mkdir -- "$partial" || exit
        keydir="$(mktemp -d "${BACKUP_PLAINTEXT_ROOT:-/dev/shm}/econgraph-verify-key.XXXXXXXX")" || exit
        # An interrupted/failed final rename must not leave an unpublished
        # directory eligible for off-site selection. The throwaway key never
        # outlives this subshell.
        trap 'rm -f -- "$partial/VERIFIED"; rm -rf -- "$keydir"' EXIT
        age-keygen -o "$keydir/key" 2>/dev/null || exit
        check_recipient="$(age-keygen -y "$keydir/key")" || exit
        [[ "$check_recipient" =~ ^age1[0-9a-z]+$ ]] || exit 1
        # Hash the tar stream on its way to age. A plain pipeline stage: if
        # either neighbour fails it fails too, and nothing can block on it.
        tar -C "$source" -cf - . | python3 -c "$BACKUP_HASH_THROUGH" "$keydir/encrypted.sha256" |
            age -r "$recipient" -r "$check_recipient" -o "$partial/backup.tar.age" || exit
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
        [[ ! -e "$partial" ]] || exit 1
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
