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
    # Exercise recipient parsing before any dump or downtime. No identity is used.
    printf '' | age -r "$BACKUP_AGE_RECIPIENT" >/dev/null || die "invalid age recipient"
    BACKUP_PLAINTEXT_ROOT="${BACKUP_PLAINTEXT_ROOT:-/dev/shm}"
    [[ "$(stat -f -c %T -- "$BACKUP_PLAINTEXT_ROOT")" == tmpfs ]] ||
        die "BACKUP_PLAINTEXT_ROOT must be tmpfs; plaintext staging on disk is refused"
}

backup_plaintext_dir() {
    (umask 077; mktemp -d "${BACKUP_PLAINTEXT_ROOT:-/dev/shm}/econgraph-backup.XXXXXXXX")
}

# The source is temporary staging, never an existing backup. Publish only after
# every write succeeds. Failed ciphertext is kept without a VERIFIED marker.
backup_seal_dir() {
    local source="$1" output="$2" partial="${2}.partial"
    [[ ! -e "$output" && ! -e "$partial" ]] || die "backup output already exists: $output"
    (umask 077
        mkdir -p -- "$(dirname "$output")" || exit
        mkdir -- "$partial" || exit
        # An interrupted/failed final rename must not leave an unpublished
        # directory eligible for off-site selection.
        trap 'rm -f -- "$partial/VERIFIED"' EXIT
        tar -C "$source" -cf - . | age -r "$BACKUP_AGE_RECIPIENT" -o "$partial/backup.tar.age" || exit
        (cd "$partial" || exit; sha256sum backup.tar.age > SHA256SUMS) || exit
        date -u +%Y-%m-%dT%H:%M:%SZ > "$partial/VERIFIED" || exit
        mv -T -n -- "$partial" "$output" || exit
        [[ ! -e "$partial" ]] || exit 1
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
