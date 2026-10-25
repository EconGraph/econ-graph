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
        role = "worker"
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
    command = <<-EOT
      # Check if kind is installed
      if ! command -v kind &> /dev/null; then
        echo "Installing kind..."
        if [[ "$OSTYPE" == "darwin"* ]]; then
          # macOS
          brew install kind
        elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
          # Linux
          curl -Lo ./kind https://kind.sigs.k8s.io/dl/v0.20.0/kind-linux-amd64
          chmod +x ./kind
          sudo mv ./kind /usr/local/bin/kind
        else
          echo "Unsupported OS: $OSTYPE"
          exit 1
        fi
      fi

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

  provisioner "local-exec" {
    when = destroy
    command = "kind delete cluster --name econ-graph"
  }
}

# Install the gateway: Gateway API CRDs, Envoy Gateway and its proxy on host
# ports 80/443 (scripts/deploy/install-gateway.sh, pinned versions).
resource "null_resource" "gateway" {
  depends_on = [null_resource.kind_cluster]

  provisioner "local-exec" {
    interpreter = ["/bin/bash", "-c"]
    command     = "${path.module}/../../scripts/deploy/install-gateway.sh --context kind-econ-graph"
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
