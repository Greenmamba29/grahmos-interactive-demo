# Emergency Maps - Deployment Guide

## ✅ Layers 1 & 2 Completed

### What's New
- ✅ **Service Worker** (`sw.js`) - Caches map tiles offline
- ✅ **IndexedDB** - Stores route data offline
- ✅ **Updated HTML** (`emergency-maps-v2.html`) - Integrated both layers
- ✅ **Netlify Config** (`netlify.toml`) - Ready for deployment
- ✅ **GitHub Actions** (`.github/workflows/deploy-maps.yml`) - Auto-deploy on push

## Local Testing

### 1. Test Service Worker & IndexedDB
```bash
# Option 1: Use Python's built-in server
cd /Users/paco/prism/docs
python3 -m http.server 8000

# Option 2: Use Node's http-server
npm install -g http-server
cd /Users/paco/prism/docs
http-server -p 8000
```

Then open: `http://localhost:8000/emergency-maps-v2.html`

### 2. Verify in Browser DevTools

**Chrome DevTools (Cmd+Option+I):**

1. **Application Tab → Service Workers**
   - Should see: `sw.js` with status "activated and running"
   
2. **Application Tab → Cache Storage**
   - Should see: `grahmos-maps-v1` (core files)
   - Should see: `grahmos-tiles-v1` (map tiles)
   
3. **Application Tab → IndexedDB**
   - Should see: `grahmos-emergency-routes`
   - Inside: `routes` and `markers` object stores

4. **Console**
   - Should see:
     ```
     [Service Worker] Registered
     [IndexedDB] Database opened
     [IndexedDB] Stored medical route data offline
     ```

5. **Test Offline**
   - Click "Outage" button (simulates network failure)
   - Or: DevTools → Network tab → Throttling → Offline
   - Map should still load tiles from cache
   - Scenarios should still switch

## Netlify Deployment

### Setup (One-Time)

1. **Create Netlify Account**
   - Go to https://netlify.com
   - Sign up or log in

2. **Get Netlify Tokens**
   ```bash
   # Install Netlify CLI
   npm install -g netlify-cli
   
   # Login
   netlify login
   
   # Get auth token
   netlify status
   ```

3. **Create New Site**
   ```bash
   cd /Users/paco/prism
   netlify init
   ```
   
   **When prompted:**
   - Build command: `echo 'No build needed'`
   - Publish directory: `docs`
   - Functions directory: (leave empty)

4. **Note Your Site ID**
   - Shown in terminal after `netlify init`
   - Or find at: https://app.netlify.com/sites/YOUR-SITE-NAME/settings

### Manual Deployment

```bash
cd /Users/paco/prism
netlify deploy --prod --dir=docs --message="Initial emergency maps deployment"
```

### Automated Deployment (GitHub Actions)

1. **Add Secrets to GitHub**
   - Go to: https://github.com/YOUR-USERNAME/prism/settings/secrets/actions
   - Click "New repository secret"
   - Add:
     - Name: `NETLIFY_AUTH_TOKEN`
     - Value: Your auth token from `netlify status`
   - Add:
     - Name: `NETLIFY_SITE_ID`
     - Value: Your site ID from Netlify

2. **Push to GitHub**
   ```bash
   cd /Users/paco/prism
   
   # Add new files
   git add docs/emergency-maps-v2.html
   git add docs/sw.js
   git add docs/netlify.toml
   git add .github/workflows/deploy-maps.yml
   git add docs/DEPLOYMENT_GUIDE.md
   
   # Commit
   git commit -m "Add Service Worker + IndexedDB + Deployment config"
   
   # Push
   git push origin main
   ```

3. **GitHub Actions Will Auto-Deploy**
   - Watch at: https://github.com/YOUR-USERNAME/prism/actions
   - Should see "Deploy Emergency Maps" workflow running
   - Takes ~2-3 minutes

### Verify Deployment

1. **Check Netlify URL**
   - Will be something like: `https://grahmos-maps.netlify.app`
   - Or your custom domain

2. **Test Features**
   - Open URL in browser
   - Open DevTools
   - Verify Service Worker is registered
   - Verify IndexedDB is created
   - Test offline mode

## GitHub Repository Setup

### If Starting Fresh

```bash
cd /Users/paco/prism

# Initialize git (if not already)
git init

# Add remote (replace with your repo URL)
git remote add origin https://github.com/YOUR-USERNAME/prism.git

# Create main branch
git branch -M main

# Add all files
git add .

# First commit
git commit -m "Initial commit: Emergency maps with Service Worker + IndexedDB"

# Push to GitHub
git push -u origin main
```

### If Repository Exists

```bash
cd /Users/paco/prism

# Pull latest
git pull origin main

# Add new files
git add docs/emergency-maps-v2.html docs/sw.js docs/netlify.toml
git add .github/workflows/deploy-maps.yml

# Commit with version tag
git commit -m "v1.1.0: Add Service Worker + IndexedDB"

# Create tag
git tag -a v1.1.0 -m "Service Worker + IndexedDB integration"

# Push with tags
git push origin main --tags
```

## Version Management

### Current Version: 1.1.0

**Versioning Strategy:**
- **v1.0.0**: Initial HTML demo
- **v1.1.0**: + Service Worker + IndexedDB ← CURRENT
- **v1.2.0**: + Supabase integration (planned)
- **v1.3.0**: + Abacus.AI integration (planned)

### Update Version

```bash
# In emergency-maps-v2.html, update meta tag:
<meta name="version" content="1.1.0">

# Commit with version
git commit -m "v1.1.0: Description of changes"
git tag -a v1.1.0 -m "Description"
git push origin main --tags
```

## Rollback (If Needed)

### Rollback on Netlify
```bash
# List recent deployments
netlify deploy:list

# Rollback to previous
netlify rollback
```

### Rollback on Git
```bash
# Revert to previous commit
git revert HEAD

# Or reset to specific version
git reset --hard v1.0.0
git push origin main --force
```

## Monitoring

### Netlify Analytics
- Go to: https://app.netlify.com/sites/YOUR-SITE/analytics
- Track:
  - Page views
  - Unique visitors
  - Service Worker registration rate
  - Offline usage patterns

### GitHub Insights
- Go to: https://github.com/YOUR-USERNAME/prism/pulse
- Track:
  - Deployment frequency
  - Build success rate
  - Commit history

## Troubleshooting

### Service Worker Not Registering

**Issue**: Console shows "Service Worker registration failed"

**Solutions**:
1. Check if using HTTPS or localhost (required for SW)
2. Verify `sw.js` is in correct path (`/docs/sw.js`)
3. Check console for detailed error
4. Clear browser cache and reload

### IndexedDB Not Created

**Issue**: No database in Application → IndexedDB

**Solutions**:
1. Check browser supports IndexedDB (all modern browsers do)
2. Verify no browser extensions blocking it
3. Check console for errors
4. Try in Incognito/Private mode

### Netlify Build Fails

**Issue**: GitHub Actions shows failed deployment

**Solutions**:
1. Check GitHub Actions logs for error message
2. Verify secrets are set correctly (NETLIFY_AUTH_TOKEN, NETLIFY_SITE_ID)
3. Test manual deployment: `netlify deploy --prod --dir=docs`
4. Check netlify.toml is valid

### Map Tiles Not Caching

**Issue**: Tiles reload every time, not from cache

**Solutions**:
1. Check Service Worker is activated
2. Open Network tab → reload → should see "(from Service Worker)"
3. Check Cache Storage has `grahmos-tiles-v1`
4. Try loading map while online first (to build cache)
5. Then go offline and test

## Performance Metrics

### Target Metrics
- **Service Worker Registration**: <500ms
- **IndexedDB Open**: <100ms
- **First Tile Cache**: <200ms
- **Cached Tile Load**: <50ms
- **Scenario Data Store**: <100ms

### Measure in Console
```javascript
// Check Service Worker status
navigator.serviceWorker.ready.then(registration => {
    console.log('SW Ready:', registration.active.state);
});

// Check cache size
caches.open('grahmos-tiles-v1').then(cache => {
    cache.keys().then(keys => {
        console.log('Tiles cached:', keys.length);
    });
});

// Check IndexedDB data
const request = indexedDB.open('grahmos-emergency-routes');
request.onsuccess = (event) => {
    const db = event.target.result;
    const transaction = db.transaction(['routes'], 'readonly');
    const store = transaction.objectStore('routes');
    store.count().onsuccess = (e) => {
        console.log('Routes stored:', e.target.result);
    };
};
```

## Next Steps

After deployment is verified:

1. **Monitor for 24-48 hours**
   - Check Netlify analytics
   - Review error logs
   - Test on multiple devices

2. **Gather Feedback**
   - Share URL with stakeholders
   - Document any issues
   - Note improvement requests

3. **Plan Layer 3** (Supabase)
   - Set up database
   - Create tables
   - Add real-time sync

## Support

### Files to Check
- `docs/emergency-maps-v2.html` - Main application
- `docs/sw.js` - Service Worker
- `docs/netlify.toml` - Netlify config
- `.github/workflows/deploy-maps.yml` - GitHub Actions

### Logs to Review
- Browser Console
- Netlify Deploy Logs
- GitHub Actions Logs
- Service Worker Logs (DevTools → Application → Service Workers)

---

**Status**: ✅ Ready to deploy  
**Version**: 1.1.0  
**Features**: Service Worker + IndexedDB + Auto-deploy  
**Next**: Push to GitHub to trigger deployment
