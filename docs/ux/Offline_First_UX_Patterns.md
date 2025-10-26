# PRISM Offline-First UX Design Patterns
## Comprehensive Connection State Management & CRDT User Experience

**Version:** 2.0.0  
**Date:** 2025-01-20  
**Prepared by:** Product Manager Agent  
**Status:** Phase 2 - Ready for Implementation  
**Target**: Seamless offline experience across all connection states

---

## Executive Summary

This specification defines user experience patterns for PRISM's offline-first architecture, ensuring users can work productively regardless of connectivity. The patterns cover all connection states, conflict resolution workflows, and mobile-optimized interactions that maintain the illusion of continuous operation.

### Success Criteria
- **Connection States**: UI patterns for all 5 connection states with clear user feedback
- **Conflict Resolution**: Intuitive CRDT merge conflict workflows with user control
- **Sync Progress**: Time-estimated progress indicators with queue management
- **Mobile Experience**: Touch-optimized offline workflows with gesture support

---

## Connection State Management

### The 5 Connection States

#### State 1: Fully Connected 🟢
**Characteristics**: Real-time P2P mesh connectivity, all features available
**User Indicators**: 
- Green connection indicator in header
- Real-time status updates
- Instant message delivery confirmation
- Live collaboration cursors/indicators

```jsx
// Connection Status Component
const ConnectionStatus = ({ state, peers, latency }) => {
  if (state === 'CONNECTED') {
    return (
      <div className="flex items-center space-x-2 text-green-600">
        <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
        <span className="text-sm font-medium">Connected</span>
        <span className="text-xs text-gray-500">
          {peers} peers • {latency}ms
        </span>
      </div>
    );
  }
};
```

**User Experience Patterns:**
```jsx
// Real-time Agent Status Updates
<AgentCard agent={agent}>
  <StatusBadge 
    status={agent.status} 
    lastUpdate={agent.lastHeartbeat}
    realTime={true}
  />
  <TaskProgress 
    progress={agent.currentTask?.progress} 
    streaming={true}
  />
  {/* Live indicators for active agents */}
  {agent.status === 'active' && (
    <div className="flex items-center">
      <div className="w-1 h-1 bg-green-400 rounded-full animate-pulse mr-1" />
      <span className="text-xs text-green-600">Working now</span>
    </div>
  )}
</AgentCard>
```

#### State 2: Degraded Connection 🟡
**Characteristics**: Intermittent connectivity, some peers unreachable, increased latency
**User Indicators**:
- Yellow warning indicator
- Network quality bars (similar to mobile signal)
- Affected feature notifications
- Automatic retry indicators

```jsx
const DegradedConnectionUI = ({ availablePeers, totalPeers, degradedFeatures }) => (
  <div className="bg-yellow-50 border-l-4 border-yellow-400 p-4 mb-4">
    <div className="flex">
      <ExclamationTriangleIcon className="h-5 w-5 text-yellow-400" />
      <div className="ml-3">
        <p className="text-sm text-yellow-700">
          Network connection is degraded
        </p>
        <div className="mt-2 text-xs text-yellow-600">
          <p>Connected to {availablePeers}/{totalPeers} peers</p>
          <p>Affected features: {degradedFeatures.join(', ')}</p>
        </div>
        <div className="mt-3">
          <button className="text-xs bg-yellow-100 hover:bg-yellow-200 px-2 py-1 rounded">
            View Network Status
          </button>
        </div>
      </div>
    </div>
  </div>
);
```

**Adaptive Behavior:**
```jsx
// Feature Degradation Patterns
const AdaptiveFeatureToggle = ({ feature, connectionState }) => {
  const isAvailable = feature.availability[connectionState];
  
  if (!isAvailable) {
    return (
      <Tooltip content={`${feature.name} requires stable connection`}>
        <button 
          className="opacity-50 cursor-not-allowed" 
          disabled
        >
          {feature.name} (Unavailable)
        </button>
      </Tooltip>
    );
  }
  
  return <FeatureButton feature={feature} />;
};
```

#### State 3: Offline Mode 🔴
**Characteristics**: No network connectivity, full local operation
**User Indicators**:
- Clear offline mode banner
- Queue count for pending operations
- Local-only feature indicators
- Offline-specific UI patterns

```jsx
const OfflineModeBanner = ({ queuedOperations, lastSync }) => (
  <div className="bg-red-50 border-l-4 border-red-400 p-4 sticky top-0 z-50">
    <div className="flex items-center justify-between">
      <div className="flex items-center">
        <WifiSlashIcon className="h-5 w-5 text-red-400 mr-3" />
        <div>
          <p className="text-sm font-medium text-red-800">
            Working Offline
          </p>
          <p className="text-xs text-red-600">
            {queuedOperations} operations queued • Last sync: {formatTime(lastSync)}
          </p>
        </div>
      </div>
      <div className="flex space-x-2">
        <button className="text-xs bg-red-100 hover:bg-red-200 px-2 py-1 rounded">
          View Queue ({queuedOperations})
        </button>
        <button className="text-xs bg-red-100 hover:bg-red-200 px-2 py-1 rounded">
          Retry Connection
        </button>
      </div>
    </div>
  </div>
);
```

**Offline Operation Patterns:**
```jsx
// Queue-Based Operation UI
const OfflineOperationButton = ({ operation, onQueue }) => {
  const [isQueued, setIsQueued] = useState(false);
  
  const handleClick = async () => {
    if (navigator.onLine) {
      await operation.execute();
    } else {
      await onQueue(operation);
      setIsQueued(true);
      
      // Show feedback
      toast.success('Operation queued for when you\'re back online');
    }
  };
  
  return (
    <button
      onClick={handleClick}
      className={`px-4 py-2 rounded ${
        isQueued 
          ? 'bg-orange-100 text-orange-800' 
          : 'bg-blue-500 text-white'
      }`}
    >
      {isQueued ? (
        <>
          <ClockIcon className="w-4 h-4 inline mr-1" />
          Queued
        </>
      ) : (
        operation.label
      )}
    </button>
  );
};
```

#### State 4: Syncing/Reconnecting 🔄
**Characteristics**: Establishing connectivity, syncing queued operations
**User Indicators**:
- Progress indicators for sync operations
- Estimated time remaining
- Operation-by-operation status
- Conflict resolution prompts

```jsx
const SyncProgressIndicator = ({ 
  totalOperations, 
  completedOperations, 
  currentOperation,
  estimatedTimeRemaining,
  conflicts = []
}) => (
  <div className="bg-blue-50 border-l-4 border-blue-400 p-4">
    <div className="flex items-center justify-between mb-2">
      <div className="flex items-center">
        <ArrowPathIcon className="h-5 w-5 text-blue-400 animate-spin mr-2" />
        <span className="text-sm font-medium text-blue-800">
          Syncing {completedOperations}/{totalOperations} operations
        </span>
      </div>
      <span className="text-xs text-blue-600">
        ~{estimatedTimeRemaining}s remaining
      </span>
    </div>
    
    {/* Progress Bar */}
    <div className="w-full bg-blue-200 rounded-full h-2 mb-3">
      <div 
        className="bg-blue-600 h-2 rounded-full transition-all duration-300"
        style={{ width: `${(completedOperations / totalOperations) * 100}%` }}
      />
    </div>
    
    {/* Current Operation */}
    {currentOperation && (
      <p className="text-xs text-blue-700 mb-2">
        Currently: {currentOperation.description}
      </p>
    )}
    
    {/* Conflicts */}
    {conflicts.length > 0 && (
      <div className="mt-3 p-2 bg-yellow-50 rounded">
        <p className="text-xs text-yellow-700 font-medium">
          {conflicts.length} conflicts need your attention
        </p>
        <button className="text-xs text-yellow-800 underline mt-1">
          Resolve conflicts
        </button>
      </div>
    )}
  </div>
);
```

#### State 5: Conflict Resolution Required ⚠️
**Characteristics**: CRDT merge conflicts requiring user decision
**User Indicators**:
- Conflict resolution modal/sidebar
- Side-by-side diff view
- Resolution options (merge strategies)
- Batch conflict resolution tools

```jsx
const ConflictResolutionUI = ({ conflicts, onResolve }) => {
  const [selectedConflict, setSelectedConflict] = useState(0);
  const conflict = conflicts[selectedConflict];
  
  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-lg shadow-xl max-w-4xl w-full mx-4 max-h-90vh overflow-hidden">
        <div className="p-4 border-b">
          <h2 className="text-lg font-semibold">
            Resolve Conflicts ({selectedConflict + 1}/{conflicts.length})
          </h2>
          <p className="text-sm text-gray-600">
            Changes were made to the same data while you were offline
          </p>
        </div>
        
        <div className="p-4 flex-1 overflow-auto">
          <ConflictDiffView 
            conflict={conflict}
            onResolve={(resolution) => {
              onResolve(conflict.id, resolution);
              if (selectedConflict < conflicts.length - 1) {
                setSelectedConflict(selectedConflict + 1);
              }
            }}
          />
        </div>
        
        <div className="p-4 border-t bg-gray-50 flex justify-between">
          <div className="flex space-x-2">
            {conflicts.length > 1 && (
              <>
                <button 
                  onClick={() => setSelectedConflict(Math.max(0, selectedConflict - 1))}
                  disabled={selectedConflict === 0}
                  className="px-3 py-1 text-sm border rounded disabled:opacity-50"
                >
                  Previous
                </button>
                <button 
                  onClick={() => setSelectedConflict(Math.min(conflicts.length - 1, selectedConflict + 1))}
                  disabled={selectedConflict === conflicts.length - 1}
                  className="px-3 py-1 text-sm border rounded disabled:opacity-50"
                >
                  Next
                </button>
              </>
            )}
          </div>
          
          <div className="space-x-2">
            <button className="px-3 py-1 text-sm border rounded">
              Resolve All Automatically
            </button>
            <button className="px-4 py-2 bg-blue-500 text-white rounded">
              Apply Resolution
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
```

---

## CRDT Conflict Resolution Workflows

### Visual Diff Interface

#### Side-by-Side Comparison
```jsx
const ConflictDiffView = ({ conflict, onResolve }) => {
  const [selectedResolution, setSelectedResolution] = useState(null);
  
  return (
    <div className="space-y-4">
      {/* Conflict Summary */}
      <div className="bg-yellow-50 p-3 rounded">
        <h3 className="font-medium text-yellow-800">
          Conflict: {conflict.resource.type} - {conflict.resource.name}
        </h3>
        <p className="text-sm text-yellow-700">
          Modified by {conflict.authors.join(' and ')} while offline
        </p>
      </div>
      
      {/* Diff View */}
      <div className="grid grid-cols-2 gap-4">
        <div className="border rounded">
          <div className="bg-red-50 px-3 py-2 border-b">
            <h4 className="font-medium text-red-800">Your Changes</h4>
            <p className="text-xs text-red-600">
              Modified {formatTime(conflict.localVersion.timestamp)}
            </p>
          </div>
          <div className="p-3">
            <DiffHighlight 
              content={conflict.localVersion.content}
              changes={conflict.localVersion.changes}
              type="local"
            />
          </div>
        </div>
        
        <div className="border rounded">
          <div className="bg-blue-50 px-3 py-2 border-b">
            <h4 className="font-medium text-blue-800">Remote Changes</h4>
            <p className="text-xs text-blue-600">
              Modified by {conflict.remoteVersion.author} {formatTime(conflict.remoteVersion.timestamp)}
            </p>
          </div>
          <div className="p-3">
            <DiffHighlight 
              content={conflict.remoteVersion.content}
              changes={conflict.remoteVersion.changes}
              type="remote"
            />
          </div>
        </div>
      </div>
      
      {/* Resolution Options */}
      <div className="space-y-2">
        <h4 className="font-medium">Choose Resolution:</h4>
        
        <div className="space-y-2">
          <label className="flex items-center space-x-2">
            <input 
              type="radio" 
              name="resolution" 
              value="local"
              onChange={() => setSelectedResolution('local')}
            />
            <span>Keep your changes</span>
          </label>
          
          <label className="flex items-center space-x-2">
            <input 
              type="radio" 
              name="resolution" 
              value="remote"
              onChange={() => setSelectedResolution('remote')}
            />
            <span>Keep remote changes</span>
          </label>
          
          <label className="flex items-center space-x-2">
            <input 
              type="radio" 
              name="resolution" 
              value="merge"
              onChange={() => setSelectedResolution('merge')}
            />
            <span>Merge both changes (automatic)</span>
          </label>
          
          <label className="flex items-center space-x-2">
            <input 
              type="radio" 
              name="resolution" 
              value="manual"
              onChange={() => setSelectedResolution('manual')}
            />
            <span>Manual merge (edit)</span>
          </label>
        </div>
      </div>
      
      {/* Manual Merge Editor */}
      {selectedResolution === 'manual' && (
        <div className="border rounded">
          <div className="bg-green-50 px-3 py-2 border-b">
            <h4 className="font-medium text-green-800">Manual Merge</h4>
          </div>
          <div className="p-3">
            <CodeEditor 
              value={conflict.mergeBase}
              onChange={(value) => {
                setSelectedResolution({ type: 'manual', content: value });
              }}
              language={conflict.resource.language}
              showDiff={true}
            />
          </div>
        </div>
      )}
    </div>
  );
};
```

### Automatic Resolution Strategies

#### Intelligent Merge Patterns
```jsx
const AutoResolutionEngine = {
  // Strategy 1: Last Writer Wins with Metadata
  lastWriterWins: (conflict) => {
    return conflict.versions.reduce((latest, version) => {
      return version.timestamp > latest.timestamp ? version : latest;
    });
  },
  
  // Strategy 2: Semantic Merge for Code
  semanticMerge: (conflict) => {
    if (conflict.resource.type === 'code') {
      return mergeCodeChanges(
        conflict.localVersion,
        conflict.remoteVersion,
        conflict.baseVersion
      );
    }
    return null; // Fallback to manual
  },
  
  // Strategy 3: Additive Merge for Lists
  additiveMerge: (conflict) => {
    if (conflict.resource.type === 'list') {
      return {
        ...conflict.baseVersion,
        items: [
          ...new Set([
            ...conflict.localVersion.items,
            ...conflict.remoteVersion.items
          ])
        ]
      };
    }
    return null;
  },
  
  // Strategy 4: Field-Level Merge for Objects
  fieldLevelMerge: (conflict) => {
    const merged = { ...conflict.baseVersion };
    
    // Apply non-conflicting local changes
    Object.keys(conflict.localVersion.changes).forEach(field => {
      if (!conflict.remoteVersion.changes[field]) {
        merged[field] = conflict.localVersion[field];
      }
    });
    
    // Apply non-conflicting remote changes  
    Object.keys(conflict.remoteVersion.changes).forEach(field => {
      if (!conflict.localVersion.changes[field]) {
        merged[field] = conflict.remoteVersion[field];
      }
    });
    
    return merged;
  }
};
```

### Batch Conflict Resolution

#### Bulk Resolution Interface
```jsx
const BatchConflictResolution = ({ conflicts, onResolveAll }) => {
  const [resolutionStrategy, setResolutionStrategy] = useState('smart');
  const [preview, setPreview] = useState(null);
  
  const generatePreview = async (strategy) => {
    const resolutions = await Promise.all(
      conflicts.map(conflict => 
        AutoResolutionEngine[strategy](conflict)
      )
    );
    setPreview(resolutions);
  };
  
  return (
    <div className="p-4 border rounded">
      <h3 className="font-medium mb-4">
        Resolve {conflicts.length} conflicts automatically
      </h3>
      
      <div className="space-y-3">
        <div>
          <label className="block text-sm font-medium mb-2">
            Resolution Strategy
          </label>
          <select 
            value={resolutionStrategy}
            onChange={(e) => {
              setResolutionStrategy(e.target.value);
              generatePreview(e.target.value);
            }}
            className="w-full border rounded px-3 py-2"
          >
            <option value="smart">Smart Merge (Recommended)</option>
            <option value="lastWriterWins">Keep Latest Changes</option>
            <option value="keepLocal">Keep All Local Changes</option>
            <option value="keepRemote">Keep All Remote Changes</option>
          </select>
        </div>
        
        {/* Preview Results */}
        {preview && (
          <div className="bg-gray-50 p-3 rounded">
            <h4 className="font-medium mb-2">Preview Results:</h4>
            <div className="space-y-1 text-sm">
              <div className="flex justify-between">
                <span>Auto-resolved:</span>
                <span className="text-green-600">
                  {preview.filter(r => r.success).length}
                </span>
              </div>
              <div className="flex justify-between">
                <span>Manual review needed:</span>
                <span className="text-yellow-600">
                  {preview.filter(r => !r.success).length}
                </span>
              </div>
            </div>
          </div>
        )}
        
        <div className="flex space-x-2">
          <button 
            onClick={() => onResolveAll(resolutionStrategy)}
            className="px-4 py-2 bg-blue-500 text-white rounded"
          >
            Apply to All
          </button>
          <button className="px-4 py-2 border rounded">
            Review Each Conflict
          </button>
        </div>
      </div>
    </div>
  );
};
```

---

## Sync Progress Indicators

### Time-Estimated Progress

#### Smart Progress Calculation
```jsx
const useSmartProgress = (operations) => {
  const [progress, setProgress] = useState({
    completed: 0,
    total: operations.length,
    estimatedTimeRemaining: 0,
    currentOperation: null
  });
  
  useEffect(() => {
    const calculateProgress = () => {
      const operationWeights = {
        'agent.create': 3,     // Heavy operation
        'agent.update': 1,     // Light operation  
        'task.create': 2,      // Medium operation
        'file.upload': 5,      // Very heavy
        'metadata.update': 0.5 // Very light
      };
      
      const totalWeight = operations.reduce((sum, op) => 
        sum + (operationWeights[op.type] || 1), 0
      );
      
      const completedWeight = operations
        .filter(op => op.status === 'completed')
        .reduce((sum, op) => sum + (operationWeights[op.type] || 1), 0);
      
      const progressPercent = (completedWeight / totalWeight) * 100;
      const avgTimePerWeight = 200; // ms per weight unit
      const remainingWeight = totalWeight - completedWeight;
      const estimatedTime = remainingWeight * avgTimePerWeight / 1000; // seconds
      
      setProgress({
        completed: operations.filter(op => op.status === 'completed').length,
        total: operations.length,
        progressPercent,
        estimatedTimeRemaining: Math.ceil(estimatedTime),
        currentOperation: operations.find(op => op.status === 'processing')
      });
    };
    
    const interval = setInterval(calculateProgress, 500);
    return () => clearInterval(interval);
  }, [operations]);
  
  return progress;
};
```

#### Enhanced Progress Display
```jsx
const SyncProgressDisplay = ({ operations }) => {
  const progress = useSmartProgress(operations);
  const [showDetails, setShowDetails] = useState(false);
  
  return (
    <div className="bg-white border rounded-lg shadow p-4">
      {/* Header */}
      <div className="flex items-center justify-between mb-3">
        <h3 className="font-medium">Syncing Changes</h3>
        <button 
          onClick={() => setShowDetails(!showDetails)}
          className="text-sm text-blue-600 hover:text-blue-800"
        >
          {showDetails ? 'Hide Details' : 'Show Details'}
        </button>
      </div>
      
      {/* Main Progress */}
      <div className="space-y-3">
        <div className="flex items-center justify-between text-sm">
          <span>{progress.completed}/{progress.total} operations</span>
          <span className="text-gray-500">
            ~{progress.estimatedTimeRemaining}s remaining
          </span>
        </div>
        
        <div className="w-full bg-gray-200 rounded-full h-2">
          <div 
            className="bg-blue-600 h-2 rounded-full transition-all duration-500 relative"
            style={{ width: `${progress.progressPercent}%` }}
          >
            {/* Animated progress indicator */}
            <div className="absolute inset-0 bg-blue-400 rounded-full animate-pulse opacity-50" />
          </div>
        </div>
        
        {/* Current Operation */}
        {progress.currentOperation && (
          <div className="flex items-center space-x-2 text-sm text-gray-600">
            <ArrowPathIcon className="w-4 h-4 animate-spin" />
            <span>
              {progress.currentOperation.description || progress.currentOperation.type}
            </span>
          </div>
        )}
      </div>
      
      {/* Detailed View */}
      {showDetails && (
        <div className="mt-4 space-y-2 max-h-40 overflow-y-auto">
          {operations.map((operation, index) => (
            <div 
              key={operation.id}
              className="flex items-center justify-between text-sm p-2 rounded bg-gray-50"
            >
              <div className="flex items-center space-x-2">
                {operation.status === 'completed' ? (
                  <CheckIcon className="w-4 h-4 text-green-500" />
                ) : operation.status === 'processing' ? (
                  <ArrowPathIcon className="w-4 h-4 text-blue-500 animate-spin" />
                ) : (
                  <ClockIcon className="w-4 h-4 text-gray-400" />
                )}
                <span>{operation.description}</span>
              </div>
              <span className="text-xs text-gray-500">
                {operation.status === 'completed' ? 'Done' : 
                 operation.status === 'processing' ? 'Syncing...' : 'Queued'}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
```

### Queue Management Interface

#### Operation Queue Visualization
```jsx
const OperationQueue = ({ queue, onReorder, onCancel, onPriorityChange }) => {
  return (
    <div className="bg-white border rounded-lg shadow">
      <div className="p-4 border-b">
        <h3 className="font-medium">Operation Queue ({queue.length})</h3>
        <p className="text-sm text-gray-600">
          Operations will sync when connection is restored
        </p>
      </div>
      
      <DragDropContext onDragEnd={onReorder}>
        <Droppable droppableId="queue">
          {(provided) => (
            <div {...provided.droppableProps} ref={provided.innerRef}>
              {queue.map((operation, index) => (
                <Draggable 
                  key={operation.id} 
                  draggableId={operation.id} 
                  index={index}
                >
                  {(provided, snapshot) => (
                    <div
                      ref={provided.innerRef}
                      {...provided.draggableProps}
                      className={`p-3 border-b flex items-center justify-between ${
                        snapshot.isDragging ? 'bg-blue-50' : ''
                      }`}
                    >
                      <div className="flex items-center space-x-3">
                        <div 
                          {...provided.dragHandleProps}
                          className="text-gray-400 hover:text-gray-600 cursor-move"
                        >
                          <Bars3Icon className="w-4 h-4" />
                        </div>
                        
                        <div>
                          <p className="font-medium text-sm">
                            {operation.description}
                          </p>
                          <p className="text-xs text-gray-500">
                            {formatTime(operation.queuedAt)}
                          </p>
                        </div>
                      </div>
                      
                      <div className="flex items-center space-x-2">
                        <select 
                          value={operation.priority}
                          onChange={(e) => onPriorityChange(operation.id, e.target.value)}
                          className="text-xs border rounded px-2 py-1"
                        >
                          <option value="low">Low</option>
                          <option value="normal">Normal</option>
                          <option value="high">High</option>
                        </select>
                        
                        <button 
                          onClick={() => onCancel(operation.id)}
                          className="text-red-500 hover:text-red-700"
                        >
                          <XMarkIcon className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  )}
                </Draggable>
              ))}
              {provided.placeholder}
            </div>
          )}
        </Droppable>
      </DragDropContext>
    </div>
  );
};
```

---

## Mobile Offline Workflows

### Touch-Optimized Interactions

#### Swipe Gestures for Quick Actions
```jsx
const SwipeableAgentCard = ({ agent, onOfflineAction }) => {
  const [swipeState, setSwipeState] = useState({ x: 0, revealed: false });
  
  const swipeActions = [
    {
      id: 'queue-task',
      label: 'Queue Task',
      icon: PlusIcon,
      color: 'bg-blue-500',
      action: () => onOfflineAction('queue-task', agent.id)
    },
    {
      id: 'priority-high',  
      label: 'High Priority',
      icon: ExclamationTriangleIcon,
      color: 'bg-yellow-500',
      action: () => onOfflineAction('priority-high', agent.id)
    },
    {
      id: 'offline-stop',
      label: 'Stop When Online',
      icon: StopIcon,
      color: 'bg-red-500', 
      action: () => onOfflineAction('stop-when-online', agent.id)
    }
  ];
  
  return (
    <div className="relative overflow-hidden bg-white rounded-lg shadow">
      {/* Swipe Actions Background */}
      <div className="absolute inset-0 flex justify-end">
        <div className="flex">
          {swipeActions.map((action) => (
            <button
              key={action.id}
              className={`w-16 h-full ${action.color} text-white flex flex-col items-center justify-center`}
              onClick={action.action}
            >
              <action.icon className="w-5 h-5" />
              <span className="text-xs mt-1">{action.label}</span>
            </button>
          ))}
        </div>
      </div>
      
      {/* Main Card Content */}
      <div 
        className="relative bg-white p-4 transform transition-transform"
        style={{ 
          transform: `translateX(${swipeState.x}px)`,
          touchAction: 'pan-x'
        }}
        {...useSwipeGesture({
          onSwipe: (deltaX) => {
            const maxSwipe = -192; // Width of 3 actions (64px each)
            const clampedX = Math.max(maxSwipe, Math.min(0, deltaX));
            setSwipeState({ x: clampedX, revealed: clampedX < -20 });
          }
        })}
      >
        <AgentCard agent={agent} />
        
        {/* Offline Indicator */}
        {!navigator.onLine && (
          <div className="absolute top-2 right-2">
            <div className="w-2 h-2 bg-orange-500 rounded-full" />
          </div>
        )}
      </div>
    </div>
  );
};
```

#### Pull-to-Refresh for Sync
```jsx
const PullToRefreshContainer = ({ children, onRefresh }) => {
  const [pullState, setPullState] = useState({
    pulling: false,
    distance: 0,
    shouldRefresh: false
  });
  
  const handlePullStart = (e) => {
    if (window.scrollY === 0) {
      setPullState(prev => ({ ...prev, pulling: true }));
    }
  };
  
  const handlePullMove = (e) => {
    if (pullState.pulling && window.scrollY === 0) {
      const touch = e.touches[0];
      const distance = Math.max(0, touch.clientY - pullState.startY);
      const shouldRefresh = distance > 80;
      
      setPullState(prev => ({
        ...prev,
        distance,
        shouldRefresh
      }));
    }
  };
  
  const handlePullEnd = () => {
    if (pullState.shouldRefresh) {
      onRefresh();
    }
    
    setPullState({
      pulling: false,
      distance: 0, 
      shouldRefresh: false
    });
  };
  
  return (
    <div 
      className="relative"
      onTouchStart={handlePullStart}
      onTouchMove={handlePullMove}
      onTouchEnd={handlePullEnd}
    >
      {/* Pull Indicator */}
      {pullState.pulling && (
        <div 
          className="absolute top-0 left-0 right-0 flex items-center justify-center transition-all"
          style={{ 
            height: `${Math.min(pullState.distance, 120)}px`,
            transform: `translateY(-${120 - Math.min(pullState.distance, 120)}px)`
          }}
        >
          <div className="flex flex-col items-center">
            <ArrowPathIcon 
              className={`w-6 h-6 text-blue-500 ${
                pullState.shouldRefresh ? 'animate-spin' : ''
              }`}
            />
            <span className="text-sm text-blue-600 mt-2">
              {pullState.shouldRefresh ? 'Release to sync' : 'Pull to sync'}
            </span>
          </div>
        </div>
      )}
      
      {/* Main Content */}
      <div 
        className="transition-transform"
        style={{ 
          transform: `translateY(${Math.min(pullState.distance * 0.5, 60)}px)`
        }}
      >
        {children}
      </div>
    </div>
  );
};
```

### Mobile-Specific Offline Features

#### Offline Agent Controls
```jsx
const MobileOfflineControls = ({ agents, queuedOperations }) => {
  return (
    <div className="fixed bottom-0 left-0 right-0 bg-white border-t shadow-lg">
      <div className="p-4">
        {/* Offline Status */}
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center space-x-2">
            <div className="w-2 h-2 bg-orange-500 rounded-full" />
            <span className="text-sm font-medium">Offline Mode</span>
          </div>
          <span className="text-xs text-gray-500">
            {queuedOperations.length} queued
          </span>
        </div>
        
        {/* Quick Actions */}
        <div className="grid grid-cols-3 gap-2">
          <button className="flex flex-col items-center p-3 bg-blue-50 rounded-lg">
            <PlusIcon className="w-5 h-5 text-blue-600 mb-1" />
            <span className="text-xs text-blue-800">Queue Task</span>
          </button>
          
          <button className="flex flex-col items-center p-3 bg-green-50 rounded-lg">
            <ClockIcon className="w-5 h-5 text-green-600 mb-1" />
            <span className="text-xs text-green-800">View Queue</span>
          </button>
          
          <button className="flex flex-col items-center p-3 bg-yellow-50 rounded-lg">
            <WifiIcon className="w-5 h-5 text-yellow-600 mb-1" />
            <span className="text-xs text-yellow-800">Try Sync</span>
          </button>
        </div>
      </div>
    </div>
  );
};
```

#### Haptic Feedback Integration
```jsx
const useHapticFeedback = () => {
  const triggerHaptic = (type = 'impact') => {
    if ('vibrate' in navigator) {
      switch (type) {
        case 'success':
          navigator.vibrate([50]);
          break;
        case 'error':
          navigator.vibrate([100, 50, 100]);
          break;
        case 'warning':
          navigator.vibrate([80, 40, 80]);
          break;
        case 'impact':
        default:
          navigator.vibrate([25]);
          break;
      }
    }
  };
  
  return { triggerHaptic };
};

// Usage in components
const OfflineActionButton = ({ action, onPress }) => {
  const { triggerHaptic } = useHapticFeedback();
  
  const handlePress = () => {
    triggerHaptic('impact');
    onPress();
    
    // Show visual feedback
    toast.success(`${action.label} queued for sync`);
  };
  
  return (
    <button 
      onClick={handlePress}
      className="active:scale-95 transition-transform"
    >
      {action.label}
    </button>
  );
};
```

---

## Implementation Guidelines

### State Management Pattern
```typescript
// Offline-first state management
interface OfflineState {
  connectionState: 'connected' | 'degraded' | 'offline' | 'syncing' | 'conflicts';
  queuedOperations: Operation[];
  conflicts: Conflict[];
  syncProgress: {
    total: number;
    completed: number;
    estimatedTimeRemaining: number;
  };
  networkHealth: {
    peers: number;
    latency: number;
    bandwidth: number;
  };
}

const useOfflineState = () => {
  const [state, setState] = useState<OfflineState>({
    connectionState: 'connected',
    queuedOperations: [],
    conflicts: [],
    syncProgress: { total: 0, completed: 0, estimatedTimeRemaining: 0 },
    networkHealth: { peers: 0, latency: 0, bandwidth: 0 }
  });
  
  // Connection monitoring
  useEffect(() => {
    const updateConnectionState = () => {
      // Complex logic to determine actual connection state
      // based on P2P mesh health, not just navigator.onLine
    };
    
    const interval = setInterval(updateConnectionState, 1000);
    return () => clearInterval(interval);
  }, []);
  
  return { state, actions: { queueOperation, resolveConflict, retrySync } };
};
```

### Performance Considerations
```jsx
// Optimized rendering for large operation queues
const VirtualizedQueue = ({ operations }) => {
  const { visibleItems, containerRef, scrollElementRef } = useVirtualizer({
    count: operations.length,
    getScrollElement: () => scrollElementRef.current,
    estimateSize: () => 60, // Estimated height per item
    overscan: 10
  });
  
  return (
    <div ref={containerRef} className="h-80 overflow-auto">
      <div 
        ref={scrollElementRef}
        style={{ height: `${operations.length * 60}px` }}
        className="relative"
      >
        {visibleItems.map((virtualItem) => (
          <div
            key={virtualItem.index}
            className="absolute top-0 left-0 w-full"
            style={{
              height: `${virtualItem.size}px`,
              transform: `translateY(${virtualItem.start}px)`
            }}
          >
            <OperationItem operation={operations[virtualItem.index]} />
          </div>
        ))}
      </div>
    </div>
  );
};
```

### Testing Strategy
```javascript
// E2E testing for offline scenarios
describe('Offline-First UX', () => {
  beforeEach(async () => {
    await page.goto('/dashboard');
    await page.evaluate(() => {
      // Simulate offline state
      Object.defineProperty(navigator, 'onLine', {
        writable: true,
        value: false
      });
    });
  });
  
  it('should show offline banner when disconnected', async () => {
    await expect(page.locator('[data-testid="offline-banner"]')).toBeVisible();
    await expect(page.locator('[data-testid="offline-banner"]'))
      .toContainText('Working Offline');
  });
  
  it('should queue operations when offline', async () => {
    await page.click('[data-testid="create-agent-button"]');
    await page.fill('[data-testid="agent-name"]', 'test-agent');
    await page.click('[data-testid="submit-button"]');
    
    // Should show queued confirmation
    await expect(page.locator('[data-testid="queue-notification"]'))
      .toContainText('Operation queued for when you\'re back online');
  });
  
  it('should resolve conflicts with user input', async () => {
    // Simulate conflict state
    await page.evaluate(() => {
      window.__prism_test__.simulateConflicts([mockConflict]);
    });
    
    await expect(page.locator('[data-testid="conflict-modal"]')).toBeVisible();
    await page.click('[data-testid="keep-local-changes"]');
    await page.click('[data-testid="apply-resolution"]');
    
    await expect(page.locator('[data-testid="conflict-modal"]')).not.toBeVisible();
  });
});
```

---

## Next Steps & Validation

### Phase 2A: Core Patterns Implementation
- Implement 5 connection state UI components
- Build CRDT conflict resolution interface
- Create sync progress with time estimation
- Add mobile gesture support

### Phase 2B: Advanced Features
- Batch conflict resolution tools
- Smart auto-resolution engine
- Mobile haptic feedback integration
- Performance optimization for large queues

### Phase 2C: Testing & Refinement
- User testing with real offline scenarios
- Performance testing with thousands of queued operations
- Mobile UX validation across devices
- Accessibility testing for all connection states

### Success Validation Metrics
- **Connection State Clarity**: 95% of users correctly identify current connection state
- **Conflict Resolution Time**: <60 seconds average time to resolve conflicts
- **Sync Completion Accuracy**: Time estimates within 20% of actual sync time
- **Mobile Interaction Success**: 98% success rate for touch gestures and swipe actions

---

*These offline-first UX patterns ensure PRISM users maintain productivity regardless of connectivity, with clear feedback and intuitive workflows for all network conditions.*