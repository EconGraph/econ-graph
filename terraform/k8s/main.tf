# Local Kubernetes Cluster with Terraform
# This creates a local K8s cluster using kind (Kubernetes in Docker)

terraform {
  required_version = ">= 1.0"
  required_providers {
    local = {
      source  = "hashicorp/local"
      version = "~> 2.4"
    }
    null = {
      source  = "hashicorp/null"
      version = "~> 3.2"
    }
  }
}

# Create kind cluster configuration
resource "local_file" "kind_config" {
  filename = "${path.module}/kind-config.yaml"
  content = yamlencode({
    kind = "Cluster"
    apiVersion = "kind.x-k8s.io/v1alpha4"
    nodes = [
      {
        role = "control-plane"
        kubeadmConfigPatches = [
          "kind: InitConfiguration\nnodeRegistration:\n  kubeletExtraArgs:\n    node-labels: \"ingress-ready=true\""
        ]
        extraPortMappings = [
          {
            containerPort = 80
            hostPort      = 80
            protocol      = "TCP"
          },
          {
            containerPort = 443
            hostPort      = 443
            protocol      = "TCP"
          },
          {
            containerPort = 3000
            hostPort      = 3000
            protocol      = "TCP"
          },
          {
            containerPort = 8080
            hostPort      = 8080
            protocol      = "TCP"
          }
        ]
      },
      {
        # Production Postgres data lives on host btrfs subvolumes, bind-mounted
        # into this node at the same paths (docs/deployment/HOST_STORAGE.md).
        # The static PVs in k8s/manifests/host-storage.yaml point at these
        # paths and are pinned to this node by the label below, so deleting
        # the cluster no longer deletes the database.
        role = "worker"
        labels = {
          "econgraph.io/host-storage" = "true"
        }
        extraMounts = [
          {
            hostPath      = "/srv/econgraph/data-plane"
            containerPath = "/srv/econgraph/data-plane"
          },
          {
            hostPath      = "/srv/econgraph/app-prod"
            containerPath = "/srv/econgraph/app-prod"
          }
        ]
      }
    ]
  })
}

# Create kind cluster
resource "null_resource" "kind_cluster" {
  depends_on = [local_file.kind_config]

  provisioner "local-exec" {
    # Without this, local-exec runs the command through /bin/sh, which is dash (not bash)
    # on Debian/Ubuntu; dash doesn't understand this script's `[[ ]]` or `&>`, and silently
    # misparses the `command -v` checks, always falling into "Unsupported OS: $OSTYPE".
    interpreter = ["/bin/bash", "-c"]
    # The cluster runs on the rootful Docker daemon, whatever DOCKER_HOST the
    # caller's shell has (kind can't start under rootless Docker on a btrfs
    # data root); same default as scripts/deploy/lib/kind-env.sh.
    environment = {
      DOCKER_HOST = "unix:///var/run/docker.sock"
    }
    command = <<-EOT
      # The kind the deploy scripts use: scripts/deploy/install-kind.sh puts
      # the pinned version in the user's data directory (no sudo; does nothing
      # when it is already there), and that copy is run, whatever is on PATH.
      "${path.module}/../../scripts/deploy/install-kind.sh" || exit 1
      source "${path.module}/../../scripts/deploy/lib/kind-version.sh" || exit 1
      kind() { "$KIND_PINNED_BIN" "$@"; }

      # Check if kubectl is installed
      if ! command -v kubectl &> /dev/null; then
        echo "Installing kubectl..."
        if [[ "$OSTYPE" == "darwin"* ]]; then
          brew install kubectl
        elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
          curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
          chmod +x kubectl
          sudo mv kubectl /usr/local/bin/kubectl
        fi
      fi

      # Create cluster if it doesn't exist
      if ! kind get clusters | grep -q "econ-graph"; then
        echo "Creating kind cluster 'econ-graph'..."
        kind create cluster --name econ-graph --config ${local_file.kind_config.filename}
      else
        echo "Cluster 'econ-graph' already exists"
      fi

      # Set kubectl context
      kubectl cluster-info --context kind-econ-graph
    EOT
  }

  # No destroy provisioner: `terraform destroy` leaves the cluster running.
  # Deleting it loses everything stored in its nodes, so it goes only through
  # scripts/deploy/recreate-kind-cluster.sh or
  # scripts/deploy/teardown.sh --include-data-plane --delete-cluster, which back
  # the databases up first and ask for confirmation.
}

# Install NGINX Ingress Controller
resource "null_resource" "nginx_ingress" {
  depends_on = [null_resource.kind_cluster]

  provisioner "local-exec" {
    command = <<-EOT
      echo "Installing NGINX Ingress Controller..."
      kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/main/deploy/static/provider/kind/deploy.yaml

      echo "Waiting for ingress controller to be ready..."
      kubectl wait --namespace ingress-nginx \
        --for=condition=ready pod \
        --selector=app.kubernetes.io/component=controller \
        --timeout=90s
    EOT
  }
}

# Create namespace
resource "null_resource" "create_namespace" {
  depends_on = [null_resource.kind_cluster]

  provisioner "local-exec" {
    command = <<-EOT
      echo "Creating econ-graph namespace..."
      kubectl create namespace econ-graph --dry-run=client -o yaml | kubectl apply -f -
    EOT
  }
}

# Output cluster info
output "cluster_info" {
  value = {
    cluster_name = "econ-graph"
    context      = "kind-econ-graph"
    kubeconfig   = "~/.kube/config"
    ingress_ip   = "localhost"
    frontend_url = "http://localhost:3000"
    backend_url  = "http://localhost:8080"
  }
}
