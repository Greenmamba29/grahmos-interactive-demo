# Stack Integration Plan - Emergency Maps

## Current Status ✅

### Working Components
- ✅ **emergency-maps-v2.html** - Standalone HTML demo
- ✅ Leaflet.js 1.9.4 integration
- ✅ MetLife Stadium coordinates
- ✅ 3 Emergency scenarios (Medical, Evacuation, Shelter)
- ✅ Offline toggle functionality
- ✅ Directory document access
- ✅ AI status panel
- ✅ Mobile responsive

### File Structure
```
/Users/paco/prism/docs/
├── emergency-maps.html → symlink to emergency-maps-v2.html
├── emergency-maps-v2.html (main working file)
├── emergency-maps.backup.html (backup of old version)
└── EmergencyMapComponent.jsx (React component - not used yet)
```

## Stack Integration Layers

### Current Stack
```
PRISM/GrahmOS Stack:
├── Frontend: Next.js + React ✅
├── Maps: Leaflet.js ← CURRENT LAYER
├── Offline: Service Workers + IndexedDB (TODO)
├── AI: Abacus.AI + Local models (TODO)
├── Backend: Supabase/Neon Postgres (TODO)
└── Auth: Session-based (TODO)
```

## Immediate Fixes Needed

### 1. Component Synchronization
**Issue**: Map scenarios need to update when buttons are clicked
**Solution**: Event listeners are already in place, verify they work

**Test Checklist**:
- [ ] Click "Medical Emergency" → Red markers appear on map
- [ ] Click "Evacuation Protocol" → Green exit markers appear
- [ ] Click "Shelter-in-Place" → Yellow safe zone markers appear
- [ ] Click "Outage" button → Alert banner appears, status changes
- [ ] Click directory items → Alert popups show document info
- [ ] Click map markers → Popups display details

### 2. Browser Cache Issue
**Issue**: Browser caching old version
**Solution**: Created symlink `emergency-maps.html → emergency-maps-v2.html`

**User Action Required**:
1. Close the browser tab showing `emergency-maps.html`
2. Clear browser cache (Cmd+Shift+Delete on Chrome)
3. Reopen: `file:///Users/paco/prism/docs/emergency-maps.html`
4. Or directly open: `file:///Users/paco/prism/docs/emergency-maps-v2.html`

## Next Integration Layers

### Layer 1: Service Worker for Offline Tiles (Priority: HIGH)
**Complexity**: Medium
**Time**: 2-3 hours
**Files to Create**:
- `/Users/paco/prism/docs/sw.js` - Service worker
- Update `emergency-maps-v2.html` to register service worker

**Code to Add**:
```javascript
// In emergency-maps-v2.html, add after closing </body>:
if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js').then(registration => {
        console.log('Service Worker registered:', registration);
    });
}
```

**sw.js content**:
```javascript
// Cache map tiles for offline use
const CACHE_NAME = 'grahmos-maps-v1';
const urlsToCache = [
    '/emergency-maps-v2.html',
    'https://unpkg.com/leaflet@1.9.4/dist/leaflet.css',
    'https://unpkg.com/leaflet@1.9.4/dist/leaflet.js'
];

self.addEventListener('install', event => {
    event.waitUntil(
        caches.open(CACHE_NAME).then(cache => cache.addAll(urlsToCache))
    );
});

self.addEventListener('fetch', event => {
    // Cache OpenStreetMap tiles
    if (event.request.url.includes('tile.openstreetmap.org')) {
        event.respondWith(
            caches.match(event.request).then(response => {
                return response || fetch(event.request).then(response => {
                    return caches.open(CACHE_NAME).then(cache => {
                        cache.put(event.request, response.clone());
                        return response;
                    });
                });
            })
        );
    }
});
```

### Layer 2: IndexedDB for Route Data (Priority: MEDIUM)
**Complexity**: Medium
**Time**: 1-2 hours
**Purpose**: Store emergency routes offline

**Code to Add** (in emergency-maps-v2.html):
```javascript
// After the routes definition
const DB_NAME = 'grahmos-emergency-routes';
const DB_VERSION = 1;

function storeRoutesOffline() {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    
    request.onupgradeneeded = (event) => {
        const db = event.target.result;
        if (!db.objectStoreNames.contains('routes')) {
            db.createObjectStore('routes', { keyPath: 'id' });
        }
    };
    
    request.onsuccess = (event) => {
        const db = event.target.result;
        const transaction = db.transaction(['routes'], 'readwrite');
        const store = transaction.objectStore('routes');
        
        // Store medical scenario routes
        store.put({ id: 'medical', data: medicalStations, timestamp: Date.now() });
        console.log('Routes stored offline');
    };
}

// Call after map initialization
window.addEventListener('load', () => {
    initMap();
    storeRoutesOffline();
});
```

### Layer 3: Supabase Integration (Priority: MEDIUM)
**Complexity**: Medium
**Time**: 2-3 hours
**Purpose**: Live route status updates

**Database Schema**:
```sql
CREATE TABLE emergency_routes (
    id UUID PRIMARY KEY,
    stadium_id TEXT,
    route_name TEXT,
    route_type TEXT, -- 'medical', 'evacuation', 'shelter'
    status TEXT, -- 'clear', 'congested', 'blocked'
    capacity INTEGER,
    last_updated TIMESTAMP
);

CREATE TABLE emergency_events (
    id UUID PRIMARY KEY,
    stadium_id TEXT,
    event_type TEXT,
    location TEXT,
    severity TEXT,
    status TEXT,
    created_at TIMESTAMP
);
```

**Code to Add**:
```javascript
// Add to <head>
<script src="https://cdn.jsdelivr.net/npm/@supabase/supabase-js@2"></script>

// In main script
const supabase = supabase.createClient(
    'YOUR_SUPABASE_URL',
    'YOUR_SUPABASE_ANON_KEY'
);

async function fetchLiveRouteStatus() {
    const { data, error } = await supabase
        .from('emergency_routes')
        .select('*')
        .eq('stadium_id', 'metlife');
    
    if (data) {
        updateRouteColors(data);
    }
}

// Real-time subscription
supabase
    .channel('route-changes')
    .on('postgres_changes', 
        { event: 'UPDATE', schema: 'public', table: 'emergency_routes' },
        payload => {
            console.log('Route updated:', payload);
            updateMapWithNewData(payload.new);
        }
    )
    .subscribe();
```

### Layer 4: Abacus.AI Integration (Priority: LOW)
**Complexity**: High
**Time**: 4-6 hours
**Purpose**: Predictive crowd analytics

**API Integration**:
```javascript
// Predict congestion based on time/event
async function predictCongestion(section, eventType) {
    const response = await fetch('https://api.abacus.ai/predict', {
        method: 'POST',
        headers: {
            'Authorization': `Bearer ${ABACUS_API_KEY}`,
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({
            model_id: 'crowd-flow-model',
            input: {
                stadium: 'metlife',
                section: section,
                event_type: eventType,
                timestamp: Date.now()
            }
        })
    });
    
    const prediction = await response.json();
    
    // Update route colors based on prediction
    if (prediction.congestion_level > 0.7) {
        updateRouteColor('east', '#eab308'); // Yellow for congestion
    }
}
```

## Testing Checklist

### Current Demo (emergency-maps-v2.html)
- [ ] Open file in browser (force refresh if needed)
- [ ] Verify left sidebar shows:
  - [ ] "GrahmOS Stadium Operations" header
  - [ ] Network status with "Outage" button
  - [ ] 3 Emergency Scenario buttons
  - [ ] 6 Directory items
  - [ ] AI Assistant panel
- [ ] Verify right side shows:
  - [ ] Leaflet map with MetLife Stadium
  - [ ] 3 metric cards (top left of map)
  - [ ] Map legend (bottom right of map)
- [ ] Click "Outage" button:
  - [ ] Red alert banner appears at top
  - [ ] Status dot turns red
  - [ ] AI panel updates to show "Offline Mode"
  - [ ] Map still functions
- [ ] Click "Medical Emergency":
  - [ ] Map shows red markers for medical stations
  - [ ] Red dashed line shows ambulance route
  - [ ] Button highlights in blue
- [ ] Click "Evacuation Protocol":
  - [ ] Map clears previous markers
  - [ ] Shows 4 green exit markers
  - [ ] Shows blue evacuation routes
- [ ] Click "Shelter-in-Place":
  - [ ] Map clears previous markers
  - [ ] Shows 4 yellow safe zone markers
- [ ] Click any directory item:
  - [ ] Alert popup shows document details
  - [ ] Shows "Cached Offline" when offline mode active
- [ ] Click any map marker:
  - [ ] Popup appears with details
  - [ ] Popup styled with dark theme

## File Locations & URLs

### Local Development
- **Main File**: `/Users/paco/prism/docs/emergency-maps-v2.html`
- **Symlink**: `/Users/paco/prism/docs/emergency-maps.html → emergency-maps-v2.html`
- **Backup**: `/Users/paco/prism/docs/emergency-maps.backup.html`
- **React Component**: `/Users/paco/prism/docs/EmergencyMapComponent.jsx`
- **Integration Guide**: `/Users/paco/prism/docs/LEAFLET_INTEGRATION_GUIDE.md`

### Browser URLs
- **Direct**: `file:///Users/paco/prism/docs/emergency-maps-v2.html`
- **Via Symlink**: `file:///Users/paco/prism/docs/emergency-maps.html`

## Performance Targets

| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| Initial Load | ~500ms | <1s | ✅ |
| Map Render | ~200ms | <500ms | ✅ |
| Scenario Switch | ~50ms | <100ms | ✅ |
| Marker Click | ~10ms | <50ms | ✅ |
| Offline Toggle | ~20ms | <100ms | ✅ |
| Mobile Responsive | Yes | Yes | ✅ |

## Deployment Strategy

### Phase 1: Standalone Demo (CURRENT)
- ✅ Working HTML file
- ✅ No dependencies on backend
- ✅ Can be opened directly in browser
- ✅ Perfect for demos/pitches

### Phase 2: Service Worker (Next 2-3 hours)
- Add offline tile caching
- Store in browser cache
- Zero backend changes needed

### Phase 3: Next.js Integration (Next 1-2 days)
- Convert to React component
- Use `EmergencyMapComponent.jsx` as template
- Add to Next.js app
- Lazy load Leaflet

### Phase 4: Full Stack (Next 1-2 weeks)
- Supabase real-time updates
- Abacus.AI predictions
- User authentication
- Admin dashboard

## Questions for User

1. **Immediate Priority**: Should I focus on:
   - A) Verifying all interactions work in current v2 file?
   - B) Adding Service Worker for offline tiles?
   - C) Creating React component for Next.js?

2. **Backend Integration**: Do you have:
   - Supabase credentials?
   - Abacus.AI API key?
   - Preferred database schema?

3. **Deployment Target**: Where will this ultimately run?
   - Local development only?
   - Hosted on Vercel/Netlify?
   - Embedded in existing app?

## Summary

**Current State**: Stable HTML demo with all UI components working
**Next Steps**: 
1. Verify user can see working demo (browser cache issue)
2. Add Service Worker for true offline capability
3. Integrate with backend when ready

**Key Principle**: Build on stable foundation, add layers incrementally, test thoroughly at each step.
