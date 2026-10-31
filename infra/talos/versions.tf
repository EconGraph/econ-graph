terraform {
  required_version = "= 1.13.2"
  required_providers {
    libvirt = {
      source  = "dmacvicar/libvirt"
      version = "= 0.9.9"
    }
    talos = {
      source  = "siderolabs/talos"
      version = "= 0.12.0"
    }
  }
  encryption {
    key_provider "pbkdf2" "local" {
      passphrase = var.state_passphrase
    }
    method "aes_gcm" "local" {
      keys = key_provider.pbkdf2.local
    }
    state {
      method   = method.aes_gcm.local
      enforced = true
    }
    plan {
      method   = method.aes_gcm.local
      enforced = true
    }
  }
}

provider "libvirt" {
  uri = "qemu:///system"
}
provider "talos" {}

variable "state_passphrase" {
  type      = string
  sensitive = true
  validation {
    condition     = length(var.state_passphrase) >= 32
    error_message = "Use the SOPS-protected random state key; no plaintext fallback is permitted."
  }
}

variable "cluster" {
  type = object({
    name         = string
    owner_id     = string
    bridge       = string
    disk_path    = string
    api_vip      = string
    pod_cidr     = string
    service_cidr = string
    data_gib     = number
    nodes = map(object({
      role = string
      ip   = string
      mac  = string
    }))
  })
  validation {
    condition = (
      can(regex("^econgraph-[a-z0-9-]+$", var.cluster.name)) &&
      length([for node in var.cluster.nodes : node if node.role == "controlplane"]) == 3 &&
      contains([2, 3], length([for node in var.cluster.nodes : node if node.role == "worker"])) &&
      alltrue([for node in var.cluster.nodes : contains(["controlplane", "worker"], node.role)]) &&
      var.cluster.data_gib >= 100
    )
    error_message = "Use an explicitly named econgraph cluster with three control planes, two or three workers and at least 100GiB data disks."
  }
}

locals {
  talos_version      = "v1.14.5"
  kubernetes_version = "1.36.4"
  schematic          = "47f718feafaf5934d42735b54da7805fa768627393a4318a0c2d47fd5b1ecb79"
  installer          = "factory.talos.dev/metal-installer/${local.schematic}:${local.talos_version}"
  workers            = { for name, node in var.cluster.nodes : name => node if node.role == "worker" }
  controlplanes      = { for name, node in var.cluster.nodes : name => node if node.role == "controlplane" }
}
