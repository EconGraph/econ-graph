# Variables for local Kubernetes cluster

variable "cluster_name" {
  description = "Name of the kind cluster"
  type        = string
  default     = "econ-graph"
}

variable "node_count" {
  description = "Number of worker nodes"
  type        = number
  default     = 1
}

variable "node_image" {
  description = "Node image of every kind node, written into kind-config.yaml (Kubernetes 1.36, pinned by digest; needs kind v0.33.0 or newer)"
  type        = string
  default     = "kindest/node:v1.36.4@sha256:099e049362a1526b2db71494e1947aae99bd16290d7c895f2b7ea312e3cbfaed"
}

variable "enable_metrics" {
  description = "Enable metrics server"
  type        = bool
  default     = true
}
