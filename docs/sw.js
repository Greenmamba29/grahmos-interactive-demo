// GrahmOS Emergency Maps - Service Worker
// Caches map tiles and assets for offline functionality

const CACHE_NAME = 'grahmos-platform-v2';
const TILE_CACHE = 'grahmos-tiles-v2';

// Core files to cache immediately
const urlsToCache = [
    '/index.html',
    '/demo-intro.html',
    '/emergency-maps-v2.html',
    '/enterprise-resilience-demo.html',
    '/demo-summary.html',
    'https://unpkg.com/leaflet@1.9.4/dist/leaflet.css',
    'https://unpkg.com/leaflet@1.9.4/dist/leaflet.js'
];

// Install event - cache core files
self.addEventListener('install', event => {
    console.log('[Service Worker] Installing...');
    event.waitUntil(
        caches.open(CACHE_NAME)
            .then(cache => {
                console.log('[Service Worker] Caching core files');
                return cache.addAll(urlsToCache);
            })
            .then(() => self.skipWaiting())
    );
});

// Activate event - clean up old caches
self.addEventListener('activate', event => {
    console.log('[Service Worker] Activating...');
    event.waitUntil(
        caches.keys().then(cacheNames => {
            return Promise.all(
                cacheNames.map(cacheName => {
                    if (cacheName !== CACHE_NAME && cacheName !== TILE_CACHE) {
                        console.log('[Service Worker] Deleting old cache:', cacheName);
                        return caches.delete(cacheName);
                    }
                })
            );
        }).then(() => self.clients.claim())
    );
});

// Fetch event - cache OpenStreetMap tiles
self.addEventListener('fetch', event => {
    const { request } = event;
    const url = new URL(request.url);

    // Cache OpenStreetMap tiles
    if (url.hostname.includes('tile.openstreetmap.org')) {
        event.respondWith(
            caches.open(TILE_CACHE).then(cache => {
                return cache.match(request).then(response => {
                    if (response) {
                        console.log('[Service Worker] Tile from cache:', url.pathname);
                        return response;
                    }

                    // Fetch from network and cache
                    return fetch(request).then(networkResponse => {
                        // Only cache successful responses
                        if (networkResponse.status === 200) {
                            cache.put(request, networkResponse.clone());
                            console.log('[Service Worker] Tile cached:', url.pathname);
                        }
                        return networkResponse;
                    }).catch(error => {
                        console.log('[Service Worker] Tile fetch failed (offline?):', error);
                        // Return empty tile or placeholder
                        return new Response('', { status: 503, statusText: 'Service Unavailable' });
                    });
                });
            })
        );
        return;
    }

    // For other requests, try cache first, then network
    event.respondWith(
        caches.match(request).then(response => {
            if (response) {
                return response;
            }
            return fetch(request).then(response => {
                // Cache successful responses
                if (response.status === 200 && request.method === 'GET') {
                    const responseToCache = response.clone();
                    caches.open(CACHE_NAME).then(cache => {
                        cache.put(request, responseToCache);
                    });
                }
                return response;
            });
        })
    );
});

// Message event - for manual cache updates
self.addEventListener('message', event => {
    if (event.data.action === 'skipWaiting') {
        self.skipWaiting();
    }
    
    if (event.data.action === 'clearCache') {
        event.waitUntil(
            caches.keys().then(cacheNames => {
                return Promise.all(
                    cacheNames.map(cacheName => caches.delete(cacheName))
                );
            }).then(() => {
                event.ports[0].postMessage({ success: true });
            })
        );
    }
});
