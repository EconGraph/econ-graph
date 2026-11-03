# shellcheck shell=bash
# shellcheck disable=SC2034 # resolved values are consumed by the sourcing scripts
# Resolve only the upgrade-backup root, without importing backup.env's Docker,
# cluster, or nightly-backup settings into the deploy writer's environment.
load_keycloak_backup_root() {
    local configuration
    configuration="$(
        config_dir="${ECONGRAPH_CONFIG_DIR:-${HOME}/.config/econgraph}"
        if [[ -f "$config_dir/backup.env" ]]; then
            # Same trusted shell-assignment file and precedence as backup/lib.sh.
            # shellcheck disable=SC1091
            source "$config_dir/backup.env" || exit "$?"
        fi
        if [[ -n "${KEYCLOAK_BACKUP_ROOT:-}" ]]; then
            printf 'true\n%s/' "$KEYCLOAK_BACKUP_ROOT"
        else
            printf 'false\n%s/' "${ECONGRAPH_BACKUP_ROOT:-${HOME}/econgraph-backups}/keycloak"
        fi
    )" || return
    # The final slash preserves trailing path newlines in command substitution.
    configuration="${configuration%/}"
    # Read both values from the same config snapshot, without eval or splitting
    # the path on whitespace. The first line is a fixed boolean, never path data.
    KEYCLOAK_BACKUP_ROOT_REQUIRED="${configuration%%$'\n'*}"
    KEYCLOAK_BACKUP_ROOT_RESOLVED="${configuration#*$'\n'}"
}
