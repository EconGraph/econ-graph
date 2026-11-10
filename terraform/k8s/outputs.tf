# Outputs for local Kubernetes cluster

output "cluster_name" {
  description = "Name of the created cluster"
  value       = var.cluster_name
}

output "kubectl_context" {
  description = "Kubectl context to use"
  value       = "kind-${var.cluster_name}"
}

output "frontend_url" {
  description = "Frontend application URL"
  value       = "http://localhost"
}

output "backend_url" {
  description = "Backend API URL"
  value       = "http://localhost/api"
}

output "ingress_url" {
  description = "Ingress controller URL"
  value       = "http://localhost"
}

output "kubectl_commands" {
  description = "Useful kubectl commands"
  value = {
    get_pods     = "kubectl get pods -n econ-graph"
    get_services = "kubectl get services -n econ-graph"
    get_ingress  = "kubectl get ingress -n econ-graph"
    logs_backend = "kubectl logs -f deployment/econ-graph-backend -n econ-graph"
    logs_frontend = "kubectl logs -f deployment/econ-graph-frontend -n econ-graph"
  }
}
