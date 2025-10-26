# PRISM API User Experience Specification
## Developer-First API Design for Multi-Agent Platform

**Version:** 2.0.0  
**Date:** 2025-01-20  
**Prepared by:** Product Manager Agent  
**Status:** Phase 2 - Ready for Development  
**Target**: <15 minutes developer onboarding time

---

## Executive Summary

This specification defines the user experience for PRISM's API ecosystem, focusing on developer productivity and satisfaction. The goal is to enable developers to make their first successful API call within 15 minutes of discovering PRISM, with comprehensive SDK support and intuitive error handling.

### Success Metrics
- **Developer Onboarding**: <15 minutes to first API call success
- **SDK Coverage**: 10+ code examples per supported language (JavaScript, Rust, Python)
- **Error Experience**: User-friendly messages for all error codes with actionable solutions
- **Performance Balance**: Rate limiting strategy that protects system while enabling productivity

---

## Developer Onboarding Experience

### 15-Minute Success Journey

#### Phase 1: Discovery & Setup (0-5 minutes)
**Goal**: From documentation to running environment

```bash
# Step 1: Quick Install (30 seconds)
curl -fsSL https://get.prism.dev/install.sh | sh

# Step 2: Initialize Project (1 minute)
prism init my-first-project --template quickstart

# Step 3: Start Local Environment (2 minutes)
cd my-first-project
prism dev start

# Step 4: Verify Installation (30 seconds)
prism status
# ✓ PRISM Agent: Running
# ✓ API Server: http://localhost:4001
# ✓ Dashboard: http://localhost:3000
```

#### Phase 2: Authentication & First Call (5-10 minutes)
**Goal**: Authenticate and make first successful API request

```javascript
// Step 5: Get API Key (Interactive CLI - 2 minutes)
$ prism auth login
# Opens browser to dashboard
# Copy API key: prism_dev_1234567890abcdef

// Step 6: First API Call (30 seconds)
const prism = require('@prism/sdk');
const client = new prism.Client({
  apiKey: 'prism_dev_1234567890abcdef',
  endpoint: 'http://localhost:4001'
});

// Step 7: Create First Agent (2 minutes)
const agent = await client.agents.create({
  type: 'developer',
  name: 'my-first-agent',
  capabilities: ['code-review', 'testing']
});

console.log(`✓ Agent created: ${agent.id}`);
```

#### Phase 3: Meaningful Interaction (10-15 minutes)
**Goal**: Deploy agent and see it working

```javascript
// Step 8: Deploy Agent to Task (3 minutes)
const task = await client.tasks.create({
  type: 'code-review',
  title: 'Review README.md',
  description: 'Check for typos and clarity',
  assignee: agent.id,
  files: ['README.md']
});

// Step 9: Watch Progress (2 minutes)
const progress = await client.tasks.watch(task.id, (update) => {
  console.log(`Progress: ${update.progress}% - ${update.message}`);
});

// Expected Output:
// Progress: 25% - Analyzing file structure
// Progress: 50% - Checking grammar and syntax  
// Progress: 75% - Generating suggestions
// Progress: 100% - Review complete
```

### Onboarding Success Indicators
- ✅ Environment running in <5 minutes
- ✅ First API call successful in <10 minutes
- ✅ Agent deployed and working in <15 minutes
- ✅ Developer has clear next steps

### Onboarding Failure Recovery
```bash
# Common Issues & Solutions
prism doctor                    # Automatic diagnostics
prism logs --tail 50           # Recent system logs
prism reset --confirm          # Nuclear option - full reset
prism support --collect-logs   # Support bundle generation
```

---

## SDK Documentation & Examples

### JavaScript/TypeScript SDK

#### Installation & Quick Start
```bash
npm install @prism/sdk
# or
yarn add @prism/sdk
```

#### Example 1: Agent Management
```typescript
import { PrismClient, AgentType } from '@prism/sdk';

const client = new PrismClient({
  apiKey: process.env.PRISM_API_KEY,
  endpoint: 'https://api.prism.dev'
});

// Create specialized agent
const ctoAgent = await client.agents.create({
  type: AgentType.CTO,
  name: 'technical-lead',
  resources: {
    cpu: '2 cores',
    memory: '4GB',
    priority: 'high'
  },
  config: {
    expertise: ['architecture', 'performance', 'security'],
    reviewStyle: 'thorough',
    escalationThreshold: 'high'
  }
});

console.log(`CTO Agent deployed: ${ctoAgent.id}`);
```

#### Example 2: Real-time Event Handling
```typescript
// WebSocket connection for real-time updates
const events = client.events.connect();

events.on('agent.status.changed', (event) => {
  console.log(`Agent ${event.agent_id}: ${event.status}`);
});

events.on('task.completed', async (event) => {
  const result = await client.tasks.getResult(event.task_id);
  console.log('Task completed:', result.summary);
});

// Handle connection issues gracefully
events.on('disconnected', () => {
  console.log('Connection lost, attempting reconnect...');
  setTimeout(() => events.reconnect(), 5000);
});
```

#### Example 3: Batch Operations
```typescript
// Efficient bulk operations
const agents = await client.agents.createBatch([
  { type: 'developer', name: 'frontend-specialist' },
  { type: 'developer', name: 'backend-specialist' },
  { type: 'qa', name: 'test-automation' }
]);

// Parallel task assignment
const tasks = await Promise.all([
  client.tasks.create({ type: 'frontend-review', assignee: agents[0].id }),
  client.tasks.create({ type: 'backend-review', assignee: agents[1].id }),
  client.tasks.create({ type: 'test-generation', assignee: agents[2].id })
]);

console.log(`Created ${tasks.length} tasks across ${agents.length} agents`);
```

#### Example 4: Error Handling & Retries
```typescript
import { PrismError, ErrorCode } from '@prism/sdk';

try {
  const agent = await client.agents.create(agentConfig);
} catch (error) {
  if (error instanceof PrismError) {
    switch (error.code) {
      case ErrorCode.INSUFFICIENT_RESOURCES:
        console.log('Not enough resources available');
        // Show user current resource usage
        const usage = await client.system.getResourceUsage();
        console.log(`CPU: ${usage.cpu}%, Memory: ${usage.memory}%`);
        break;
        
      case ErrorCode.RATE_LIMIT_EXCEEDED:
        console.log(`Rate limited. Retry after ${error.retryAfter}ms`);
        setTimeout(() => retryOperation(), error.retryAfter);
        break;
        
      default:
        console.error('Unexpected error:', error.message);
    }
  }
}
```

#### Example 5: Offline-First Usage
```typescript
// SDK automatically handles offline scenarios
const client = new PrismClient({
  apiKey: process.env.PRISM_API_KEY,
  offline: {
    enabled: true,
    storage: 'localStorage', // or 'indexedDB'
    syncOnReconnect: true
  }
});

// Queue operations when offline
const task = await client.tasks.create({
  type: 'code-review',
  title: 'Offline review task'
}); // Queued locally if offline

// Listen for sync events
client.on('sync.started', () => console.log('Syncing queued operations...'));
client.on('sync.completed', (results) => {
  console.log(`Synced ${results.length} operations`);
});
```

### Rust SDK

#### Installation
```toml
[dependencies]
prism-sdk = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
```

#### Example 1: High-Performance Agent Operations
```rust
use prism_sdk::{Client, AgentConfig, AgentType};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new()
        .api_key(std::env::var("PRISM_API_KEY")?)
        .endpoint("https://api.prism.dev")
        .build()?;

    // Create high-performance agent
    let agent = client.agents().create(AgentConfig {
        agent_type: AgentType::Developer,
        name: "rust-specialist".to_string(),
        resources: Some(ResourceConfig {
            cpu_cores: 8,
            memory_gb: 16,
            priority: Priority::High,
        }),
        ..Default::default()
    }).await?;

    println!("Agent created: {}", agent.id);
    Ok(())
}
```

#### Example 2: Stream Processing
```rust
use futures_util::StreamExt;
use prism_sdk::events::EventStream;

// Real-time event processing
let mut events = client.events().stream().await?;

while let Some(event) = events.next().await {
    match event? {
        Event::AgentStatusChanged { agent_id, status } => {
            println!("Agent {} status: {:?}", agent_id, status);
        }
        Event::TaskCompleted { task_id, result } => {
            println!("Task {} completed with result: {}", task_id, result);
        }
        _ => {} // Handle other events
    }
}
```

#### Example 3: Concurrent Task Management
```rust
use tokio::task::JoinSet;

// Spawn multiple concurrent tasks
let mut set = JoinSet::new();

for i in 0..10 {
    let client = client.clone();
    set.spawn(async move {
        let task = client.tasks().create(TaskConfig {
            task_type: "parallel-processing".to_string(),
            data: format!("batch-{}", i).into_bytes(),
            ..Default::default()
        }).await?;
        
        // Wait for completion
        client.tasks().wait_for_completion(task.id).await
    });
}

// Collect all results
while let Some(result) = set.join_next().await {
    match result {
        Ok(Ok(task_result)) => println!("Task completed: {}", task_result.id),
        Ok(Err(e)) => eprintln!("Task failed: {}", e),
        Err(e) => eprintln!("Join error: {}", e),
    }
}
```

### Python SDK

#### Installation
```bash
pip install prism-sdk
```

#### Example 1: Data Science Integration
```python
import prism
import pandas as pd
import asyncio

async def main():
    client = prism.Client(
        api_key=os.environ['PRISM_API_KEY'],
        endpoint='https://api.prism.dev'
    )
    
    # Create data analysis agent
    agent = await client.agents.create(
        type='analyst',
        name='data-scientist',
        capabilities=['pandas', 'numpy', 'matplotlib'],
        resources={'memory': '8GB', 'cpu': '4 cores'}
    )
    
    # Upload dataset for analysis
    dataset = pd.read_csv('sales_data.csv')
    task = await client.tasks.create(
        type='data-analysis',
        assignee=agent.id,
        data=dataset.to_json(),
        instructions='Analyze sales trends and generate insights'
    )
    
    # Stream results as they're generated
    async for update in client.tasks.stream_progress(task.id):
        if update.type == 'insight':
            print(f"Insight: {update.content}")
        elif update.type == 'visualization':
            # Save generated chart
            with open(f"chart_{update.timestamp}.png", 'wb') as f:
                f.write(update.data)

asyncio.run(main())
```

#### Example 2: Jupyter Notebook Integration
```python
# Cell 1: Setup
import prism
from prism.jupyter import PrismMagics

client = prism.Client()
client.load_ipython_extension()

# Cell 2: Magic commands for interactive development
%%prism agent create
type: developer
name: notebook-assistant
capabilities: [python, data-analysis, visualization]

# Cell 3: Interactive task execution
%%prism task run --stream
type: code-review
files: [analysis.py, visualization.py]
instructions: Check for performance issues and suggest optimizations
```

#### Example 4: Machine Learning Workflow
```python
import prism
from sklearn.model_selection import train_test_split
import joblib

async def ml_workflow():
    client = prism.Client()
    
    # Create specialized ML agent
    ml_agent = await client.agents.create(
        type='ml-engineer',
        name='model-trainer',
        resources={'gpu': '1x V100', 'memory': '32GB'},
        packages=['tensorflow', 'pytorch', 'scikit-learn']
    )
    
    # Training task with automatic checkpointing
    training_task = await client.tasks.create(
        type='model-training',
        assignee=ml_agent.id,
        config={
            'dataset': 'gs://my-bucket/training-data.parquet',
            'model_type': 'transformer',
            'epochs': 100,
            'checkpoint_interval': 10
        }
    )
    
    # Monitor training progress
    async for checkpoint in client.tasks.stream_checkpoints(training_task.id):
        print(f"Epoch {checkpoint.epoch}: Loss = {checkpoint.loss:.4f}")
        
        # Early stopping condition
        if checkpoint.loss < 0.001:
            await client.tasks.stop(training_task.id)
            break
    
    # Download trained model
    model_artifacts = await client.tasks.get_artifacts(training_task.id)
    model = joblib.load(model_artifacts['model.pkl'])
    
    return model
```

---

## Error Handling User Experience

### Error Code System

#### HTTP Status Code Mapping
```json
{
  "200": "Success",
  "201": "Created", 
  "400": "Bad Request - Check request format",
  "401": "Unauthorized - Invalid API key",
  "403": "Forbidden - Insufficient permissions", 
  "404": "Not Found - Resource doesn't exist",
  "409": "Conflict - Resource already exists",
  "422": "Validation Error - Invalid data",
  "429": "Rate Limited - Too many requests",
  "500": "Server Error - Try again later",
  "503": "Service Unavailable - System maintenance"
}
```

#### Custom PRISM Error Codes
```typescript
enum PrismErrorCode {
  // Resource Constraints (4000-4099)
  INSUFFICIENT_RESOURCES = 4001,
  STORAGE_QUOTA_EXCEEDED = 4002,
  CPU_LIMIT_EXCEEDED = 4003,
  MEMORY_LIMIT_EXCEEDED = 4004,
  
  // Agent Management (4100-4199)
  AGENT_NOT_FOUND = 4101,
  AGENT_BUSY = 4102,
  AGENT_OFFLINE = 4103,
  AGENT_CREATION_FAILED = 4104,
  INVALID_AGENT_TYPE = 4105,
  
  // Network & P2P (4200-4299)
  NETWORK_PARTITION = 4201,
  PEER_UNREACHABLE = 4202,
  SYNC_CONFLICT = 4203,
  CONNECTION_TIMEOUT = 4204,
  
  // Task Management (4300-4399)
  TASK_NOT_FOUND = 4301,
  TASK_CANCELLED = 4302,
  TASK_TIMEOUT = 4303,
  INVALID_TASK_TYPE = 4304,
  DEPENDENCY_FAILED = 4305,
  
  // Authentication & Authorization (4400-4499)
  API_KEY_INVALID = 4401,
  API_KEY_EXPIRED = 4402,
  PERMISSION_DENIED = 4403,
  RATE_LIMIT_EXCEEDED = 4429
}
```

### User-Friendly Error Messages

#### Error Response Format
```json
{
  "error": {
    "code": 4001,
    "type": "INSUFFICIENT_RESOURCES",
    "message": "Not enough resources available to create agent",
    "details": {
      "requested": {
        "cpu": "4 cores",
        "memory": "8GB"
      },
      "available": {
        "cpu": "2 cores", 
        "memory": "4GB"
      }
    },
    "suggestions": [
      "Reduce resource requirements for the agent",
      "Wait for other agents to complete their tasks", 
      "Upgrade to a higher tier plan for more resources"
    ],
    "documentation": "https://docs.prism.dev/errors/insufficient-resources",
    "support": "https://support.prism.dev/contact"
  }
}
```

#### Context-Aware Error Messages

**Agent Creation Errors:**
```json
{
  "error": {
    "code": 4104,
    "type": "AGENT_CREATION_FAILED", 
    "message": "Failed to create CTO agent: Docker image not available",
    "context": {
      "agent_type": "CTO",
      "requested_image": "prism/cto-agent:latest",
      "available_images": ["prism/cto-agent:v1.0.0", "prism/cto-agent:v0.9.0"]
    },
    "quick_fix": {
      "description": "Use an available image version",
      "code": "client.agents.create({ type: 'CTO', image: 'prism/cto-agent:v1.0.0' })"
    }
  }
}
```

**Network Errors:**
```json
{
  "error": {
    "code": 4201,
    "type": "NETWORK_PARTITION",
    "message": "Network partition detected - some agents are unreachable",
    "impact": {
      "affected_agents": 3,
      "degraded_features": ["real-time sync", "distributed tasks"],
      "estimated_recovery": "2-5 minutes"
    },
    "user_actions": [
      "Current tasks will continue on available agents",
      "New tasks will be queued until network recovers",
      "Check network status at: /dashboard/network"
    ]
  }
}
```

### Error Recovery Workflows

#### Automatic Recovery Patterns
```typescript
// SDK automatically retries transient errors
const retryConfig = {
  maxRetries: 3,
  backoffStrategy: 'exponential', // 1s, 2s, 4s
  retryableErrors: [
    'NETWORK_TIMEOUT',
    'SERVER_ERROR', 
    'RATE_LIMIT_EXCEEDED'
  ]
};

// User-configurable retry behavior
const client = new PrismClient({
  retry: retryConfig,
  onRetry: (attempt, error) => {
    console.log(`Retry attempt ${attempt} for error: ${error.type}`);
  }
});
```

#### Manual Recovery Actions
```typescript
// Provide recovery methods for common scenarios
try {
  await client.agents.create(config);
} catch (error) {
  if (error.code === 'INSUFFICIENT_RESOURCES') {
    // Show resource optimization suggestions
    const optimization = await client.resources.getSuggestions();
    console.log('Resource optimization options:', optimization);
    
    // Offer alternative configurations
    const alternatives = await client.agents.getAlternativeConfigs(config);
    console.log('Alternative configurations:', alternatives);
  }
}
```

---

## Rate Limiting Strategy

### Intelligent Rate Limiting

#### Tier-Based Limits
```yaml
rate_limits:
  free_tier:
    requests_per_minute: 100
    agents_concurrent: 2
    storage_mb: 100
    burst_allowance: 150  # 50% burst capacity
    
  pro_tier:
    requests_per_minute: 1000
    agents_concurrent: 10
    storage_gb: 10
    burst_allowance: 1500
    
  enterprise_tier:
    requests_per_minute: 10000
    agents_concurrent: 100
    storage_tb: 1
    burst_allowance: 15000
```

#### Smart Rate Limiting Algorithm
```typescript
interface RateLimitStrategy {
  // Token bucket with refill based on user behavior
  tokenBucket: {
    capacity: number;
    refillRate: number; // tokens per second
    burstMultiplier: number; // allow temporary bursts
  };
  
  // Priority-based queuing
  priorityQueue: {
    high: number;    // Agent management operations
    medium: number;  // Task operations
    low: number;     // Metrics and monitoring
  };
  
  // Adaptive limits based on system load
  adaptive: {
    enabled: boolean;
    loadThreshold: number; // 0.0-1.0
    scaleFactor: number;   // Reduce limits when load > threshold
  };
}
```

#### Rate Limit Headers
```http
HTTP/1.1 200 OK
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 856
X-RateLimit-Reset: 1642694400
X-RateLimit-Burst-Remaining: 150
X-RateLimit-Retry-After: 60
```

### User Experience Optimizations

#### Proactive Rate Limit Management
```typescript
// SDK tracks rate limits and provides warnings
client.on('rateLimitWarning', (info) => {
  console.log(`Warning: ${info.remaining} requests remaining`);
  console.log(`Rate limit resets in ${info.resetIn}ms`);
  
  // Suggest optimization
  if (info.remaining < 10) {
    console.log('Consider implementing request batching');
    console.log('Upgrade to Pro tier for higher limits');
  }
});

// Automatic request queuing when rate limited
const queuedRequest = await client.agents.create(config, {
  queueIfRateLimited: true,
  maxQueueTime: 30000 // 30 seconds max wait
});
```

#### Developer-Friendly Rate Limit Errors
```json
{
  "error": {
    "code": 4429,
    "type": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded: 100 requests per minute",
    "rate_limit": {
      "limit": 100,
      "remaining": 0,
      "reset": "2025-01-20T21:15:00Z",
      "reset_in_ms": 45000
    },
    "optimization_tips": [
      "Implement request batching to reduce API calls",
      "Use webhooks instead of polling for real-time updates", 
      "Cache frequently accessed data locally",
      "Consider upgrading to Pro tier (1000 req/min)"
    ],
    "batch_endpoints": [
      "POST /api/v1/agents/batch",
      "POST /api/v1/tasks/batch",
      "GET /api/v1/agents?ids=1,2,3,4"
    ]
  }
}
```

#### Request Optimization Guidance
```typescript
// SDK provides optimization suggestions
const usage = await client.usage.getAnalytics({
  period: '24h'
});

if (usage.inefficientPatterns.length > 0) {
  console.log('Optimization opportunities:');
  usage.inefficientPatterns.forEach(pattern => {
    console.log(`• ${pattern.description}`);
    console.log(`  Impact: ${pattern.potentialSavings} fewer requests/day`);
    console.log(`  Fix: ${pattern.suggestedFix}`);
  });
}

// Example output:
// • Polling for task status every second
//   Impact: 2,880 fewer requests/day  
//   Fix: Use WebSocket events or increase polling interval
```

---

## Performance-UX Balance

### Response Time Expectations

#### API Endpoint Performance Targets
```yaml
performance_targets:
  authentication:
    target_ms: 100
    max_acceptable_ms: 500
    
  agent_operations:
    create: 2000  # Agent deployment takes time
    read: 50      # Fast status checks
    update: 500   # Config changes
    delete: 1000  # Graceful shutdown
    
  task_operations:
    create: 200
    read: 50
    update: 100
    cancel: 500
    
  real_time_events:
    websocket_latency: 10
    event_delivery: 50
    connection_setup: 1000
```

#### Progressive Loading Strategy
```typescript
// Load critical data first, then enhance
const agentSummary = await client.agents.getSummary(id); // <50ms
renderAgentCard(agentSummary);

// Load detailed data asynchronously
const agentDetails = await client.agents.getDetails(id); // <500ms
enhanceAgentCard(agentDetails);

// Load heavy data last
const agentMetrics = await client.agents.getMetrics(id, '7d'); // <2000ms
renderMetricsCharts(agentMetrics);
```

### Caching Strategy

#### Multi-Level Caching
```typescript
const client = new PrismClient({
  cache: {
    // Level 1: In-memory cache (fastest)
    memory: {
      enabled: true,
      maxSize: '50MB',
      ttl: 60000 // 1 minute
    },
    
    // Level 2: Browser storage (persistent)
    localStorage: {
      enabled: true,
      maxSize: '200MB', 
      ttl: 3600000 // 1 hour
    },
    
    // Level 3: Service worker (offline)
    serviceWorker: {
      enabled: true,
      strategy: 'stale-while-revalidate'
    }
  }
});
```

### Error Recovery Performance

#### Fast Failure Detection
```typescript
// Circuit breaker pattern for failing services
const circuitBreaker = {
  failureThreshold: 5,     // Fail after 5 consecutive errors
  timeout: 30000,          // 30 second timeout
  resetTimeout: 60000,     // Try again after 1 minute
  
  onOpen: () => {
    // Circuit open - show cached data immediately
    showCachedDashboard();
    displayConnectionWarning();
  },
  
  onClose: () => {
    // Circuit closed - refresh data
    refreshDashboard();
    hideConnectionWarning();
  }
};
```

---

## Next Steps & Implementation

### Phase 2A: API Foundation (Week 1)
- Implement error response format standardization
- Create SDK error handling infrastructure
- Set up rate limiting with user-friendly messages
- Build API documentation generator

### Phase 2B: Developer Experience (Week 2) 
- Complete SDK code examples for all three languages
- Implement interactive API playground
- Create onboarding flow automation
- Add performance monitoring and optimization tips

### Phase 2C: Production Readiness (Week 3)
- Load testing of rate limiting strategies
- Error recovery workflow validation
- SDK performance optimization
- Developer feedback integration

### Success Validation
- **Onboarding Time**: Measure actual time-to-first-success with real developers
- **Error Resolution**: Track error->solution time across different error types
- **Rate Limit Impact**: Monitor request success rates under various load conditions
- **Developer Satisfaction**: NPS scores from API documentation and SDK experience

---

*This API UX specification ensures that PRISM's powerful distributed capabilities are accessible to developers of all skill levels, with a focus on productivity, clarity, and graceful error handling.*