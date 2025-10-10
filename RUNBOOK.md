# 2389 Agent Operations Runbook

**Purpose**: Day-to-day operational procedures for production 2389 Agent deployments
**Audience**: DevOps Engineers, SREs, On-Call Personnel
**Last Updated**: 2025-10-10

---

## Table of Contents

- [Quick Reference](#quick-reference)
- [Daily Operations](#daily-operations)
- [Common Tasks](#common-tasks)
- [Incident Response](#incident-response)
- [Maintenance Procedures](#maintenance-procedures)
- [Monitoring & Alerts](#monitoring--alerts)
- [Troubleshooting](#troubleshooting)
- [Emergency Procedures](#emergency-procedures)

---

## Quick Reference

### Critical Endpoints

| Service | URL | Purpose |
|---------|-----|---------|
| Agent Health | `http://agent:8080/health` | Overall health status |
| Agent Metrics | `http://agent:8080/metrics` | Operational metrics |
| Agent Readiness | `http://agent:8080/ready` | K8s readiness probe |
| Agent Liveness | `http://agent:8080/live` | K8s liveness probe |
| MQTT Broker | `mqtt://mqtt-broker:1883` | Message transport |

### Quick Health Check

```bash
# Check if agent is healthy
curl -f http://localhost:8080/health && echo "✅ HEALTHY" || echo "❌ UNHEALTHY"

# Check MQTT connection
curl -s http://localhost:8080/health | jq '.checks.mqtt.status'

# Check recent task activity
curl -s http://localhost:8080/health | jq '.checks.task_processing'
```

### Emergency Contacts

| Role | Contact | Escalation |
|------|---------|------------|
| Primary On-Call | [Your team contact] | Page immediately |
| Secondary On-Call | [Backup contact] | After 15 minutes |
| Engineering Lead | [Lead contact] | Critical incidents only |

---

## Daily Operations

### Morning Health Check (5 minutes)

```bash
# 1. Check all agents are running
kubectl get pods -n agent2389 -l app=agent

# Expected: All pods in "Running" state, 1/1 ready

# 2. Check agent health
kubectl get pods -n agent2389 -l app=agent -o wide | \
  while read name ready status restarts age; do
    if [ "$name" != "NAME" ]; then
      echo "Checking $name..."
      kubectl exec -n agent2389 $name -- curl -sf http://localhost:8080/health | jq .status
    fi
  done

# 3. Check MQTT broker
kubectl exec -n agent2389 deploy/mqtt-broker -- mosquitto_sub -t '#' -C 1 -W 5

# 4. Review overnight alerts
# Check your alerting system (PagerDuty, Opsgenie, etc.)

# 5. Check metrics dashboard
# Open Grafana dashboard: [Your Grafana URL]
```

### Daily Metrics Review

**Key Metrics to Check**:

```bash
# Task processing rate (should be stable)
curl -s http://localhost:8080/metrics | jq '.tasks.tasks_completed'

# Error rate (should be < 5%)
curl -s http://localhost:8080/metrics | jq '
  (.tasks.tasks_failed / .tasks.tasks_received) * 100
'

# Average processing time (baseline varies by workload)
curl -s http://localhost:8080/metrics | jq '.tasks.avg_processing_time_ms'

# MQTT connection status (should be true)
curl -s http://localhost:8080/metrics | jq '.mqtt.connected'
```

**What to Look For**:
- ✅ Task completion rate matches expected workload
- ✅ Error rate < 5%
- ✅ Processing time within baseline (±20%)
- ✅ MQTT connected = true
- ✅ No recent restarts

**If anything looks wrong**: See [Incident Response](#incident-response)

---

## Common Tasks

### Deploy New Agent

#### Docker Compose Deployment

```bash
# 1. Pull latest image
docker pull agent2389:latest

# 2. Update docker-compose.yml with new version tag
vim docker-compose.yml  # Update image: agent2389:v1.2.3

# 3. Stop old agent (zero-downtime: start new first)
docker-compose up -d agent

# 4. Verify new agent is healthy
sleep 10
docker-compose exec agent curl -f http://localhost:8080/health

# 5. Check logs for errors
docker-compose logs --tail=50 agent | grep ERROR

# 6. If healthy, old containers will auto-stop
# If unhealthy, rollback:
#   docker-compose down
#   git checkout previous-version
#   docker-compose up -d
```

#### Kubernetes Deployment

```bash
# 1. Update deployment with new image
kubectl set image deployment/agent -n agent2389 \
  agent=agent2389:v1.2.3

# 2. Watch rollout
kubectl rollout status deployment/agent -n agent2389

# 3. Verify new pods are healthy
kubectl get pods -n agent2389 -l app=agent
kubectl exec -n agent2389 deploy/agent -- curl -f http://localhost:8080/health

# 4. Check logs for errors
kubectl logs -n agent2389 -l app=agent --tail=100 | grep ERROR

# 5. If deployment fails, rollback
# kubectl rollout undo deployment/agent -n agent2389
```

---

### Update Agent Configuration

#### Environment Variables (Quick Changes)

```bash
# Docker Compose
vim docker-compose.yml  # Update environment section
docker-compose up -d agent
docker-compose logs --tail=50 agent

# Kubernetes
kubectl edit configmap agent-config -n agent2389
kubectl rollout restart deployment/agent -n agent2389
kubectl rollout status deployment/agent -n agent2389
```

#### Configuration File (Agent Config Changes)

```bash
# 1. Update config file
vim config/production-agent.toml

# 2. Validate syntax
cargo run --bin agent2389 -- --config config/production-agent.toml validate

# 3. Deploy updated config
# Docker: Mount new config volume
# K8s: Update ConfigMap from file
kubectl create configmap agent-config \
  --from-file=config/production-agent.toml \
  --dry-run=client -o yaml | \
  kubectl apply -f -

# 4. Restart agents to pick up new config
kubectl rollout restart deployment/agent -n agent2389

# 5. Verify config loaded correctly
kubectl logs -n agent2389 -l app=agent --tail=20 | grep "Configuration loaded"
```

---

### Scale Agent Horizontally

#### Increase Agent Count

```bash
# Docker Compose (manual)
docker-compose up -d --scale agent=3

# Kubernetes
kubectl scale deployment/agent -n agent2389 --replicas=3

# Verify all replicas are healthy
kubectl get pods -n agent2389 -l app=agent
kubectl wait --for=condition=ready pod -l app=agent -n agent2389 --timeout=90s

# Check load distribution
for pod in $(kubectl get pods -n agent2389 -l app=agent -o name); do
  echo "Checking $pod..."
  kubectl exec -n agent2389 $pod -- curl -s http://localhost:8080/metrics | \
    jq '{tasks_received: .tasks.tasks_received, tasks_completed: .tasks.tasks_completed}'
done
```

#### Decrease Agent Count

```bash
# Kubernetes (graceful scale-down)
kubectl scale deployment/agent -n agent2389 --replicas=1

# Verify remaining agents handle load
kubectl top pods -n agent2389 -l app=agent
```

---

### Restart Agent (Graceful)

```bash
# Docker Compose
docker-compose restart agent
docker-compose logs --tail=50 agent

# Kubernetes (rolling restart)
kubectl rollout restart deployment/agent -n agent2389
kubectl rollout status deployment/agent -n agent2389

# Verify health after restart
kubectl exec -n agent2389 deploy/agent -- curl -f http://localhost:8080/health
```

---

### View Agent Logs

#### Real-time Log Monitoring

```bash
# Docker
docker-compose logs -f agent

# Kubernetes (all pods)
kubectl logs -n agent2389 -l app=agent -f

# Kubernetes (specific pod)
kubectl logs -n agent2389 pod/agent-7d9f8c-xyz -f

# Filter for errors only
kubectl logs -n agent2389 -l app=agent --tail=100 | jq 'select(.level=="ERROR")'
```

#### Search Logs for Specific Issues

```bash
# Find all errors in last hour
kubectl logs -n agent2389 -l app=agent --since=1h | jq 'select(.level=="ERROR")'

# Find logs for specific task
kubectl logs -n agent2389 -l app=agent --tail=1000 | \
  jq 'select(.span.task_id=="550e8400-e29b-41d4-a716-446655440000")'

# Find MQTT connection issues
kubectl logs -n agent2389 -l app=agent --tail=500 | \
  jq 'select(.target | contains("mqtt"))'

# Find slow tasks (>5 seconds)
kubectl logs -n agent2389 -l app=agent --tail=1000 | \
  jq 'select(.fields.duration_ms > 5000)'
```

---

### Check MQTT Broker Health

```bash
# Test MQTT broker connectivity
docker exec mqtt-broker mosquitto_sub -t '$SYS/#' -C 5

# Check broker metrics
docker exec mqtt-broker mosquitto_sub -t '$SYS/broker/clients/connected' -C 1

# Monitor live MQTT traffic
docker exec mqtt-broker mosquitto_sub -t '#' -v

# Check agent MQTT status
curl -s http://localhost:8080/metrics | jq '.mqtt'
```

---

## Incident Response

### Incident Severity Levels

| Severity | Response Time | Description |
|----------|--------------|-------------|
| **P1 - Critical** | Immediate | Service down, data loss, security breach |
| **P2 - High** | 15 minutes | Degraded performance, partial outage |
| **P3 - Medium** | 1 hour | Non-critical feature broken |
| **P4 - Low** | Next business day | Minor issues, cosmetic bugs |

---

### P1: Agent Not Responding

**Symptoms**: Health checks failing, no task processing, no MQTT messages

**Immediate Actions**:

```bash
# 1. Check if agent process is running
kubectl get pods -n agent2389 -l app=agent

# 2. Check pod status and events
kubectl describe pod -n agent2389 -l app=agent

# 3. Check recent logs for errors
kubectl logs -n agent2389 -l app=agent --tail=100 | grep -E "ERROR|FATAL|panic"

# 4. Check health endpoint if pod is running
kubectl exec -n agent2389 deploy/agent -- curl -f http://localhost:8080/health || echo "HEALTH CHECK FAILED"

# 5. Check MQTT broker connectivity
kubectl exec -n agent2389 deploy/mqtt-broker -- mosquitto_sub -t '#' -C 1 -W 5
```

**Diagnosis & Resolution**:

**If pod is CrashLoopBackOff**:
```bash
# Check logs from crashed pod
kubectl logs -n agent2389 pod/agent-xyz --previous

# Common causes:
# - Configuration error: Check ConfigMap
# - Resource limits: Check kubectl describe pod
# - Dependency unavailable: Check MQTT broker
```

**If pod is Running but unhealthy**:
```bash
# Check what's failing
curl -s http://localhost:8080/health | jq '.checks'

# If MQTT check is failing:
kubectl exec -n agent2389 deploy/mqtt-broker -- mosquitto_sub -t '#' -C 1 -W 5

# If task_processing check is stale:
# Check for hung tasks in logs
kubectl logs -n agent2389 -l app=agent --tail=500 | jq 'select(.span.name=="task_processing")'
```

**Escalation**:
- If cannot resolve in 15 minutes → Page secondary on-call
- If data loss risk → Page engineering lead immediately

---

### P2: High Error Rate

**Symptoms**: Error rate > 10%, many failed tasks

**Immediate Actions**:

```bash
# 1. Check current error rate
curl -s http://localhost:8080/metrics | jq '
  {
    received: .tasks.tasks_received,
    failed: .tasks.tasks_failed,
    error_rate: ((.tasks.tasks_failed / .tasks.tasks_received) * 100)
  }
'

# 2. Find recent errors
kubectl logs -n agent2389 -l app=agent --since=10m | jq 'select(.level=="ERROR")' | head -20

# 3. Check for common error patterns
kubectl logs -n agent2389 -l app=agent --since=10m | jq 'select(.level=="ERROR") | .fields.message' | sort | uniq -c

# 4. Check tool failures
curl -s http://localhost:8080/metrics | jq '.tools.tool_stats'
```

**Common Causes & Solutions**:

**Tool timeouts**:
```bash
# Check which tools are timing out
curl -s http://localhost:8080/metrics | jq '.tools.tool_stats | to_entries[] | select(.value.timeouts > 0)'

# Solution: Increase tool timeout in config or investigate slow external services
```

**LLM API failures**:
```bash
# Check logs for API errors
kubectl logs -n agent2389 -l app=agent --since=10m | grep "API"

# Solution: Check API key, rate limits, service status
```

**MQTT publish failures**:
```bash
# Check MQTT metrics
curl -s http://localhost:8080/metrics | jq '.mqtt.publish_failures'

# Solution: Check broker health, network connectivity
```

---

### P2: MQTT Connection Lost

**Symptoms**: MQTT connected = false, no message flow

**Immediate Actions**:

```bash
# 1. Check MQTT broker is running
kubectl get pods -n agent2389 -l app=mqtt-broker

# 2. Test broker connectivity from agent
kubectl exec -n agent2389 deploy/agent -- nc -zv mqtt-broker 1883

# 3. Check broker logs
kubectl logs -n agent2389 -l app=mqtt-broker --tail=100

# 4. Check agent MQTT status
curl -s http://localhost:8080/metrics | jq '.mqtt'
```

**Resolution**:

**If broker is down**:
```bash
# Restart broker
kubectl rollout restart deployment/mqtt-broker -n agent2389
kubectl rollout status deployment/mqtt-broker -n agent2389

# Verify agents reconnect (automatic with backoff)
sleep 30
curl -s http://localhost:8080/metrics | jq '.mqtt.connected'
```

**If broker is running but agents can't connect**:
```bash
# Check network policy
kubectl describe networkpolicy -n agent2389

# Check service endpoints
kubectl get endpoints -n agent2389 mqtt-broker

# Restart agent to force reconnect
kubectl rollout restart deployment/agent -n agent2389
```

---

### P3: Slow Task Processing

**Symptoms**: High p95/p99 latency, task backlog building

**Immediate Actions**:

```bash
# 1. Check processing time percentiles
curl -s http://localhost:8080/metrics | jq '.tasks | {avg: .avg_processing_time_ms, p50: .processing_time_p50_ms, p95: .processing_time_p95_ms, p99: .processing_time_p99_ms}'

# 2. Check current pipeline depth
curl -s http://localhost:8080/metrics | jq '.tasks.current_pipeline_depth'

# 3. Check tool execution times
curl -s http://localhost:8080/metrics | jq '.tools.tool_stats | to_entries[] | {tool: .key, avg_time: .value.avg_execution_time_ms}'

# 4. Check for hung tasks
kubectl logs -n agent2389 -l app=agent --since=10m | jq 'select(.span.name=="task_processing" and .fields.duration_ms > 30000)'
```

**Solutions**:

**High LLM latency**:
```bash
# Check external LLM API status
# Consider scaling horizontally to increase throughput

kubectl scale deployment/agent -n agent2389 --replicas=3
```

**High tool execution time**:
```bash
# Identify slow tools
curl -s http://localhost:8080/metrics | jq '.tools.tool_stats | to_entries[] | select(.value.avg_execution_time_ms > 5000)'

# Review tool configuration, increase timeouts, or optimize tool implementation
```

**Pipeline depth at max**:
```bash
# Scale horizontally to increase capacity
kubectl scale deployment/agent -n agent2389 --replicas=5
```

---

## Maintenance Procedures

### Log Rotation

#### Docker Compose

```bash
# Configure in docker-compose.yml
services:
  agent:
    logging:
      driver: "json-file"
      options:
        max-size: "50m"
        max-file: "3"

# Manually rotate if needed
docker-compose kill -s SIGUSR1 agent
```

#### Kubernetes

```bash
# Kubernetes handles log rotation automatically
# Check disk usage
kubectl exec -n agent2389 deploy/agent -- df -h /var/log

# Force log cleanup if needed
kubectl exec -n agent2389 deploy/agent -- find /var/log -name "*.log" -mtime +7 -delete
```

---

### Metrics Cleanup

```bash
# Metrics are kept in-memory, reset on agent restart
# To force metrics reset (e.g., after testing)

# Docker
docker-compose restart agent

# Kubernetes
kubectl rollout restart deployment/agent -n agent2389
```

---

### Certificate Renewal

**For MQTT TLS (if using TLS)**:

```bash
# 1. Generate new certificates
./scripts/generate-certs.sh

# 2. Update secrets
kubectl create secret tls mqtt-tls \
  --cert=certs/mqtt-broker.crt \
  --key=certs/mqtt-broker.key \
  --dry-run=client -o yaml | \
  kubectl apply -n agent2389 -f -

# 3. Restart broker to load new certs
kubectl rollout restart deployment/mqtt-broker -n agent2389

# 4. Verify agents reconnect
sleep 30
curl -s http://localhost:8080/metrics | jq '.mqtt.connected'
```

---

### Database Backup (If using persistent storage)

```bash
# If using persistent task history or state

# 1. Create backup
kubectl exec -n agent2389 deploy/agent -- pg_dump > backup-$(date +%Y%m%d).sql

# 2. Upload to S3 or backup storage
aws s3 cp backup-$(date +%Y%m%d).sql s3://backups/agent2389/

# 3. Verify backup
aws s3 ls s3://backups/agent2389/ | tail -5
```

---

## Monitoring & Alerts

### Key Metrics to Monitor

| Metric | Threshold | Alert Severity |
|--------|-----------|----------------|
| Error rate | > 10% | P2 |
| Task processing time (p95) | > 10s | P3 |
| MQTT disconnections | Any | P2 |
| Agent restarts | > 3 in 10min | P2 |
| Memory usage | > 80% | P3 |
| CPU usage | > 80% | P3 |

### Prometheus Alert Rules

```yaml
# agent-alerts.yml
groups:
- name: agent2389
  rules:
  - alert: AgentDown
    expr: up{job="agent2389"} == 0
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "Agent {{ $labels.instance }} is down"

  - alert: HighErrorRate
    expr: |
      rate(agent2389_tasks_failed[5m]) / rate(agent2389_tasks_received[5m]) > 0.1
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High error rate: {{ $value | humanizePercentage }}"

  - alert: MQTTDisconnected
    expr: agent2389_mqtt_connected == 0
    for: 30s
    labels:
      severity: critical
    annotations:
      summary: "MQTT connection lost"

  - alert: SlowTaskProcessing
    expr: agent2389_processing_time_p95_ms > 10000
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "Slow task processing: {{ $value }}ms (p95)"
```

### Grafana Dashboard Panels

**Essential Panels**:
1. Task throughput (tasks/second)
2. Error rate (%)
3. Processing time percentiles (p50, p95, p99)
4. MQTT connection status
5. Active tasks (pipeline depth)
6. Memory usage
7. CPU usage

**Dashboard JSON**: See `docs/grafana-dashboard.json`

---

## Troubleshooting

### Diagnostic Commands

```bash
# Complete health check
curl -s http://localhost:8080/health | jq

# All metrics
curl -s http://localhost:8080/metrics | jq

# Agent logs (last 100 lines)
kubectl logs -n agent2389 -l app=agent --tail=100

# Agent logs (errors only)
kubectl logs -n agent2389 -l app=agent --tail=500 | jq 'select(.level=="ERROR")'

# MQTT broker status
kubectl exec -n agent2389 deploy/mqtt-broker -- mosquitto_sub -t '$SYS/#' -C 10

# Resource usage
kubectl top pods -n agent2389 -l app=agent

# Pod events
kubectl get events -n agent2389 --sort-by='.lastTimestamp'
```

### Common Issues

See **[TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md)** for detailed troubleshooting guides.

---

## Emergency Procedures

### Emergency Rollback

```bash
# Kubernetes
kubectl rollout undo deployment/agent -n agent2389
kubectl rollout status deployment/agent -n agent2389

# Docker Compose
git checkout previous-working-commit
docker-compose down
docker-compose up -d
```

### Emergency Shutdown

```bash
# Kubernetes (graceful)
kubectl scale deployment/agent -n agent2389 --replicas=0

# Docker Compose
docker-compose stop agent

# Verify no tasks are running
curl -s http://localhost:8080/metrics | jq '.tasks.tasks_processing'
```

### Emergency Contact Escalation

1. **Immediate**: Primary on-call (PagerDuty/phone)
2. **15 minutes**: Secondary on-call
3. **30 minutes**: Engineering lead
4. **1 hour**: CTO (critical incidents only)

---

## Appendix

### Useful Scripts

```bash
# Quick agent health check
alias agent-health='curl -sf http://localhost:8080/health | jq .status'

# Watch metrics
alias agent-watch='watch -n 5 "curl -s http://localhost:8080/metrics | jq \"{tasks: .tasks, mqtt: .mqtt}\""'

# Tail agent logs
alias agent-logs='kubectl logs -n agent2389 -l app=agent -f'
```

### Related Documentation

- [DEPLOYMENT.md](DEPLOYMENT.md) - Deployment procedures
- [TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) - Detailed troubleshooting
- [OBSERVABILITY.md](docs/OBSERVABILITY.md) - Monitoring system details
- [DEPLOYMENT_TESTING.md](DEPLOYMENT_TESTING.md) - Testing procedures

---

**Document Version**: 1.0
**Last Review**: 2025-10-10
**Next Review**: 2025-11-10
