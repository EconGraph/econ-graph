# Host prep owns the existing LAN bridge and nodatacow subvolume. This module
# neither adopts host storage nor creates a competing NAT/default network.
resource "libvirt_pool" "cluster" {
  name = var.cluster.name
  type = "dir"
  target = {
    path = var.cluster.disk_path
  }
}

resource "libvirt_volume" "installer" {
  name = "talos-${local.talos_version}-${local.schematic}.iso"
  pool = libvirt_pool.cluster.name
  target = {
    format = { type = "raw" }
  }
  create = {
    content = {
      url = "https://factory.talos.dev/image/${local.schematic}/${local.talos_version}/metal-amd64.iso"
    }
  }
}

resource "libvirt_volume" "os" {
  for_each   = var.cluster.nodes
  name       = "${var.cluster.name}-${each.key}-os.qcow2"
  pool       = libvirt_pool.cluster.name
  capacity   = 40 * 1024 * 1024 * 1024
  allocation = 0
  target     = { format = { type = "qcow2" } }
}

resource "libvirt_volume" "data" {
  for_each   = local.workers
  name       = "${var.cluster.name}-${each.key}-postgres.qcow2"
  pool       = libvirt_pool.cluster.name
  capacity   = var.cluster.data_gib * 1024 * 1024 * 1024
  allocation = 0
  target     = { format = { type = "qcow2" } }
}

resource "libvirt_domain" "node" {
  for_each    = var.cluster.nodes
  name        = "${var.cluster.name}-${each.key}"
  description = "econgraph-talos owner=${var.cluster.owner_id} cluster=${var.cluster.name} role=${each.value.role}"
  type        = "kvm"
  memory      = each.value.role == "controlplane" ? 8192 : 65536
  memory_unit = "MiB"
  vcpu        = each.value.role == "controlplane" ? 4 : 16
  running     = true
  autostart   = true
  cpu         = { mode = "host-passthrough" }
  os = {
    type         = "hvm"
    type_arch    = "x86_64"
    type_machine = "q35"
    boot_devices = [{ dev = "hd" }, { dev = "cdrom" }]
  }
  devices = {
    disks = concat([
      {
        device = "disk"
        driver = { name = "qemu", type = "qcow2" }
        source = { file = { file = libvirt_volume.os[each.key].path } }
        target = { dev = "vda", bus = "virtio" }
      }
      ], each.value.role == "worker" ? [
      {
        device = "disk"
        driver = { name = "qemu", type = "qcow2" }
        source = { file = { file = libvirt_volume.data[each.key].path } }
        target = { dev = "vdb", bus = "virtio" }
      }
      ] : [], [
      {
        device    = "cdrom"
        read_only = true
        driver    = { name = "qemu", type = "raw" }
        source    = { file = { file = libvirt_volume.installer.path } }
        target    = { dev = "sda", bus = "sata" }
      }
    ])
    interfaces = [{
      mac    = { address = each.value.mac }
      model  = { type = "virtio" }
      source = { bridge = { bridge = var.cluster.bridge } }
    }]
    channels = [{
      source = { unix = { mode = "bind" } }
      target = { virt_io = { name = "org.qemu.guest_agent.0" } }
    }]
  }
}

# The CLI imports the original secrets from SOPS via a Linux memory file before
# apply. Its build guard refuses to generate a different CA on a rebuild.
resource "talos_machine_secrets" "cluster" {
  talos_version = local.talos_version
}

data "talos_machine_configuration" "node" {
  for_each           = var.cluster.nodes
  cluster_name       = var.cluster.name
  cluster_endpoint   = "https://${var.cluster.api_vip}:6443"
  machine_type       = each.value.role
  machine_secrets    = talos_machine_secrets.cluster.machine_secrets
  talos_version      = local.talos_version
  kubernetes_version = local.kubernetes_version
  config_patches = concat([
    yamlencode({
      machine = {
        install = { disk = "/dev/vda", image = local.installer }
        network = {
          hostname = "${var.cluster.name}-${each.key}"
          interfaces = each.value.role == "controlplane" ? [{
            deviceSelector = { hardwareAddr = each.value.mac }
            dhcp           = true
            vip            = { ip = var.cluster.api_vip }
          }] : []
        }
        features = { kubePrism = { enabled = true, port = 7445 } }
        nodeLabels = each.value.role == "worker" ? {
          "econgraph.io/storage"        = "postgres"
          "topology.kubernetes.io/zone" = "${var.cluster.name}-${each.key}"
        } : {}
        kubelet = {
          extraMounts = each.value.role == "worker" ? [{
            destination = "/var/mnt/postgres"
            type        = "bind"
            source      = "/var/mnt/postgres"
            options     = ["bind", "rshared", "rw"]
          }] : []
        }
      }
      cluster = {
        network = {
          cni            = { name = "none" }
          podSubnets     = [var.cluster.pod_cidr]
          serviceSubnets = [var.cluster.service_cidr]
        }
        proxy = { disabled = true }
      }
    })
    ], each.value.role == "worker" ? [yamlencode({
      apiVersion = "v1alpha1"
      kind       = "UserVolumeConfig"
      name       = "postgres"
      volumeType = "partition"
      provisioning = {
        diskSelector = { match = "disk.dev_path == '/dev/vdb'" }
        minSize      = "${var.cluster.data_gib - 1}GiB"
        grow         = true
      }
      filesystem = { type = "xfs" }
  })] : [])
}

resource "talos_machine_configuration_apply" "node" {
  for_each                    = var.cluster.nodes
  node                        = each.value.ip
  client_configuration        = talos_machine_secrets.cluster.client_configuration
  machine_configuration_input = data.talos_machine_configuration.node[each.key].machine_configuration
  depends_on                  = [libvirt_domain.node]
  timeouts                    = { create = "20m", update = "20m" }
}

resource "talos_machine_bootstrap" "cluster" {
  node                 = local.controlplanes[sort(keys(local.controlplanes))[0]].ip
  client_configuration = talos_machine_secrets.cluster.client_configuration
  depends_on           = [talos_machine_configuration_apply.node]
}

data "talos_client_configuration" "cluster" {
  cluster_name         = var.cluster.name
  client_configuration = talos_machine_secrets.cluster.client_configuration
  endpoints            = [for node in local.controlplanes : node.ip]
  nodes                = [for node in var.cluster.nodes : node.ip]
}

resource "talos_cluster_kubeconfig" "cluster" {
  node                 = local.controlplanes[sort(keys(local.controlplanes))[0]].ip
  client_configuration = talos_machine_secrets.cluster.client_configuration
  depends_on           = [talos_machine_bootstrap.cluster]
}

output "inventory" {
  value = {
    cluster_name = var.cluster.name
    owner_id     = var.cluster.owner_id
    api_endpoint = "https://${var.cluster.api_vip}:6443"
    nodes        = var.cluster.nodes
    pod_cidr     = var.cluster.pod_cidr
    service_cidr = var.cluster.service_cidr
    talos        = local.talos_version
    kubernetes   = local.kubernetes_version
  }
}
output "talosconfig" {
  value     = data.talos_client_configuration.cluster.talos_config
  sensitive = true
}
output "kubeconfig" {
  value     = talos_cluster_kubeconfig.cluster.kubeconfig_raw
  sensitive = true
}
