#!/bin/bash

# Write host disk metrics for node-exporter's textfile collector
# (docs/deployment/HOST_DISK_MONITORING.md). Installed by
# scripts/host/install-host-disk-exporter.sh and run every minute by
# econgraph-host-disk-check.timer.
#
# For each --path ROLE=PATH it reports the size and free space of the
# filesystem holding PATH; --mount ROLE=PATH also requires PATH to be a
# mount point, so an unmounted store is reported missing rather than as the
# filesystem underneath it. With --libvirt it reports every active libvirt
# storage pool (qemu:///system): the pool filesystem's size and free space,
# and the virtual and allocated sizes of the volumes in it. Thin qcow2 disks
# can grow to their virtual size, so a pool whose volumes could still grow by
# more than its free space is overcommitted.
#
# Metrics (all gauges):
#   econgraph_host_filesystem_present{role}                 1 if measured, else 0
#   econgraph_host_filesystem_size_bytes{role,...}          filesystem size
#   econgraph_host_filesystem_avail_bytes{role,...}         free to unprivileged users
#     (path roles add mountpoint, device and fstype; libvirt_pool adds pool)
#   econgraph_libvirt_up                                     1 if virsh answered
#   econgraph_libvirt_pool_volumes{pool}                     number of volumes
#   econgraph_libvirt_pool_volumes_capacity_bytes{pool}      sum of virtual sizes
#   econgraph_libvirt_pool_volumes_allocation_bytes{pool}    sum of allocated sizes
#   econgraph_host_disk_check_timestamp_seconds              when this file was written
#
# It only reads: nothing here changes a pool, volume or filesystem
# (pool-refresh only re-reads the pool directory into libvirt's cache).
# The file is written to a temporary name and renamed, so node-exporter never
# reads half of it.
#
# Usage: host-disk-check.sh --output FILE [--path ROLE=PATH]... [--mount ROLE=PATH]... [--libvirt]

set -euo pipefail

VIRSH_URI="qemu:///system"

usage() {
    echo "usage: $0 --output FILE [--path ROLE=PATH]... [--mount ROLE=PATH]... [--libvirt]" >&2
    exit 2
}

output=""
libvirt="false"
specs=()
while [ "$#" -gt 0 ]; do
    case "$1" in
        --output) [ "$#" -ge 2 ] || usage; output="$2"; shift 2 ;;
        --path | --mount)
            [ "$#" -ge 2 ] || usage
            case "$2" in
                [a-z]*=/*) ;;
                *) echo "error: $1 wants ROLE=/absolute/path, got '$2'" >&2; exit 2 ;;
            esac
            role="${2%%=*}"
            if ! [[ "$role" =~ ^[a-z][a-z0-9_]*$ ]]; then
                echo "error: role '$role' must be lower-case letters, digits and _" >&2
                exit 2
            fi
            specs+=("${1#--} $2")
            shift 2
            ;;
        --libvirt) libvirt="true"; shift ;;
        *) usage ;;
    esac
done
[ -n "$output" ] || usage

# Escape a Prometheus label value.
esc() {
    local v="${1//\\/\\\\}"
    v="${v//\"/\\\"}"
    printf '%s' "${v//$'\n'/\\n}"
}

# Field <name> of `virsh ... --bytes` output on stdin ("Capacity:  123 bytes").
field() { awk -v k="$1:" '$1 == k { print $2; exit }'; }

is_count() { [[ "$1" =~ ^[0-9]+$ ]]; }

present="" size="" avail="" lv="" pools=""

measure_path() {
    local kind="$1" role="$2" path="$3" line src fstype bytes free target
    if [ "$kind" = mount ] && ! mountpoint -q -- "$path"; then
        echo "warning: $role: $path is not a mount point" >&2
        present+="econgraph_host_filesystem_present{role=\"$(esc "$role")\",path=\"$(esc "$path")\"} 0"$'\n'
        return
    fi
    # GNU df, one line per filesystem; the mount point is last and may hold spaces.
    if ! line="$(df -B1 --output=source,fstype,size,avail,target -- "$path" 2>/dev/null | tail -n +2 | head -n 1)" \
        || [ -z "$line" ]; then
        echo "warning: $role: cannot stat the filesystem holding $path" >&2
        present+="econgraph_host_filesystem_present{role=\"$(esc "$role")\",path=\"$(esc "$path")\"} 0"$'\n'
        return
    fi
    read -r src fstype bytes free target <<<"$line"
    if ! is_count "$bytes" || ! is_count "$free"; then
        echo "warning: $role: unexpected df output: $line" >&2
        present+="econgraph_host_filesystem_present{role=\"$(esc "$role")\",path=\"$(esc "$path")\"} 0"$'\n'
        return
    fi
    local labels
    labels="role=\"$(esc "$role")\",mountpoint=\"$(esc "$target")\",device=\"$(esc "$src")\",fstype=\"$(esc "$fstype")\""
    present+="econgraph_host_filesystem_present{role=\"$(esc "$role")\",path=\"$(esc "$path")\"} 1"$'\n'
    size+="econgraph_host_filesystem_size_bytes{$labels} $bytes"$'\n'
    avail+="econgraph_host_filesystem_avail_bytes{$labels} $free"$'\n'
}

measure_pool() {
    local pool="$1" info cap free vols path vinfo vcap valloc n=0 sum_cap=0 sum_alloc=0 label
    label="pool=\"$(esc "$pool")\""
    virsh -c "$VIRSH_URI" pool-refresh "$pool" >/dev/null 2>&1 || true
    if ! info="$(virsh -c "$VIRSH_URI" pool-info --bytes "$pool" 2>/dev/null)"; then
        echo "warning: pool $pool: pool-info failed" >&2
        return 1
    fi
    cap="$(field Capacity <<<"$info")"
    free="$(field Available <<<"$info")"
    if ! is_count "$cap" || ! is_count "$free"; then
        echo "warning: pool $pool: unexpected pool-info output" >&2
        return 1
    fi
    # vol-list prints a two-line header, then "<name> <path>"; the path is the
    # last field and identifies the volume unambiguously.
    if ! vols="$(virsh -c "$VIRSH_URI" vol-list --pool "$pool" 2>/dev/null)"; then
        echo "warning: pool $pool: vol-list failed" >&2
        return 1
    fi
    while IFS= read -r path; do
        [ -n "$path" ] || continue
        if ! vinfo="$(virsh -c "$VIRSH_URI" vol-info --bytes "$path" 2>/dev/null)"; then
            echo "warning: pool $pool: vol-info failed for $path" >&2
            return 1
        fi
        vcap="$(field Capacity <<<"$vinfo")"
        valloc="$(field Allocation <<<"$vinfo")"
        if ! is_count "$vcap" || ! is_count "$valloc"; then
            echo "warning: pool $pool: unexpected vol-info output for $path" >&2
            return 1
        fi
        n=$((n + 1))
        sum_cap=$((sum_cap + vcap))
        sum_alloc=$((sum_alloc + valloc))
    done < <(awk 'NR > 2 && NF >= 2 { print $NF }' <<<"$vols")
    present+="econgraph_host_filesystem_present{role=\"libvirt_pool\",$label} 1"$'\n'
    size+="econgraph_host_filesystem_size_bytes{role=\"libvirt_pool\",$label} $cap"$'\n'
    avail+="econgraph_host_filesystem_avail_bytes{role=\"libvirt_pool\",$label} $free"$'\n'
    pools+="econgraph_libvirt_pool_volumes{$label} $n"$'\n'
    pools+="econgraph_libvirt_pool_volumes_capacity_bytes{$label} $sum_cap"$'\n'
    pools+="econgraph_libvirt_pool_volumes_allocation_bytes{$label} $sum_alloc"$'\n'
}

for spec in ${specs[@]+"${specs[@]}"}; do
    kind="${spec%% *}"
    rest="${spec#* }"
    measure_path "$kind" "${rest%%=*}" "${rest#*=}"
done

if [ "$libvirt" = true ]; then
    if names="$(virsh -c "$VIRSH_URI" pool-list --name 2>/dev/null)"; then
        up=1
        while IFS= read -r pool; do
            [ -n "$pool" ] || continue
            measure_pool "$pool" || {
                up=0
                present+="econgraph_host_filesystem_present{role=\"libvirt_pool\",pool=\"$(esc "$pool")\"} 0"$'\n'
            }
        done <<<"$names"
    else
        echo "warning: virsh pool-list failed" >&2
        up=0
    fi
    lv="econgraph_libvirt_up $up"$'\n'
fi

{
    echo "# HELP econgraph_host_filesystem_present 1 if the host disk check measured this filesystem, 0 if it could not."
    echo "# TYPE econgraph_host_filesystem_present gauge"
    printf '%s' "$present"
    echo "# HELP econgraph_host_filesystem_size_bytes Size of the filesystem holding a monitored host path or libvirt pool."
    echo "# TYPE econgraph_host_filesystem_size_bytes gauge"
    printf '%s' "$size"
    echo "# HELP econgraph_host_filesystem_avail_bytes Space available to unprivileged users on that filesystem."
    echo "# TYPE econgraph_host_filesystem_avail_bytes gauge"
    printf '%s' "$avail"
    if [ "$libvirt" = true ]; then
        echo "# HELP econgraph_libvirt_up 1 if virsh answered for every active storage pool."
        echo "# TYPE econgraph_libvirt_up gauge"
        printf '%s' "$lv"
        for m in volumes volumes_capacity_bytes volumes_allocation_bytes; do
            case "$m" in
                volumes) help="Number of volumes in the libvirt storage pool." ;;
                volumes_capacity_bytes) help="Sum of the virtual sizes of the pool's volumes (what thin disks can grow to)." ;;
                volumes_allocation_bytes) help="Sum of the space the pool's volumes occupy now." ;;
            esac
            echo "# HELP econgraph_libvirt_pool_$m $help"
            echo "# TYPE econgraph_libvirt_pool_$m gauge"
            grep "^econgraph_libvirt_pool_$m{" <<<"$pools" || true
        done
    fi
    echo "# HELP econgraph_host_disk_check_timestamp_seconds When the host disk check last wrote this file."
    echo "# TYPE econgraph_host_disk_check_timestamp_seconds gauge"
    echo "econgraph_host_disk_check_timestamp_seconds $(date +%s)"
} >"$output.tmp"
chmod 0644 "$output.tmp"
mv -f "$output.tmp" "$output"
