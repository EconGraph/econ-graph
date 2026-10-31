# VM/API resources are mocked. The real pinned Talos provider generates secrets
# and parses all machine patches; no libvirt daemon or node is contacted.
mock_provider "libvirt" {}

override_resource {
  target = talos_machine_configuration_apply.node
}
override_resource {
  target = talos_machine_bootstrap.cluster
}
override_resource {
  target = talos_cluster_kubeconfig.cluster
  values = { kubeconfig_raw = "test-only-no-client-credential" }
}

run "generate_current_talos_configuration" {
  command = apply
  variables {
    state_passphrase = "test-only-encryption-passphrase-no-runtime-secrets"
    cluster = {
      name         = "econgraph-fit-schema"
      owner_id     = "f075f737-7f7e-4a41-aa14-16830a1db6f7"
      bridge       = "br-eg-fit"
      disk_path    = "/does-not-exist/econgraph-fit-schema"
      api_vip      = "192.0.2.20"
      pod_cidr     = "10.244.0.0/16"
      service_cidr = "10.96.0.0/12"
      data_gib     = 200
      nodes = {
        cp-1     = { role = "controlplane", ip = "192.0.2.21", mac = "52:54:00:ec:01:21" }
        cp-2     = { role = "controlplane", ip = "192.0.2.22", mac = "52:54:00:ec:01:22" }
        cp-3     = { role = "controlplane", ip = "192.0.2.23", mac = "52:54:00:ec:01:23" }
        worker-1 = { role = "worker", ip = "192.0.2.31", mac = "52:54:00:ec:01:31" }
        worker-2 = { role = "worker", ip = "192.0.2.32", mac = "52:54:00:ec:01:32" }
      }
    }
  }
  assert {
    condition     = strcontains(data.talos_machine_configuration.node["worker-1"].machine_configuration, "kind: UserVolumeConfig")
    error_message = "Worker data-volume documents must survive Talos configuration generation."
  }
  assert {
    condition     = strcontains(data.talos_machine_configuration.node["cp-1"].machine_configuration, "name: none")
    error_message = "The cluster must leave CNI bootstrap to the platform lane."
  }
  assert {
    condition     = output.inventory.kubernetes == "1.36.4"
    error_message = "Keep the Kubernetes pin in the supported platform operator matrix."
  }
}
