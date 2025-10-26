#!/usr/bin/env node

const express = require('express');
const WebSocket = require('ws');
const axios = require('axios');
const fs = require('fs').promises;
const path = require('path');

const app = express();
const PORT = 3001;

// Test results storage
let testResults = [];
let realTimeMetrics = {
    totalTests: 0,
    passedTests: 0,
    failedTests: 0,
    averageExecutionTime: 0,
    mobileSpecificMetrics: {
        iosBatteryImpact: 0,
        androidBatteryImpact: 0,
        p2pMeshStability: 0,
        networkRecoveryTime: 0
    }
};

// WebSocket server for real-time updates
const wss = new WebSocket.Server({ port: 3002 });

app.use(express.json());

// Health check endpoint
app.get('/health', (req, res) => {
    res.json({ status: 'healthy', timestamp: new Date().toISOString() });
});

// Submit test results
app.post('/results', async (req, res) => {
    try {
        const result = {
            ...req.body,
            timestamp: new Date().toISOString(),
            id: Date.now().toString()
        };
        
        testResults.push(result);
        updateMetrics(result);
        
        // Broadcast to WebSocket clients
        broadcastUpdate('test_result', result);
        
        // Save to file
        await saveResults();
        
        console.log(`📊 Test result collected: ${result.testName} - ${result.status}`);
        
        res.json({ status: 'success', id: result.id });
    } catch (error) {
        console.error('Error saving test result:', error);
        res.status(500).json({ error: 'Failed to save test result' });
    }
});

// Get all test results
app.get('/results', (req, res) => {
    res.json({
        results: testResults,
        metrics: realTimeMetrics,
        count: testResults.length
    });
});

// Get real-time metrics
app.get('/metrics', (req, res) => {
    res.json(realTimeMetrics);
});

// Mobile-specific metrics endpoint
app.get('/mobile-metrics', (req, res) => {
    const mobileTests = testResults.filter(r => 
        r.platform === 'ios' || r.platform === 'android'
    );
    
    const iosTests = mobileTests.filter(r => r.platform === 'ios');
    const androidTests = mobileTests.filter(r => r.platform === 'android');
    
    const metrics = {
        totalMobileTests: mobileTests.length,
        iosTests: {
            count: iosTests.length,
            successRate: calculateSuccessRate(iosTests),
            averageBatteryImpact: calculateAverageBatteryImpact(iosTests)
        },
        androidTests: {
            count: androidTests.length,
            successRate: calculateSuccessRate(androidTests),
            averageBatteryImpact: calculateAverageBatteryImpact(androidTests)
        },
        p2pMeshMetrics: calculateP2PMeshMetrics(mobileTests),
        networkRecoveryMetrics: calculateNetworkRecoveryMetrics(mobileTests)
    };
    
    res.json(metrics);
});

function updateMetrics(result) {
    realTimeMetrics.totalTests++;
    
    if (result.status === 'passed') {
        realTimeMetrics.passedTests++;
    } else {
        realTimeMetrics.failedTests++;
    }
    
    // Update average execution time
    const totalTime = realTimeMetrics.averageExecutionTime * (realTimeMetrics.totalTests - 1) + 
                     (result.executionTime || 0);
    realTimeMetrics.averageExecutionTime = totalTime / realTimeMetrics.totalTests;
    
    // Update mobile-specific metrics
    if (result.platform === 'ios' && result.batteryImpact) {
        realTimeMetrics.mobileSpecificMetrics.iosBatteryImpact = 
            (realTimeMetrics.mobileSpecificMetrics.iosBatteryImpact + result.batteryImpact) / 2;
    }
    
    if (result.platform === 'android' && result.batteryImpact) {
        realTimeMetrics.mobileSpecificMetrics.androidBatteryImpact = 
            (realTimeMetrics.mobileSpecificMetrics.androidBatteryImpact + result.batteryImpact) / 2;
    }
    
    if (result.p2pMeshStability) {
        realTimeMetrics.mobileSpecificMetrics.p2pMeshStability = 
            (realTimeMetrics.mobileSpecificMetrics.p2pMeshStability + result.p2pMeshStability) / 2;
    }
    
    if (result.networkRecoveryTime) {
        realTimeMetrics.mobileSpecificMetrics.networkRecoveryTime = 
            (realTimeMetrics.mobileSpecificMetrics.networkRecoveryTime + result.networkRecoveryTime) / 2;
    }
}

function broadcastUpdate(type, data) {
    const message = JSON.stringify({ type, data, timestamp: new Date().toISOString() });
    
    wss.clients.forEach(client => {
        if (client.readyState === WebSocket.OPEN) {
            client.send(message);
        }
    });
}

async function saveResults() {
    try {
        const data = {
            results: testResults,
            metrics: realTimeMetrics,
            lastUpdated: new Date().toISOString()
        };
        
        await fs.writeFile('/app/results/test-results.json', JSON.stringify(data, null, 2));
    } catch (error) {
        console.error('Error saving results to file:', error);
    }
}

function calculateSuccessRate(tests) {
    if (tests.length === 0) return 0;
    const passed = tests.filter(t => t.status === 'passed').length;
    return (passed / tests.length) * 100;
}

function calculateAverageBatteryImpact(tests) {
    const testsWithBattery = tests.filter(t => t.batteryImpact !== undefined);
    if (testsWithBattery.length === 0) return 0;
    
    const total = testsWithBattery.reduce((sum, t) => sum + t.batteryImpact, 0);
    return total / testsWithBattery.length;
}

function calculateP2PMeshMetrics(tests) {
    const p2pTests = tests.filter(t => t.p2pMeshStability !== undefined);
    if (p2pTests.length === 0) return { stability: 0, recoveryTime: 0 };
    
    const avgStability = p2pTests.reduce((sum, t) => sum + t.p2pMeshStability, 0) / p2pTests.length;
    const avgRecovery = p2pTests.reduce((sum, t) => sum + (t.meshRecoveryTime || 0), 0) / p2pTests.length;
    
    return {
        stability: avgStability,
        recoveryTime: avgRecovery
    };
}

function calculateNetworkRecoveryMetrics(tests) {
    const networkTests = tests.filter(t => t.networkRecoveryTime !== undefined);
    if (networkTests.length === 0) return { averageRecoveryTime: 0, successRate: 0 };
    
    const avgRecovery = networkTests.reduce((sum, t) => sum + t.networkRecoveryTime, 0) / networkTests.length;
    const successfulRecoveries = networkTests.filter(t => t.networkRecoverySuccess === true).length;
    const successRate = (successfulRecoveries / networkTests.length) * 100;
    
    return {
        averageRecoveryTime: avgRecovery,
        successRate
    };
}

// WebSocket connection handler
wss.on('connection', (ws) => {
    console.log('📱 New WebSocket client connected');
    
    // Send current metrics to new client
    ws.send(JSON.stringify({
        type: 'initial_metrics',
        data: realTimeMetrics,
        timestamp: new Date().toISOString()
    }));
    
    ws.on('close', () => {
        console.log('📱 WebSocket client disconnected');
    });
});

// Start the server
app.listen(PORT, () => {
    console.log(`🚀 Test Results Collector running on port ${PORT}`);
    console.log(`📡 WebSocket server running on port 3002`);
    console.log(`📊 Real-time metrics collection enabled`);
});

// Graceful shutdown
process.on('SIGINT', async () => {
    console.log('💾 Saving final results...');
    await saveResults();
    console.log('👋 Test Results Collector shutting down');
    process.exit(0);
});
