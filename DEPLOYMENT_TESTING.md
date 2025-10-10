# Deployment Testing Guide

**Purpose**: Validate that the 2389 Agent system is ready for production deployment

**Prerequisites**:
- Docker and Docker Compose installed
- MQTT broker running (localhost:1883 for local tests)
- Environment variables set (`OPENAI_API_KEY`, `SERPER_API_KEY`)

---

## Test Suite Overview

This guide provides 5 deployment test levels, from basic Docker builds to full load testing:

1. **Level 1**: Docker Image Build & Validation
2. **Level 2**: Health Check Verification
3. **Level 3**: MQTT Integration Testing
4. **Level 4**: Multi-Agent Workflow Testing
5. **Level 5**: Load & Performance Testing

---

## Level 1: Docker Image Build & Validation

### Test 1.1: Build Docker Image

```bash
# Build production image
docker build -t agent2389:latest .

# Build with specific Rust version
docker build --build-arg RUST_VERSION=1.80 -t agent2389:v1.80 .
```

**Expected Output**:
- Clean build with no errors
- Final image created successfully

**Verification**:
```bash
# Check image size (should be < 100MB)
docker images agent2389:latest

# Verify image layers
docker history agent2389:latest

# Inspect image metadata
docker inspect agent2389:latest | jq '.[0].Config'
```

**Success Criteria**:
- ✅ Image builds without errors
- ✅ Image size is reasonable (< 100MB for slim build)
- ✅ Non-root user configured (UID 1001)
- ✅ Health check configured

---

### Test 1.2: Multi-Stage Build Verification

```bash
# Ensure intermediate layers are cleaned up
docker images | grep agent2389

# Should only see final images, not build stages
```

**Success Criteria**:
- ✅ No dangling build layers
- ✅ Only production images remain

---

## Level 2: Health Check Verification

### Test 2.1: Container Health Check

```bash
# Start container with minimal config
docker run -d --name test-agent-health \
  -e OPENAI_API_KEY=${OPENAI_API_KEY} \
  -p 8080:8080 \
  agent2389:latest \
  agent2389 --config /app/config/minimal-agent.toml run

# Wait for startup (5 seconds)
sleep 5

# Check container health status
docker ps | grep test-agent-health

# Should show "healthy" status
```

**Expected Output**:
```
CONTAINER ID   IMAGE              STATUS                    PORTS
abc123def456   agent2389:latest   Up 10 seconds (healthy)   0.0.0.0:8080->8080/tcp
```

**Success Criteria**:
- ✅ Container starts successfully
- ✅ Health check passes within 30 seconds
- ✅ Container status shows "healthy"

---

### Test 2.2: Health Endpoint Direct Test

```bash
# Test health endpoint
curl -f http://localhost:8080/health

# Should return JSON with healthy status
```

**Expected Output**:
```json
{
  "status": "healthy",
  "agent_id": "minimal-agent",
  "timestamp": "2025-10-10T12:34:56Z",
  "uptime_seconds": 10
}
```

**Success Criteria**:
- ✅ HTTP 200 response
- ✅ Valid JSON structure
- ✅ `status: "healthy"`

**Cleanup**:
```bash
docker rm -f test-agent-health
```

---

## Level 3: MQTT Integration Testing

These tests validate agent communication via MQTT broker.

### Test 3.1: Docker Compose Stack

```bash
# Start full stack (MQTT broker + agent)
docker-compose up -d

# Wait for services to start
sleep 10

# Verify all services running
docker-compose ps

# Check MQTT broker
docker-compose exec mqtt-broker mosquitto_sub -t '#' -C 1 -W 5

# Check agent logs
docker-compose logs --tail=50 agent
```

**Expected Output**:
- MQTT broker accepting connections on port 1883
- Agent successfully connected to broker
- No error messages in logs

**Success Criteria**:
- ✅ Both services start without errors
- ✅ Agent connects to MQTT broker
- ✅ Health check endpoint responds
- ✅ Agent publishes availability status

---

### Test 3.2: MQTT Message Flow

```bash
# Subscribe to agent availability topic
docker-compose exec mqtt-broker mosquitto_sub \
  -t '/control/agents/+/status' \
  -v &
SUB_PID=$!

# Wait for status messages
sleep 5

# Should see agent status publications
kill $SUB_PID
```

**Expected Output**:
```
/control/agents/researcher-agent/status {
  "agent_id": "researcher-agent",
  "status": "Available",
  "capabilities": ["research", "web_search"],
  "timestamp": "2025-10-10T12:34:56Z"
}
```

**Success Criteria**:
- ✅ Agent publishes status on correct topic
- ✅ Status message is valid JSON
- ✅ Agent shows as "Available"

**Cleanup**:
```bash
docker-compose down
```

---

### Test 3.3: Real MQTT Broker Integration Tests

These are the integration tests already in the test suite:

```bash
# Ensure MQTT broker is running at localhost:1883
./scripts/dev-environment.sh start

# Run integration tests
cargo test --test test_mqtt_broker_integration

# Expected: All tests pass
# - test_connect_to_real_broker
# - test_publish_status_to_real_broker
# - test_subscribe_to_tasks
# - test_disconnect_from_real_broker
```

**Success Criteria**:
- ✅ All 4 integration tests pass
- ✅ No connection errors
- ✅ Clean disconnect

---

### Test 3.4: MQTT Reconnection Tests

```bash
# Run reconnection integration tests
cargo test --test test_mqtt_reconnection_integration

# Expected: All tests pass
# - test_reconnection_after_broker_restart
# - test_unlimited_reconnection_attempts
# - test_reconnection_backoff_pattern
```

**Success Criteria**:
- ✅ Agent reconnects after broker restart
- ✅ Backoff pattern works correctly (25ms → 50ms → 100ms → 250ms)
- ✅ Unlimited retries configured

---

## Level 4: Multi-Agent Workflow Testing

### Test 4.1: V2 Routing E2E Tests

Run the complete V2 routing test suite:

```bash
# Run all V2 routing end-to-end tests
cargo test --test test_v2_routing_e2e

# Tests included:
# - test_research_write_edit_workflow
# - test_iterative_quality_refinement
# - test_max_iterations_prevents_infinite_loop
# - test_workflow_history_tracks_iterations
```

**Success Criteria**:
- ✅ All 4 E2E tests pass
- ✅ Workflows complete without errors
- ✅ Max iterations enforcement works
- ✅ Workflow history tracking accurate

---

### Test 4.2: Realistic Workflow Tests

```bash
# Run realistic workflow tests with mock LLM
cargo test --test test_realistic_v2_workflows

# Tests included:
# - test_realistic_research_write_edit_workflow
# - test_realistic_iterative_refinement_workflow
# - test_realistic_max_iterations_enforcement
```

**Success Criteria**:
- ✅ All 3 workflow tests pass
- ✅ Agent processors instantiate correctly
- ✅ Mock LLM providers work as expected

---

### Test 4.3: Live Multi-Agent Demo

Launch the full tmux demo environment:

```bash
# Set required environment variables
export OPENAI_API_KEY="sk-..."
export SERPER_API_KEY="..."

# Launch V2 workflow demo
./scripts/v2-workflow-test.sh
```

**In the tmux session**:

1. **Window 1 (agents)**: Verify all 3 agents start
   - Top-left: RESEARCHER (port 8080)
   - Top-right: WRITER (port 8081)
   - Bottom-right: EDITOR (port 8082)

2. **Window 2 (mqtt-monitors)**: Verify monitors show activity
   - Top-left: AVAILABILITY (status messages)
   - Top-right: INPUTS (task inputs)
   - Bottom-left: CONVERSATIONS (agent outputs)
   - Bottom-right: PROGRESS (workflow progress)

3. **Window 3 (injector)**: Inject test message

```bash
# In the injector window, run:
cargo run --bin inject-message-v2 -- \
  --query "Research the latest developments in Rust async programming" \
  --agent researcher-agent
```

**Expected Flow**:
1. Researcher receives task on `/control/agents/researcher-agent/input`
2. Researcher processes task, calls LLM + web_search tool
3. Researcher's router decides to forward to writer
4. Writer receives task, creates article
5. Writer's router decides to forward to editor
6. Editor polishes content
7. Editor's router completes workflow
8. Final result published to conversation topic

**Success Criteria**:
- ✅ All 3 agents start without errors
- ✅ MQTT monitors show message flow
- ✅ Workflow completes end-to-end
- ✅ Final output is coherent and complete
- ✅ No crashes or timeouts

**Cleanup**:
```bash
# Exit tmux session with Ctrl+D in each pane or:
tmux kill-session -t v2-test
```

---

## Level 5: Load & Performance Testing

### Test 5.1: Message Throughput Test

```bash
# Start local environment
./scripts/dev-environment.sh start

# Start one agent
OPENAI_API_KEY=test LOG_LEVEL=INFO cargo run -- \
  --config config/dev-agents/researcher-agent.toml run &
AGENT_PID=$!

# Wait for startup
sleep 5

# Inject 100 messages rapidly
for i in {1..100}; do
  cargo run --bin inject-message-v2 -- \
    --query "Test message $i" \
    --agent researcher-agent &
done
wait

# Monitor processing
./scripts/monitor-pipeline.sh status

# Check for errors in logs
tail -100 logs/researcher.log | grep ERROR

# Cleanup
kill $AGENT_PID
```

**Success Criteria**:
- ✅ Agent handles 100 messages without crashing
- ✅ No memory leaks (monitor with `docker stats`)
- ✅ Response times remain stable
- ✅ No dropped messages

---

### Test 5.2: Long-Running Stability Test

```bash
# Start agent with monitoring
docker-compose up -d

# Inject messages every 30 seconds for 1 hour
for i in {1..120}; do
  cargo run --bin inject-message-v2 -- \
    --query "Stability test message $i at $(date)" \
    --agent researcher-agent

  sleep 30
done

# Check container health throughout
watch -n 60 'docker-compose ps && docker stats --no-stream'
```

**Success Criteria**:
- ✅ Agent runs for 1+ hour without crashes
- ✅ Memory usage remains stable (no leaks)
- ✅ All messages processed successfully
- ✅ Health checks pass continuously

---

### Test 5.3: Concurrent Agent Load

```bash
# Start 10 agents concurrently
for i in {1..10}; do
  docker run -d --name agent-$i \
    -e OPENAI_API_KEY=${OPENAI_API_KEY} \
    -e AGENT_ID=agent-$i \
    -p $((8080+i)):8080 \
    agent2389:latest
done

# Send 10 messages to each agent (100 total)
for agent in {1..10}; do
  for msg in {1..10}; do
    cargo run --bin inject-message-v2 -- \
      --query "Load test A$agent M$msg" \
      --agent agent-$agent &
  done
done
wait

# Monitor system resources
docker stats --no-stream

# Cleanup
for i in {1..10}; do
  docker rm -f agent-$i
done
```

**Success Criteria**:
- ✅ All 10 agents start successfully
- ✅ 100 messages processed without errors
- ✅ System resources remain reasonable
- ✅ No agent crashes or hangs

---

## Level 6: Kubernetes Deployment (Optional)

If deploying to Kubernetes:

### Test 6.1: Deploy to K8s Cluster

```bash
# Create namespace
kubectl create namespace agent2389-staging

# Deploy MQTT broker
kubectl apply -f k8s/mqtt-broker.yaml -n agent2389-staging

# Wait for broker to be ready
kubectl wait --for=condition=ready pod -l app=mqtt-broker \
  -n agent2389-staging --timeout=60s

# Deploy agent
kubectl apply -f k8s/agent-deployment.yaml -n agent2389-staging

# Wait for agent to be ready
kubectl wait --for=condition=ready pod -l app=agent \
  -n agent2389-staging --timeout=90s
```

**Success Criteria**:
- ✅ All pods reach "Running" state
- ✅ Health checks pass
- ✅ No CrashLoopBackOff

---

### Test 6.2: K8s Service Connectivity

```bash
# Port-forward to test connectivity
kubectl port-forward -n agent2389-staging svc/agent 8080:8080 &
PF_PID=$!

# Test health endpoint
curl http://localhost:8080/health

# Cleanup
kill $PF_PID
```

**Success Criteria**:
- ✅ Service responds on health endpoint
- ✅ Agent is reachable from within cluster

---

### Test 6.3: K8s Scaling Test

```bash
# Scale to 3 replicas
kubectl scale deployment agent -n agent2389-staging --replicas=3

# Wait for all replicas
kubectl wait --for=condition=ready pod -l app=agent \
  -n agent2389-staging --timeout=90s

# Verify all replicas healthy
kubectl get pods -n agent2389-staging -l app=agent

# Test load distribution
for i in {1..30}; do
  cargo run --bin inject-message-v2 -- \
    --query "K8s test message $i" \
    --agent researcher-agent &
done
wait

# Check logs from all pods
kubectl logs -n agent2389-staging -l app=agent --tail=20
```

**Success Criteria**:
- ✅ All 3 replicas start successfully
- ✅ Messages distributed across replicas
- ✅ No duplicate processing

---

## Cleanup

### Docker Cleanup
```bash
docker-compose down -v
docker rmi agent2389:latest
docker system prune -f
```

### Kubernetes Cleanup
```bash
kubectl delete namespace agent2389-staging
```

### Local Cleanup
```bash
./scripts/dev-environment.sh stop
pkill mosquitto
rm -rf logs/
```

---

## Test Results Checklist

Use this checklist to track deployment testing progress:

### Level 1: Docker Build
- [ ] Docker image builds successfully
- [ ] Image size is reasonable (< 100MB)
- [ ] Multi-stage build cleanup works
- [ ] Image metadata correct (non-root user, health check)

### Level 2: Health Checks
- [ ] Container health check passes
- [ ] Health endpoint returns 200 OK
- [ ] Health JSON structure valid

### Level 3: MQTT Integration
- [ ] Docker Compose stack starts
- [ ] Agent connects to MQTT broker
- [ ] Status messages published correctly
- [ ] Integration tests pass (4/4)
- [ ] Reconnection tests pass (3/3)

### Level 4: Multi-Agent Workflows
- [ ] V2 routing E2E tests pass (4/4)
- [ ] Realistic workflow tests pass (3/3)
- [ ] Live demo completes end-to-end
- [ ] No errors in agent logs

### Level 5: Load & Performance
- [ ] Handles 100 messages without errors
- [ ] Runs for 1+ hour without crashes
- [ ] Memory usage stable (no leaks)
- [ ] 10 concurrent agents work

### Level 6: Kubernetes (Optional)
- [ ] Deploys to K8s cluster
- [ ] Service connectivity works
- [ ] Scales to 3 replicas
- [ ] Load distribution across replicas

---

## Troubleshooting

### Issue: Docker build fails
```bash
# Clear Docker cache
docker builder prune -af

# Rebuild with no cache
docker build --no-cache -t agent2389:latest .
```

### Issue: Agent can't connect to MQTT
```bash
# Check MQTT broker is running
docker ps | grep mosquitto

# Test MQTT broker manually
mosquitto_sub -h localhost -p 1883 -t '#' -v

# Check agent MQTT config
cat config/dev-agents/researcher-agent.toml | grep broker_url
```

### Issue: Health check fails
```bash
# Check agent logs
docker logs <container-id>

# Test health endpoint manually inside container
docker exec <container-id> curl localhost:8080/health

# Verify port mapping
docker port <container-id>
```

### Issue: Tests hang or timeout
```bash
# Check MQTT broker is accessible
nc -zv localhost 1883

# Increase timeout in docker-compose.yml
healthcheck:
  timeout: 10s
  retries: 5

# Run tests with verbose output
RUST_LOG=debug cargo test --test test_mqtt_broker_integration -- --nocapture
```

---

## Continuous Integration

To automate these tests in CI/CD:

### GitHub Actions Example

```yaml
name: Deployment Tests

on: [push, pull_request]

jobs:
  deployment-tests:
    runs-on: ubuntu-latest
    services:
      mosquitto:
        image: eclipse-mosquitto:2
        ports:
          - 1883:1883
        options: >-
          --health-cmd "mosquitto_sub -t '$SYS/#' -C 1"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5

    steps:
      - uses: actions/checkout@v4

      - name: Build Docker Image
        run: docker build -t agent2389:test .

      - name: Run Integration Tests
        run: |
          cargo test --test test_mqtt_broker_integration
          cargo test --test test_mqtt_reconnection_integration

      - name: Run E2E Tests
        run: cargo test --test test_v2_routing_e2e

      - name: Health Check Test
        run: |
          docker run -d --name test-agent \
            -e OPENAI_API_KEY=${{ secrets.OPENAI_API_KEY }} \
            -p 8080:8080 \
            agent2389:test
          sleep 10
          curl -f http://localhost:8080/health
```

---

## Success Criteria Summary

**Deployment is ready when**:
- ✅ All Docker builds succeed
- ✅ All health checks pass
- ✅ All MQTT integration tests pass (7/7)
- ✅ All V2 workflow tests pass (7/7)
- ✅ Load test handles 100+ messages
- ✅ Stability test runs 1+ hour
- ✅ Documentation complete and accurate

**Next Step**: Production deployment 🚀
