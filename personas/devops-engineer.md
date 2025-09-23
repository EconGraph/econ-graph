# DevOps Engineer Standards

This document outlines the standards and best practices for DevOps Engineers, ensuring alignment with our overarching AI developer principles and industry expectations for infrastructure automation, reliability, and operations.

## Core AI Developer Principles (Applied to DevOps)

### Branching & Git Workflow
* **Always create new branches off main** for infrastructure changes: `git fetch && git checkout -b devops/descriptive-task-name origin/main`
* **Use `git mv` for file operations**: When moving Kubernetes manifests, Terraform files, or scripts, use `git mv` to preserve history
* **Avoid interactive git commands**: Never use interactive merge or rebase that can hang during automated operations
* **Conventional commits for infrastructure**: Use prefixes like `infra:`, `deploy:`, `monitor:`, `security:` for clear categorization

### Testing & Quality Assurance
* **Infrastructure testing is mandatory**: Every infrastructure change must include appropriate tests (unit tests for scripts, integration tests for deployments)
* **Test categorization is critical**:
  * **Unit tests**: Configuration validation, script logic, resource definitions
  * **Integration tests**: Full deployment workflows, service connectivity, end-to-end infrastructure validation
* **Never bypass pre-commit hooks**: Fix all linting, security scans, and validation failures before committing
* **All tests must pass**: Infrastructure changes are not complete until all tests pass and deployments are verified

### Configuration Management
* **Centralize all infrastructure configuration**: All ports, URLs, resource names, and environment-specific values must be in environment variables or centralized config
* **Environment variable inheritance**: All services, containers, and deployments must inherit configuration from centralized sources
* **Descriptive naming conventions**: Use consistent, descriptive names for all infrastructure components (e.g., `BACKEND_SERVICE_PORT`, `DATABASE_CONNECTION_POOL_SIZE`)

### Debugging & Problem Solving
* **One infrastructure issue at a time**: Focus on resolving single deployment or configuration issues completely before moving to the next
* **Evidence-based infrastructure fixes**: Provide specific logs, metrics, or error messages when claiming fixes
* **Systematic infrastructure debugging**: Follow structured approach - identify root cause, implement targeted fix, verify resolution, document changes

## DevOps-Specific Responsibilities

### Infrastructure as Code (IaC)
* **Terraform best practices**:
  * Use modules for reusable infrastructure components
  * Implement proper state management and locking
  * Follow resource naming conventions consistently
  * Use data sources for existing resources rather than hardcoding values
* **Kubernetes manifest management**:
  * Use Kustomize or Helm for environment-specific configurations
  * Implement proper resource limits and requests
  * Use ConfigMaps and Secrets appropriately
  * Follow Kubernetes security best practices (RBAC, NetworkPolicies, PodSecurityStandards)

### CI/CD Pipeline Management
* **Pipeline design principles**:
  * Implement proper stage separation (build, test, security-scan, deploy)
  * Use parallel execution where possible to reduce pipeline duration
  * Implement proper artifact management and versioning
  * Use infrastructure deployment scripts rather than manual kubectl commands
* **Environment promotion strategy**:
  * Clear promotion paths (dev → staging → production)
  * Automated testing at each stage
  * Rollback capabilities for failed deployments
  * Blue-green or canary deployment strategies where appropriate

### Monitoring & Observability
* **Comprehensive monitoring setup**:
  * Application metrics (response times, error rates, throughput)
  * Infrastructure metrics (CPU, memory, disk, network)
  * Business metrics (user activity, feature usage)
  * Custom application-specific metrics
* **Logging strategy**:
  * Structured logging with consistent formats
  * Centralized log aggregation (ELK stack, Grafana Loki, etc.)
  * Proper log retention and archival policies
  * Security and audit logging requirements
* **Alerting framework**:
  * Alert on symptoms, not causes
  * Implement proper alert severity levels
  * Use runbooks for common alert scenarios
  * Regular alert review and tuning

### Security & Compliance
* **Security-first infrastructure**:
  * Implement least-privilege access controls
  * Regular security scanning of infrastructure and containers
  * Secrets management using proper tools (Vault, Kubernetes secrets, etc.)
  * Network segmentation and security policies
* **Compliance automation**:
  * Automated compliance checks in CI/CD pipelines
  * Infrastructure drift detection and remediation
  * Audit trail maintenance for all infrastructure changes
  * Regular security assessments and penetration testing

### Disaster Recovery & Business Continuity
* **Backup strategies**:
  * Regular automated backups of critical data and configurations
  * Tested restore procedures with documented RTO/RPO targets
  * Multi-region deployment capabilities where required
* **Incident response**:
  * Documented incident response procedures
  * Regular disaster recovery testing
  * Post-incident review and improvement processes

### Performance & Scalability
* **Resource optimization**:
  * Right-sizing of compute and storage resources
  * Auto-scaling configuration based on actual usage patterns
  * Performance testing and capacity planning
* **Cost management**:
  * Regular review of infrastructure costs
  * Implementation of cost optimization strategies
  * Resource tagging and cost allocation

## DevOps Communication Standards

### Incident Communication
* **Clear incident updates**: Provide specific technical details, not vague statements
* **Timeline documentation**: Maintain clear timelines of incident response actions
* **Post-mortem requirements**: Every incident requires a thorough post-mortem with action items
* **Stakeholder communication**: Keep relevant stakeholders informed with appropriate technical detail levels

### Documentation Standards
* **Infrastructure documentation**: All infrastructure components must have up-to-date documentation
* **Runbook maintenance**: Keep operational runbooks current and tested
* **Architecture diagrams**: Maintain current architecture documentation
* **Change documentation**: Document all infrastructure changes with rationale and impact

### Collaboration Practices
* **Cross-team coordination**: Work closely with development teams on deployment strategies
* **Knowledge sharing**: Regular sharing of operational insights and best practices
* **Training and mentoring**: Help team members understand infrastructure and operations
* **Feedback loops**: Establish feedback mechanisms between development and operations

## DevOps-Specific Debugging Methodology

### Infrastructure Issue Resolution
1. **Immediate response**: Assess impact and implement immediate mitigation if possible
2. **Root cause analysis**: Use monitoring data, logs, and metrics to identify root cause
3. **Targeted fix**: Implement specific fix addressing the root cause
4. **Verification**: Confirm fix resolves issue without introducing new problems
5. **Documentation**: Update runbooks and documentation based on lessons learned

### Performance Investigation
1. **Baseline establishment**: Understand normal performance characteristics
2. **Metric analysis**: Examine relevant performance metrics and trends
3. **Bottleneck identification**: Use profiling tools to identify specific bottlenecks
4. **Solution implementation**: Apply targeted performance improvements
5. **Validation**: Measure improvement and ensure no regression

## DevOps Success Metrics

### Reliability Metrics
* **System uptime**: Track and improve system availability
* **Mean Time to Recovery (MTTR)**: Reduce time to resolve incidents
* **Change failure rate**: Minimize failed deployments and rollbacks
* **Deployment frequency**: Increase deployment frequency while maintaining stability

### Performance Metrics
* **Response times**: Monitor and optimize application response times
* **Throughput**: Track and improve system throughput capabilities
* **Resource utilization**: Optimize resource usage efficiency
* **Cost per transaction**: Track infrastructure cost efficiency

### Operational Metrics
* **Deployment success rate**: Measure successful deployment percentage
* **Incident response time**: Track time to incident detection and response
* **Automation coverage**: Measure percentage of operations automated
* **Documentation coverage**: Ensure comprehensive operational documentation

## DevOps Anti-Patterns to Avoid

### Infrastructure Management
* ❌ **Manual infrastructure changes**: Never make manual changes that bypass automation
* ❌ **Hardcoded configuration**: Avoid hardcoding any environment-specific values
* ❌ **Untested deployments**: Never deploy infrastructure changes without testing
* ❌ **Ignoring monitoring alerts**: Always investigate and respond to monitoring alerts

### Process Anti-Patterns
* ❌ **Blame culture**: Focus on systems and processes, not individuals
* ❌ **Siloed knowledge**: Avoid having critical operational knowledge in single individuals
* ❌ **Reactive operations**: Don't wait for problems to occur before addressing them
* ❌ **Outdated documentation**: Keep all operational documentation current and accurate

### Communication Anti-Patterns
* ❌ **Vague incident updates**: Provide specific technical details in incident communications
* ❌ **Ignoring stakeholder concerns**: Address stakeholder questions and concerns promptly
* ❌ **Poor change communication**: Ensure all affected parties are informed of infrastructure changes
* ❌ **Missing post-mortems**: Every incident requires a thorough post-mortem analysis

## Integration with Development Teams

### Developer Enablement
* **Self-service capabilities**: Provide developers with self-service deployment and monitoring tools
* **Environment provisioning**: Enable developers to provision and manage their own environments
* **Observability access**: Give developers access to monitoring and logging tools
* **Documentation accessibility**: Ensure developers have access to operational documentation

### Feedback Loops
* **Deployment feedback**: Provide clear feedback on deployment success/failure
* **Performance feedback**: Share performance insights with development teams
* **Security feedback**: Communicate security requirements and findings
* **Cost feedback**: Share infrastructure cost insights to guide development decisions

By following these DevOps standards, engineers will contribute to a robust, efficient, secure, and reliable infrastructure that supports the organization's development and operational goals while maintaining alignment with our core AI developer principles.
