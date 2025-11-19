# GrahmOS Demo - Deployment Guide

## ✅ Deployment Status

**Branch**: `deploy-docs` ✅ Pushed to GitHub  
**Repository**: https://github.com/Greenmamba29/grahmos-interactive-demo  
**Netlify Site ID**: `45bdb873-0091-471d-ba50-e48ee9dbdcfa`

## 🚀 Next Steps to Complete Deployment

### Option 1: Configure Netlify Dashboard (Recommended)

1. **Go to Netlify Dashboard**:
   - Visit: https://app.netlify.com/sites/45bdb873-0091-471d-ba50-e48ee9dbdcfa
   - Or search for your site in https://app.netlify.com

2. **Update Build Settings**:
   - Go to: **Site settings** → **Build & deploy** → **Build settings**
   - Set **Production branch**: `deploy-docs`
   - Build command: `echo 'Static HTML documentation ready for deployment'`
   - Publish directory: `docs`
   - Click **Save**

3. **Trigger Deploy**:
   - Go to: **Deploys** tab
   - Click **Trigger deploy** → **Deploy site**
   - Or: **Deploys** → **Deploy settings** → Enable automatic deploys from `deploy-docs` branch

4. **Set Custom Domain** (Optional):
   - Go to: **Domain settings**
   - Add your custom domain (e.g., `demo.grahmos.com`)
   - Configure DNS as instructed

### Option 2: Manual Deploy via Netlify CLI

```bash
# Install Netlify CLI with npm (not bun)
npm install -g netlify-cli

# Login to Netlify
netlify login

# Link to existing site
netlify link --id 45bdb873-0091-471d-ba50-e48ee9dbdcfa

# Deploy from deploy-docs branch
git checkout deploy-docs
netlify deploy --prod --dir=docs
```

### Option 3: Automatic Deployment from GitHub

Once you update the production branch in Netlify to `deploy-docs`, every push to this branch will automatically trigger a deployment.

## 📦 What's Included

The `deploy-docs` branch contains:
- ✅ All HTML demo pages (`docs/*.html`)
- ✅ Netlify configuration (`netlify.toml`)
- ✅ Clean README
- ✅ Minimal .gitignore

## 🎯 Demo Pages Available

1. **Home** - `index.html` - Main landing page
2. **Demo Intro** - `demo-intro.html` - Interactive capabilities showcase
3. **Demo Summary** - `demo-summary.html` - Key features overview
4. **Emergency Maps v2** - `emergency-maps-v2.html` - Interactive stadium evacuation
5. **Enterprise Resilience** - `enterprise-resilience-demo.html` - Business continuity demos

## 🔧 Configuration

### Netlify Configuration (`netlify.toml`)
```toml
[build]
  publish = "docs"
  command = "echo 'Static HTML documentation ready for deployment'"

[[redirects]]
  from = "/*"
  to = "/index.html"
  status = 200

[[headers]]
  for = "/*"
  [headers.values]
    X-Frame-Options = "SAMEORIGIN"
    Content-Security-Policy = "frame-ancestors 'self' https://*.netlify.app https://*.grahmos.com"
```

### Security Headers
- CORS enabled for iframe embedding
- XSS Protection enabled
- Content Security Policy configured

## 📊 Expected Deployment Time

- **Build time**: < 10 seconds (static files only)
- **Deploy time**: < 30 seconds
- **Total**: ~1 minute from push to live

## ✅ Verification

After deployment, verify:
1. Visit your Netlify URL
2. Check all demo pages load correctly
3. Test interactive maps functionality
4. Verify responsive design on mobile

## 🐛 Troubleshooting

### Issue: Old content showing
**Solution**: Clear Netlify cache and redeploy
```bash
netlify deploy --prod --dir=docs --clear-cache
```

### Issue: 404 errors
**Solution**: Check that `netlify.toml` redirects are configured

### Issue: Maps not loading
**Solution**: Verify Leaflet.js CDN is accessible

## 📧 Support

- **Repository**: https://github.com/Greenmamba29/grahmos-interactive-demo
- **Branch**: deploy-docs
- **Site ID**: 45bdb873-0091-471d-ba50-e48ee9dbdcfa

---

**Ready to deploy!** 🚀 Follow Option 1 above to complete the deployment.
