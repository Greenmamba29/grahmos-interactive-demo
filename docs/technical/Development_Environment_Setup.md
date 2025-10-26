# PRISM Development Environment Setup
## CI/CD Pipeline, Testing Infrastructure & Monitoring Systems

**Version:** 1.0.0  
**Date:** 2025-01-20  
**Status:** ✅ IMPLEMENTATION READY  
**Scope:** Complete development environment for 8-person team MVP development

---

## Executive Summary

This document provides the complete development environment setup for PRISM MVP development, including CI/CD pipelines, testing infrastructure, monitoring systems, and development tools optimized for the 8-person team structure.

### Environment Readiness ✅
- **CI/CD Pipeline**: GitHub Actions with automated quality gates
- **Testing Infrastructure**: Comprehensive testing framework with parallel execution
- **Monitoring Systems**: Prometheus + Grafana with real-time alerting
- **Development Tools**: Standardized toolchain for team productivity
- **Infrastructure as Code**: Complete Kubernetes deployment automation

---

## Development Infrastructure Architecture

### Overall Development Ecosystem
```mermaid
graph TB
    subgraph "Development Tools"
        IDE[VS Code + Extensions]
        GIT[Git + GitHub]
        DOCKER[Docker Desktop]
        K8S_LOCAL[Kind/Minikube]
    end
    
    subgraph "CI/CD Pipeline"
        GITHUB[GitHub Actions]
        BUILD[Build & Test]
        SECURITY[Security Scanning]
        DEPLOY[Deployment]
    end
    
    subgraph "Testing Infrastructure"
        UNIT[Unit Tests (Jest)]
        INTEGRATION[Integration Tests]
        E2E[E2E Tests (Playwright)]
        MOBILE[Mobile Testing]
    end
    
    subgraph "Environment Tiers"
        DEV[Development]
        STAGING[Staging]
        PRODUCTION[Production]
    end
    
    subgraph "Monitoring Stack"
        PROMETHEUS[Prometheus]
        GRAFANA[Grafana]
        ALERT[AlertManager]
        LOGS[ELK Stack]
    end
    
    subgraph "Infrastructure"
        AWS[AWS Cloud]
        K8S[Kubernetes]
        TERRAFORM[Terraform]
        HELM[Helm Charts]
    end
    
    IDE --> GIT
    GIT --> GITHUB
    GITHUB --> BUILD
    BUILD --> SECURITY
    SECURITY --> DEPLOY
    
    BUILD --> UNIT
    BUILD --> INTEGRATION
    BUILD --> E2E
    BUILD --> MOBILE
    
    DEPLOY --> DEV
    DEPLOY --> STAGING
    DEPLOY --> PRODUCTION
    
    DEV --> PROMETHEUS
    STAGING --> PROMETHEUS
    PRODUCTION --> PROMETHEUS
    PROMETHEUS --> GRAFANA
    PROMETHEUS --> ALERT
    
    DEPLOY --> AWS
    AWS --> K8S
    K8S --> TERRAFORM
    K8S --> HELM
```

---

## CI/CD Pipeline Implementation

### GitHub Actions Workflow Structure
```yaml
# .github/workflows/main.yml
name: PRISM CI/CD Pipeline

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main, develop]

env:
  NODE_VERSION: '20'
  DOCKER_REGISTRY: 'ghcr.io'
  K8S_NAMESPACE: 'prism-system'

jobs:
  code-quality:
    name: Code Quality & Security
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
          
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ env.NODE_VERSION }}
          cache: 'npm'
          
      - name: Install dependencies
        run: npm ci
        
      - name: Lint code
        run: npm run lint
        
      - name: Format check
        run: npm run format:check
        
      - name: Type check
        run: npm run type-check
        
      - name: Security audit
        run: npm audit --audit-level high
        
      - name: SAST scanning
        uses: github/super-linter@v4
        env:
          DEFAULT_BRANCH: main
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          
  unit-tests:
    name: Unit Tests
    runs-on: ubuntu-latest
    needs: code-quality
    strategy:
      matrix:
        node-version: [18, 20]
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js ${{ matrix.node-version }}
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: 'npm'
          
      - name: Install dependencies
        run: npm ci
        
      - name: Run unit tests
        run: npm run test:unit -- --coverage --maxWorkers=4
        
      - name: Upload coverage to Codecov
        uses: codecov/codecov-action@v3
        with:
          token: ${{ secrets.CODECOV_TOKEN }}
          
  integration-tests:
    name: Integration Tests
    runs-on: ubuntu-latest
    needs: unit-tests
    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: test
          POSTGRES_DB: prism_test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
          
      redis:
        image: redis:7
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
          
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ env.NODE_VERSION }}
          cache: 'npm'
          
      - name: Install dependencies
        run: npm ci
        
      - name: Run database migrations
        run: npm run db:migrate:test
        env:
          DATABASE_URL: postgresql://postgres:test@localhost:5432/prism_test
          
      - name: Run integration tests
        run: npm run test:integration
        env:
          DATABASE_URL: postgresql://postgres:test@localhost:5432/prism_test
          REDIS_URL: redis://localhost:6379
          
  e2e-tests:
    name: E2E Tests
    runs-on: ubuntu-latest
    needs: integration-tests
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ env.NODE_VERSION }}
          cache: 'npm'
          
      - name: Install dependencies
        run: npm ci
        
      - name: Install Playwright browsers
        run: npx playwright install --with-deps
        
      - name: Start test environment
        run: docker-compose -f docker-compose.test.yml up -d
        
      - name: Wait for services
        run: sleep 30
        
      - name: Run E2E tests
        run: npm run test:e2e
        
      - name: Upload test results
        uses: actions/upload-artifact@v3
        if: always()
        with:
          name: e2e-test-results
          path: test-results/
          
  security-scan:
    name: Security Scanning
    runs-on: ubuntu-latest
    needs: code-quality
    steps:
      - uses: actions/checkout@v4
      
      - name: Run Trivy vulnerability scanner
        uses: aquasecurity/trivy-action@master
        with:
          scan-type: 'fs'
          scan-ref: '.'
          format: 'sarif'
          output: 'trivy-results.sarif'
          
      - name: Upload Trivy scan results
        uses: github/codeql-action/upload-sarif@v2
        with:
          sarif_file: 'trivy-results.sarif'
          
      - name: Snyk vulnerability scan
        uses: snyk/actions/node@master
        env:
          SNYK_TOKEN: ${{ secrets.SNYK_TOKEN }}
        with:
          args: --severity-threshold=high
          
  build-and-push:
    name: Build & Push Images
    runs-on: ubuntu-latest
    needs: [unit-tests, integration-tests, security-scan]
    if: github.ref == 'refs/heads/main' || github.ref == 'refs/heads/develop'
    outputs:
      image-tag: ${{ steps.meta.outputs.tags }}
      image-digest: ${{ steps.build.outputs.digest }}
    steps:
      - uses: actions/checkout@v4
      
      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3
        
      - name: Login to Container Registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.DOCKER_REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
          
      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.DOCKER_REGISTRY }}/${{ github.repository }}
          tags: |
            type=ref,event=branch
            type=sha,prefix={{branch}}-
            
      - name: Build and push
        id: build
        uses: docker/build-push-action@v5
        with:
          context: .
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
          
  deploy-staging:
    name: Deploy to Staging
    runs-on: ubuntu-latest
    needs: [build-and-push, e2e-tests]
    if: github.ref == 'refs/heads/develop'
    environment: staging
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup kubectl
        uses: azure/setup-kubectl@v3
        with:
          version: 'v1.28.0'
          
      - name: Setup Helm
        uses: azure/setup-helm@v3
        with:
          version: 'v3.12.0'
          
      - name: Configure AWS credentials
        uses: aws-actions/configure-aws-credentials@v4
        with:
          aws-access-key-id: ${{ secrets.AWS_ACCESS_KEY_ID }}
          aws-secret-access-key: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
          aws-region: us-east-1
          
      - name: Update kubeconfig
        run: aws eks update-kubeconfig --name prism-staging-cluster
        
      - name: Deploy to staging
        run: |
          helm upgrade --install prism-staging ./helm/prism \
            --namespace prism-staging \
            --create-namespace \
            --set image.tag=${{ needs.build-and-push.outputs.image-tag }} \
            --set environment=staging \
            --wait --timeout=10m
            
      - name: Run smoke tests
        run: npm run test:smoke -- --env=staging
        
  deploy-production:
    name: Deploy to Production
    runs-on: ubuntu-latest
    needs: [build-and-push, e2e-tests]
    if: github.ref == 'refs/heads/main'
    environment: production
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup kubectl
        uses: azure/setup-kubectl@v3
        with:
          version: 'v1.28.0'
          
      - name: Setup Helm
        uses: azure/setup-helm@v3
        with:
          version: 'v3.12.0'
          
      - name: Configure AWS credentials
        uses: aws-actions/configure-aws-credentials@v4
        with:
          aws-access-key-id: ${{ secrets.AWS_ACCESS_KEY_ID }}
          aws-secret-access-key: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
          aws-region: us-east-1
          
      - name: Update kubeconfig
        run: aws eks update-kubeconfig --name prism-production-cluster
        
      - name: Deploy to production (Blue-Green)
        run: |
          helm upgrade --install prism-blue ./helm/prism \
            --namespace prism-production \
            --set image.tag=${{ needs.build-and-push.outputs.image-tag }} \
            --set environment=production \
            --set deployment.strategy=blue-green \
            --wait --timeout=15m
            
      - name: Run production smoke tests
        run: npm run test:smoke -- --env=production
        
      - name: Switch traffic to blue deployment
        run: kubectl patch service prism-service -p '{"spec":{"selector":{"deployment":"blue"}}}'
        
      - name: Cleanup green deployment
        run: helm uninstall prism-green --namespace prism-production || true

quality-gates:
  required_checks:
    - "Code Quality & Security"
    - "Unit Tests"
    - "Integration Tests"
    - "E2E Tests"
    - "Security Scanning"
    
  performance_requirements:
    - "Unit test coverage > 90%"
    - "Integration test coverage > 80%"
    - "E2E test pass rate > 95%"
    - "Security scan: Zero critical vulnerabilities"
    - "Build time < 15 minutes"
    - "Deployment time < 10 minutes"
```

### Branch Protection Rules
```yaml
branch_protection:
  main:
    required_status_checks:
      strict: true
      contexts:
        - "Code Quality & Security"
        - "Unit Tests (20)"
        - "Integration Tests"
        - "E2E Tests"
        - "Security Scanning"
    required_reviews:
      required_reviewers: 2
      dismiss_stale_reviews: true
      require_code_owner_reviews: true
    restrictions:
      users: []
      teams: ["senior-developers", "tech-leads"]
    enforce_admins: false
    
  develop:
    required_status_checks:
      strict: true
      contexts:
        - "Code Quality & Security"
        - "Unit Tests (20)"
        - "Integration Tests"
    required_reviews:
      required_reviewers: 1
      dismiss_stale_reviews: true
    restrictions: null
    enforce_admins: false
```

---

## Testing Infrastructure

### Comprehensive Testing Framework
```typescript
// jest.config.js - Comprehensive Jest configuration
module.exports = {
  projects: [
    {
      displayName: 'unit',
      testMatch: ['<rootDir>/src/**/__tests__/**/*.test.ts'],
      testEnvironment: 'node',
      collectCoverageFrom: [
        'src/**/*.{ts,js}',
        '!src/**/*.d.ts',
        '!src/**/__tests__/**',
        '!src/**/node_modules/**'
      ],
      coverageThreshold: {
        global: {
          branches: 90,
          functions: 90,
          lines: 90,
          statements: 90
        }
      }
    },
    {
      displayName: 'integration',
      testMatch: ['<rootDir>/tests/integration/**/*.test.ts'],
      testEnvironment: 'node',
      setupFilesAfterEnv: ['<rootDir>/tests/setup/integration.ts'],
      testTimeout: 30000
    },
    {
      displayName: 'e2e',
      testMatch: ['<rootDir>/tests/e2e/**/*.test.ts'],
      testEnvironment: 'node',
      setupFilesAfterEnv: ['<rootDir>/tests/setup/e2e.ts'],
      testTimeout: 60000
    }
  ],
  coverageReporters: ['json', 'lcov', 'text', 'clover', 'html'],
  maxWorkers: '50%',
  cache: true,
  verbose: true
};

// playwright.config.ts - E2E testing configuration
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: [
    ['html'],
    ['junit', { outputFile: 'test-results/junit-results.xml' }],
    ['json', { outputFile: 'test-results/results.json' }]
  ],
  use: {
    baseURL: process.env.E2E_BASE_URL || 'http://localhost:3000',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure'
  },
  
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] }
    },
    {
      name: 'firefox',
      use: { ...devices['Desktop Firefox'] }
    },
    {
      name: 'webkit',
      use: { ...devices['Desktop Safari'] }
    },
    {
      name: 'Mobile Chrome',
      use: { ...devices['Pixel 5'] }
    },
    {
      name: 'Mobile Safari',
      use: { ...devices['iPhone 12'] }
    }
  ],
  
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:3000',
    reuseExistingServer: !process.env.CI
  }
});
```

### API Testing Framework
```typescript
// tests/api/setup.ts - API testing infrastructure
import { TestEnvironment } from './types';

export class APITestEnvironment implements TestEnvironment {
  private baseURL: string;
  private authToken?: string;
  
  constructor(config: { baseURL: string }) {
    this.baseURL = config.baseURL;
  }
  
  async setup(): Promise<void> {
    // Setup test database
    await this.setupDatabase();
    
    // Setup authentication
    await this.setupAuthentication();
    
    // Setup test data
    await this.setupTestData();
  }
  
  async teardown(): Promise<void> {
    // Cleanup test data
    await this.cleanupTestData();
    
    // Reset database state
    await this.resetDatabase();
  }
  
  async authenticate(credentials?: TestCredentials): Promise<string> {
    const response = await fetch(`${this.baseURL}/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(credentials || this.getDefaultCredentials())
    });
    
    const { token } = await response.json();
    this.authToken = token;
    return token;
  }
  
  async makeRequest(endpoint: string, options: RequestOptions = {}): Promise<Response> {
    const headers = {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${this.authToken}`,
      ...options.headers
    };
    
    return fetch(`${this.baseURL}${endpoint}`, {
      ...options,
      headers
    });
  }
}

// Contract testing with Pact
import { Pact } from '@pact-foundation/pact';

describe('Agent API Contract Tests', () => {
  let provider: Pact;
  
  beforeAll(async () => {
    provider = new Pact({
      consumer: 'PRISM Frontend',
      provider: 'PRISM API',
      port: 1234,
      log: path.resolve(process.cwd(), 'logs', 'pact.log'),
      dir: path.resolve(process.cwd(), 'pacts'),
      logLevel: 'INFO'
    });
    
    await provider.setup();
  });
  
  afterAll(() => provider.finalize());
  
  describe('Agent Management', () => {
    beforeEach(() => {
      return provider.addInteraction({
        state: 'agent exists',
        uponReceiving: 'a request for agent details',
        withRequest: {
          method: 'GET',
          path: '/api/v1/agents/123',
          headers: {
            'Authorization': 'Bearer token'
          }
        },
        willRespondWith: {
          status: 200,
          headers: {
            'Content-Type': 'application/json'
          },
          body: {
            id: '123',
            type: 'code_generation',
            status: 'running',
            createdAt: new Date().toISOString()
          }
        }
      });
    });
    
    it('should return agent details', async () => {
      const response = await apiClient.getAgent('123');
      expect(response.id).toBe('123');
      expect(response.type).toBe('code_generation');
    });
  });
});
```

### Mobile Testing Infrastructure
```yaml
# Mobile testing configuration
mobile_testing:
  frameworks:
    ios: "XCTest with Detox integration"
    android: "Espresso with Detox integration"
    cross_platform: "Detox with React Native"
    
  device_testing:
    physical_devices:
      - "iPhone 14 Pro (iOS 16.0+)"
      - "iPhone 12 (iOS 15.0+)" 
      - "Samsung Galaxy S23 (Android 13)"
      - "Google Pixel 7 (Android 13)"
      
    simulators:
      ios: ["iPhone 14", "iPhone 13", "iPad Air"]
      android: ["Pixel 6", "Galaxy S22", "Nexus 6"]
      
  p2p_testing:
    network_simulation:
      - "3G/4G/5G network conditions"
      - "WiFi with varying signal strength"
      - "Network switching scenarios"
      - "Offline/online transitions"
      
    battery_testing:
      - "Background processing impact"
      - "P2P connection energy usage"
      - "Battery optimization validation"
      - "Adaptive behavior testing"
      
  test_execution:
    parallel_execution: "Up to 10 devices simultaneously"
    test_isolation: "Each test runs in clean environment"
    result_aggregation: "Centralized reporting"
    failure_analysis: "Automatic screenshot/video capture"

# detox.config.js - Mobile E2E testing
module.exports = {
  testRunner: 'jest',
  runnerConfig: 'tests/mobile/jest.config.js',
  
  apps: {
    'ios.debug': {
      type: 'ios.app',
      binaryPath: 'ios/build/Build/Products/Debug-iphonesimulator/PRISM.app',
      build: 'cd ios && xcodebuild -workspace PRISM.xcworkspace -scheme PRISM -configuration Debug -sdk iphonesimulator -derivedDataPath ../build'
    },
    'android.debug': {
      type: 'android.apk',
      binaryPath: 'android/app/build/outputs/apk/debug/app-debug.apk',
      build: 'cd android && ./gradlew assembleDebug assembleAndroidTest -DtestBuildType=debug'
    }
  },
  
  devices: {
    simulator: {
      type: 'ios.simulator',
      device: {
        type: 'iPhone 14'
      }
    },
    emulator: {
      type: 'android.emulator',
      device: {
        avdName: 'Pixel_6_API_31'
      }
    }
  },
  
  configurations: {
    'ios.debug': {
      device: 'simulator',
      app: 'ios.debug'
    },
    'android.debug': {
      device: 'emulator',
      app: 'android.debug'
    }
  }
};
```

---

## Monitoring & Observability

### Prometheus Configuration
```yaml
# prometheus/prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    cluster: 'prism-production'
    region: 'us-east-1'

rule_files:
  - "/etc/prometheus/rules/*.yml"

alerting:
  alertmanagers:
    - static_configs:
        - targets: ['alertmanager:9093']

scrape_configs:
  # PRISM Application Metrics
  - job_name: 'prism-api'
    scrape_interval: 10s
    kubernetes_sd_configs:
      - role: endpoints
        namespaces:
          names: ['prism-system', 'prism-agents']
    relabel_configs:
      - source_labels: [__meta_kubernetes_service_annotation_prometheus_io_scrape]
        action: keep
        regex: true
      - source_labels: [__meta_kubernetes_service_annotation_prometheus_io_path]
        action: replace
        target_label: __metrics_path__
        regex: (.+)
        
  # Sub-Agent Metrics
  - job_name: 'prism-agents'
    scrape_interval: 30s
    kubernetes_sd_configs:
      - role: pod
        namespaces:
          names: ['prism-agents']
    relabel_configs:
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
        action: keep
        regex: true
        
  # Database Metrics
  - job_name: 'postgresql'
    static_configs:
      - targets: ['postgres-exporter:9187']
    scrape_interval: 30s
    
  # Redis Metrics
  - job_name: 'redis'
    static_configs:
      - targets: ['redis-exporter:9121']
    scrape_interval: 30s
    
  # Kubernetes Metrics
  - job_name: 'kube-state-metrics'
    static_configs:
      - targets: ['kube-state-metrics:8080']
    scrape_interval: 30s
    
  # Node Metrics
  - job_name: 'node-exporter'
    kubernetes_sd_configs:
      - role: node
    relabel_configs:
      - source_labels: [__address__]
        regex: '(.*):10250'
        target_label: __address__
        replacement: '${1}:9100'

# Alert Rules
groups:
  - name: prism-alerts
    rules:
      # API Performance
      - alert: HighAPILatency
        expr: histogram_quantile(0.95, http_request_duration_seconds_bucket{job="prism-api"}) > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High API latency detected"
          description: "95th percentile latency is {{ $value }}s"
          
      - alert: APIErrorRate
        expr: rate(http_requests_total{job="prism-api",status=~"5.."}[5m]) > 0.01
        for: 3m
        labels:
          severity: critical
        annotations:
          summary: "High API error rate"
          description: "Error rate is {{ $value }} requests per second"
          
      # Agent Health
      - alert: AgentSpawnFailure
        expr: rate(prism_agent_spawn_failures_total[5m]) > 0.1
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "Agent spawn failures detected"
          description: "Agent spawn failure rate: {{ $value }} per second"
          
      - alert: AgentResourceExhaustion
        expr: prism_agent_resource_usage_ratio > 0.9
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Agent resource exhaustion"
          description: "Agent resource usage at {{ $value * 100 }}%"
          
      # System Health
      - alert: DatabaseConnectionFailure
        expr: postgres_up == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Database connection failure"
          description: "PostgreSQL database is down"
          
      - alert: RedisConnectionFailure
        expr: redis_up == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Redis connection failure"
          description: "Redis cache is down"
```

### Grafana Dashboard Configuration
```json
{
  "dashboard": {
    "id": null,
    "title": "PRISM System Overview",
    "tags": ["prism", "monitoring"],
    "timezone": "browser",
    "panels": [
      {
        "id": 1,
        "title": "API Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(http_requests_total{job=\"prism-api\"}[5m])",
            "legendFormat": "{{method}} {{endpoint}}"
          }
        ],
        "yAxes": [
          {
            "label": "Requests/sec"
          }
        ],
        "gridPos": {
          "h": 8,
          "w": 12,
          "x": 0,
          "y": 0
        }
      },
      {
        "id": 2,
        "title": "API Response Time",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, http_request_duration_seconds_bucket{job=\"prism-api\"})",
            "legendFormat": "95th percentile"
          },
          {
            "expr": "histogram_quantile(0.50, http_request_duration_seconds_bucket{job=\"prism-api\"})",
            "legendFormat": "50th percentile"
          }
        ],
        "yAxes": [
          {
            "label": "Seconds"
          }
        ],
        "gridPos": {
          "h": 8,
          "w": 12,
          "x": 12,
          "y": 0
        }
      },
      {
        "id": 3,
        "title": "Active Agents",
        "type": "stat",
        "targets": [
          {
            "expr": "prism_agents_active_total"
          }
        ],
        "gridPos": {
          "h": 4,
          "w": 6,
          "x": 0,
          "y": 8
        }
      },
      {
        "id": 4,
        "title": "Agent Spawn Rate",
        "type": "stat",
        "targets": [
          {
            "expr": "rate(prism_agent_spawns_total[5m])"
          }
        ],
        "gridPos": {
          "h": 4,
          "w": 6,
          "x": 6,
          "y": 8
        }
      },
      {
        "id": 5,
        "title": "System Resource Usage",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(container_cpu_usage_seconds_total{pod=~\"prism-.*\"}[5m]) * 100",
            "legendFormat": "CPU % - {{pod}}"
          },
          {
            "expr": "container_memory_usage_bytes{pod=~\"prism-.*\"} / 1024 / 1024 / 1024",
            "legendFormat": "Memory GB - {{pod}}"
          }
        ],
        "gridPos": {
          "h": 8,
          "w": 24,
          "x": 0,
          "y": 12
        }
      }
    ],
    "time": {
      "from": "now-1h",
      "to": "now"
    },
    "refresh": "10s"
  }
}
```

### Alertmanager Configuration
```yaml
# alertmanager/alertmanager.yml
global:
  smtp_smarthost: 'smtp.gmail.com:587'
  smtp_from: 'alerts@prism.dev'
  slack_api_url: 'https://hooks.slack.com/services/...'

templates:
  - '/etc/alertmanager/templates/*.tmpl'

route:
  group_by: ['alertname', 'cluster', 'service']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 1h
  receiver: 'default-receiver'
  routes:
    - match:
        severity: critical
      receiver: 'critical-alerts'
      group_wait: 5s
      repeat_interval: 30m
    - match:
        severity: warning
      receiver: 'warning-alerts'
      group_wait: 30s
      repeat_interval: 4h

receivers:
  - name: 'default-receiver'
    slack_configs:
      - channel: '#prism-alerts'
        title: 'PRISM Alert: {{ .GroupLabels.alertname }}'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'
        
  - name: 'critical-alerts'
    slack_configs:
      - channel: '#prism-critical'
        title: 'CRITICAL: {{ .GroupLabels.alertname }}'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'
        color: 'danger'
    email_configs:
      - to: 'oncall@prism.dev'
        subject: 'CRITICAL ALERT: {{ .GroupLabels.alertname }}'
        body: |
          {{ range .Alerts }}
          Alert: {{ .Annotations.summary }}
          Description: {{ .Annotations.description }}
          {{ end }}
          
  - name: 'warning-alerts'
    slack_configs:
      - channel: '#prism-alerts'
        title: 'Warning: {{ .GroupLabels.alertname }}'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'
        color: 'warning'

inhibit_rules:
  - source_match:
      severity: 'critical'
    target_match:
      severity: 'warning'
    equal: ['alertname', 'cluster', 'service']
```

---

## Development Tools & Standards

### VS Code Configuration
```json
// .vscode/settings.json
{
  "typescript.preferences.importModuleSpecifier": "relative",
  "editor.formatOnSave": true,
  "editor.codeActionsOnSave": {
    "source.fixAll.eslint": true,
    "source.organizeImports": true
  },
  "files.exclude": {
    "**/node_modules": true,
    "**/dist": true,
    "**/build": true,
    "**/.nyc_output": true,
    "**/coverage": true
  },
  "search.exclude": {
    "**/node_modules": true,
    "**/dist": true,
    "**/build": true,
    "**/coverage": true
  },
  "typescript.updateImportsOnFileMove.enabled": "always",
  "eslint.workingDirectories": ["src", "tests"],
  "prettier.configPath": ".prettierrc",
  "jest.jestCommandLine": "npm run test:unit"
}

// .vscode/extensions.json
{
  "recommendations": [
    "ms-vscode.vscode-typescript-next",
    "esbenp.prettier-vscode",
    "dbaeumer.vscode-eslint",
    "bradlc.vscode-tailwindcss",
    "ms-vscode.vscode-json",
    "redhat.vscode-yaml",
    "ms-kubernetes-tools.vscode-kubernetes-tools",
    "ms-vscode-remote.remote-containers",
    "github.copilot",
    "orta.vscode-jest"
  ]
}

// .vscode/launch.json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Debug Node.js App",
      "type": "node",
      "request": "launch",
      "program": "${workspaceFolder}/src/main.ts",
      "args": [],
      "runtimeArgs": ["-r", "ts-node/register"],
      "env": {
        "NODE_ENV": "development"
      },
      "console": "integratedTerminal",
      "sourceMaps": true,
      "restart": true,
      "protocol": "inspector"
    },
    {
      "name": "Debug Jest Tests",
      "type": "node",
      "request": "launch",
      "program": "${workspaceFolder}/node_modules/.bin/jest",
      "args": ["--runInBand", "--testPathPattern=${fileBasenameNoExtension}"],
      "console": "integratedTerminal",
      "internalConsoleOptions": "neverOpen",
      "sourceMaps": true
    }
  ]
}
```

### Code Quality Configuration
```javascript
// eslint.config.js
import js from '@eslint/js';
import typescript from '@typescript-eslint/eslint-plugin';
import typescriptParser from '@typescript-eslint/parser';
import prettier from 'eslint-plugin-prettier';
import security from 'eslint-plugin-security';
import jest from 'eslint-plugin-jest';

export default [
  js.configs.recommended,
  {
    files: ['**/*.ts', '**/*.js'],
    languageOptions: {
      parser: typescriptParser,
      parserOptions: {
        ecmaVersion: 2022,
        sourceType: 'module',
        project: './tsconfig.json'
      }
    },
    plugins: {
      '@typescript-eslint': typescript,
      'prettier': prettier,
      'security': security,
      'jest': jest
    },
    rules: {
      ...typescript.configs.recommended.rules,
      ...security.configs.recommended.rules,
      'prettier/prettier': 'error',
      '@typescript-eslint/no-unused-vars': 'error',
      '@typescript-eslint/no-explicit-any': 'warn',
      '@typescript-eslint/explicit-function-return-type': 'warn',
      'security/detect-object-injection': 'error',
      'security/detect-non-literal-regexp': 'error'
    }
  },
  {
    files: ['**/*.test.ts', '**/*.spec.ts'],
    rules: {
      ...jest.configs.recommended.rules,
      'jest/expect-expect': 'error',
      'jest/no-disabled-tests': 'warn',
      'jest/no-focused-tests': 'error'
    }
  }
];

// prettier.config.js
export default {
  semi: true,
  singleQuote: true,
  tabWidth: 2,
  trailingComma: 'es5',
  printWidth: 100,
  bracketSpacing: true,
  arrowParens: 'always'
};

// husky configuration (.husky/pre-commit)
#!/usr/bin/env sh
. "$(dirname -- "$0")/_/husky.sh"

echo "Running pre-commit hooks..."

# Lint staged files
npx lint-staged

# Run type checking
npm run type-check

# Run affected unit tests
npm run test:unit -- --findRelatedTests --passWithNoTests

echo "Pre-commit hooks completed successfully!"
```

### Docker Development Environment
```dockerfile
# Dockerfile.dev - Development container
FROM node:20-alpine

RUN apk add --no-cache \
    git \
    python3 \
    make \
    g++ \
    postgresql-client \
    redis-tools \
    curl

WORKDIR /app

# Install dependencies
COPY package*.json ./
RUN npm ci --only=development && npm cache clean --force

# Setup development user
RUN addgroup -g 1001 -S nodejs
RUN adduser -S nodejs -u 1001
USER nodejs

# Copy source code
COPY --chown=nodejs:nodejs . .

# Expose ports
EXPOSE 3000 9229 9230

# Development command with debugging
CMD ["npm", "run", "dev:debug"]

# docker-compose.dev.yml - Development services
version: '3.8'

services:
  app:
    build:
      context: .
      dockerfile: Dockerfile.dev
    ports:
      - "3000:3000"
      - "9229:9229"  # Node.js debugger
    volumes:
      - .:/app
      - /app/node_modules
    environment:
      - NODE_ENV=development
      - DATABASE_URL=postgresql://postgres:password@postgres:5432/prism_dev
      - REDIS_URL=redis://redis:6379
    depends_on:
      - postgres
      - redis
    command: npm run dev:watch
    
  postgres:
    image: postgres:15-alpine
    ports:
      - "5432:5432"
    environment:
      - POSTGRES_DB=prism_dev
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=password
    volumes:
      - postgres_dev_data:/var/lib/postgresql/data
      - ./scripts/db/init.sql:/docker-entrypoint-initdb.d/init.sql
    
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_dev_data:/data
    command: redis-server --appendonly yes
    
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./monitoring/prometheus:/etc/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'
      
  grafana:
    image: grafana/grafana:latest
    ports:
      - "3001:3000"
    volumes:
      - grafana_dev_data:/var/lib/grafana
      - ./monitoring/grafana/dashboards:/var/lib/grafana/dashboards
      - ./monitoring/grafana/provisioning:/etc/grafana/provisioning
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin123
      
volumes:
  postgres_dev_data:
  redis_dev_data:
  grafana_dev_data:
```

---

## Infrastructure as Code

### Terraform Configuration
```hcl
# terraform/main.tf - AWS infrastructure
terraform {
  required_version = ">= 1.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.20"
    }
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.10"
    }
  }
  
  backend "s3" {
    bucket = "prism-terraform-state"
    key    = "infrastructure/terraform.tfstate"
    region = "us-east-1"
  }
}

provider "aws" {
  region = var.aws_region
  default_tags {
    tags = {
      Project     = "PRISM"
      Environment = var.environment
      ManagedBy   = "Terraform"
    }
  }
}

# EKS Cluster
module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 19.0"

  cluster_name    = "prism-${var.environment}"
  cluster_version = "1.28"

  vpc_id                         = module.vpc.vpc_id
  subnet_ids                     = module.vpc.private_subnets
  cluster_endpoint_public_access = true

  eks_managed_node_groups = {
    system = {
      name = "system"
      instance_types = ["t3.medium"]
      min_size     = 3
      max_size     = 5
      desired_size = 3
      
      labels = {
        role = "system"
      }
      
      taints = {
        dedicated = {
          key    = "system"
          value  = "true"
          effect = "NO_SCHEDULE"
        }
      }
    }
    
    application = {
      name = "application"
      instance_types = ["t3.large"]
      min_size     = 5
      max_size     = 20
      desired_size = 5
      
      labels = {
        role = "application"
      }
    }
    
    agents = {
      name = "agents"
      instance_types = ["t3.xlarge"]
      min_size     = 5
      max_size     = 50
      desired_size = 10
      
      labels = {
        role = "agents"
      }
    }
  }
}

# RDS PostgreSQL
module "database" {
  source = "terraform-aws-modules/rds/aws"

  identifier = "prism-${var.environment}"

  engine            = "postgres"
  engine_version    = "15.3"
  instance_class    = "db.r6g.large"
  allocated_storage = 100
  storage_encrypted = true

  db_name  = "prism"
  username = "prism_admin"
  port     = "5432"

  manage_master_user_password = true

  vpc_security_group_ids = [aws_security_group.rds.id]
  db_subnet_group_name   = module.vpc.database_subnet_group

  backup_retention_period = 7
  backup_window          = "03:00-04:00"
  maintenance_window     = "Sun:04:00-Sun:05:00"

  enabled_cloudwatch_logs_exports = ["postgresql", "upgrade"]
  
  deletion_protection = var.environment == "production"
}

# ElastiCache Redis
resource "aws_elasticache_replication_group" "redis" {
  replication_group_id         = "prism-${var.environment}"
  description                  = "Redis cluster for PRISM ${var.environment}"
  
  node_type                   = "cache.r6g.large"
  parameter_group_name        = "default.redis7"
  port                        = 6379
  
  num_cache_clusters          = 3
  automatic_failover_enabled  = true
  multi_az_enabled           = true
  
  subnet_group_name          = aws_elasticache_subnet_group.redis.name
  security_group_ids         = [aws_security_group.redis.id]
  
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  
  log_delivery_configuration {
    destination      = aws_cloudwatch_log_group.redis.name
    destination_type = "cloudwatch-logs"
    log_format       = "text"
    log_type         = "slow-log"
  }
}

# Variables
variable "aws_region" {
  description = "AWS region"
  type        = string
  default     = "us-east-1"
}

variable "environment" {
  description = "Environment name"
  type        = string
  validation {
    condition     = contains(["development", "staging", "production"], var.environment)
    error_message = "Environment must be development, staging, or production."
  }
}

# Outputs
output "cluster_endpoint" {
  description = "Endpoint for EKS control plane"
  value       = module.eks.cluster_endpoint
}

output "database_endpoint" {
  description = "RDS instance endpoint"
  value       = module.database.db_instance_endpoint
  sensitive   = true
}

output "redis_endpoint" {
  description = "Redis cluster endpoint"
  value       = aws_elasticache_replication_group.redis.primary_endpoint_address
  sensitive   = true
}
```

### Helm Charts
```yaml
# helm/prism/Chart.yaml
apiVersion: v2
name: prism
description: PRISM Agent Platform Helm Chart
type: application
version: 0.1.0
appVersion: "0.1.0"

dependencies:
  - name: postgresql
    version: 12.8.0
    repository: https://charts.bitnami.com/bitnami
    condition: postgresql.enabled
  - name: redis
    version: 17.11.0
    repository: https://charts.bitnami.com/bitnami
    condition: redis.enabled

# helm/prism/values.yaml
image:
  repository: ghcr.io/prism/prism
  tag: ""
  pullPolicy: IfNotPresent

replicaCount: 3

service:
  type: ClusterIP
  port: 3000
  targetPort: 3000

ingress:
  enabled: true
  className: "nginx"
  annotations:
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
    nginx.ingress.kubernetes.io/rate-limit: "100"
  hosts:
    - host: api.prism.dev
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: prism-tls
      hosts:
        - api.prism.dev

resources:
  limits:
    cpu: 1000m
    memory: 2Gi
  requests:
    cpu: 500m
    memory: 1Gi

autoscaling:
  enabled: true
  minReplicas: 3
  maxReplicas: 50
  targetCPUUtilizationPercentage: 70
  targetMemoryUtilizationPercentage: 80

nodeSelector:
  role: application

tolerations: []

affinity:
  podAntiAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
      - weight: 100
        podAffinityTerm:
          labelSelector:
            matchExpressions:
              - key: app.kubernetes.io/name
                operator: In
                values:
                  - prism
          topologyKey: kubernetes.io/hostname

# Environment configuration
environment: production

config:
  database:
    host: ""
    port: 5432
    name: prism
    ssl: true
  redis:
    host: ""
    port: 6379
    ssl: true
  auth:
    jwtSecret: ""
    jwtExpiration: "1h"
  features:
    subAgents: true
    p2pNetworking: true
    enterpriseFeatures: true

# External dependencies
postgresql:
  enabled: false  # Use external RDS instance
  
redis:
  enabled: false  # Use external ElastiCache
  
monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 30s
    path: /metrics
```

---

## Team Development Workflow

### Development Process
```yaml
team_workflow:
  development_cycle:
    1: "Feature branch creation from develop"
    2: "Local development with hot-reload"
    3: "Unit tests and integration tests locally"
    4: "Code review via Pull Request"
    5: "Automated CI/CD pipeline validation"
    6: "Deployment to staging environment"
    7: "QA validation and acceptance testing"
    8: "Merge to main and production deployment"
    
  branch_strategy:
    main: "Production-ready code"
    develop: "Integration branch for features"
    feature/*: "Individual feature development"
    hotfix/*: "Critical production fixes"
    release/*: "Release preparation branches"
    
  code_review_process:
    required_reviewers: 2
    automated_checks:
      - "All tests passing"
      - "Code coverage > 90%"
      - "Security scan clean"
      - "Performance benchmarks met"
    review_criteria:
      - "Code quality and maintainability"
      - "Architecture compliance"
      - "Security considerations"
      - "Documentation completeness"
      
  deployment_strategy:
    staging: "Automatic on develop branch merge"
    production: "Manual approval after staging validation"
    rollback: "Automatic on health check failures"
    monitoring: "Real-time metrics and alerting"
```

### Development Commands
```json
{
  "scripts": {
    "dev": "concurrently \"npm run dev:api\" \"npm run dev:frontend\"",
    "dev:api": "nodemon --exec ts-node src/main.ts",
    "dev:frontend": "vite dev --port 3001",
    "dev:debug": "nodemon --inspect=0.0.0.0:9229 --exec ts-node src/main.ts",
    "dev:watch": "nodemon --watch src --exec npm run dev",
    
    "build": "npm run build:api && npm run build:frontend",
    "build:api": "tsc --build tsconfig.build.json",
    "build:frontend": "vite build",
    "build:docker": "docker build -t prism:latest .",
    
    "test": "npm run test:unit && npm run test:integration",
    "test:unit": "jest --config jest.config.js --testPathPattern=src",
    "test:integration": "jest --config jest.config.js --testPathPattern=tests/integration",
    "test:e2e": "playwright test",
    "test:mobile": "detox test",
    "test:coverage": "jest --coverage --coverageReporters=lcov",
    "test:watch": "jest --watch",
    
    "lint": "eslint src tests --ext .ts,.js",
    "lint:fix": "eslint src tests --ext .ts,.js --fix",
    "format": "prettier --write src tests",
    "format:check": "prettier --check src tests",
    "type-check": "tsc --noEmit",
    
    "db:migrate": "knex migrate:latest",
    "db:migrate:test": "NODE_ENV=test knex migrate:latest",
    "db:seed": "knex seed:run",
    "db:reset": "knex migrate:rollback --all && npm run db:migrate && npm run db:seed",
    
    "docker:dev": "docker-compose -f docker-compose.dev.yml up",
    "docker:test": "docker-compose -f docker-compose.test.yml up --abort-on-container-exit",
    "k8s:dev": "skaffold dev",
    "k8s:deploy": "helm upgrade --install prism-dev ./helm/prism",
    
    "start": "node dist/main.js",
    "start:prod": "NODE_ENV=production node dist/main.js"
  }
}
```

---

## Success Metrics & Monitoring

### Development Environment KPIs
```yaml
development_kpis:
  productivity_metrics:
    - "Average build time: <5 minutes"
    - "Test execution time: <10 minutes"
    - "Local development setup: <30 minutes"
    - "CI/CD pipeline duration: <15 minutes"
    
  quality_metrics:
    - "Code coverage: >90%"
    - "Test pass rate: >95%"
    - "Security scan: Zero critical vulnerabilities"
    - "Code review turnaround: <24 hours"
    
  reliability_metrics:
    - "CI/CD success rate: >95%"
    - "Environment uptime: >99.9%"
    - "Automated test stability: >98%"
    - "Deployment success rate: >99%"
    
  team_efficiency:
    - "Feature delivery velocity: >80% story points"
    - "Bug escape rate: <2%"
    - "Mean time to resolution: <4 hours"
    - "Developer satisfaction: >4.5/5"
```

### Environment Health Dashboard
```yaml
monitoring_dashboard:
  ci_cd_metrics:
    - "Build success rate over time"
    - "Test execution duration trends"
    - "Deployment frequency and success rate"
    - "Pipeline failure analysis"
    
  infrastructure_health:
    - "Kubernetes cluster resource usage"
    - "Database performance and connections"
    - "Cache hit rates and performance"
    - "Application response times"
    
  development_velocity:
    - "Feature throughput per sprint"
    - "Code quality trend analysis"
    - "Technical debt accumulation"
    - "Team productivity metrics"
```

---

## Implementation Timeline

### Week 1: Foundation Setup
```yaml
week1_deliverables:
  - [ ] GitHub repository with branch protection
  - [ ] Basic CI/CD pipeline implementation
  - [ ] Local development environment setup
  - [ ] Docker containers and docker-compose
  - [ ] Initial testing framework
  - [ ] Code quality tools configuration
  
team_assignments:
  tech_lead: "CI/CD pipeline and GitHub setup"
  devops_engineer: "Docker and local environment"
  qa_engineer: "Testing framework foundation"
  backend_engineers: "API development setup"
  frontend_engineers: "Frontend development setup"
```

### Week 2: Advanced Features
```yaml
week2_deliverables:
  - [ ] Complete monitoring stack deployment
  - [ ] E2E testing framework with Playwright
  - [ ] Mobile testing infrastructure
  - [ ] Kubernetes deployment automation
  - [ ] Security scanning integration
  - [ ] Performance testing setup
  
validation_criteria:
  - "All CI/CD quality gates functional"
  - "Local development fully operational"
  - "Monitoring dashboards displaying data"
  - "Testing frameworks executing successfully"
  - "Team onboarding completed"
```

---

## Conclusion

This comprehensive development environment setup provides the foundation for productive, high-quality PRISM MVP development. The infrastructure supports:

✅ **Developer Productivity**: Streamlined local development with hot-reload and debugging  
✅ **Quality Assurance**: Comprehensive testing with 90%+ coverage requirements  
✅ **Security**: Automated security scanning and vulnerability management  
✅ **Monitoring**: Real-time observability with proactive alerting  
✅ **Scalability**: Container-based deployment with auto-scaling capabilities  

The environment is ready for immediate team onboarding and MVP development start.

---

*This Development Environment Setup ensures maximum team productivity and quality delivery for the 8-week PRISM MVP development cycle.*