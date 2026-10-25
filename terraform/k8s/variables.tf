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
  description = "Kubernetes node image"
  type        = string
  default     = "kindest/node:v1.34.0"
}

variable "enable_metrics" {
  description = "Enable metrics server"
  type        = bool
  default     = true
}
