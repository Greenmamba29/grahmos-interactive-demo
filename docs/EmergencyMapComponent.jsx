/**
 * GrahmOS Emergency Maps - React/Next.js Integration Component
 * 
 * STACK INTEGRATION:
 * ├── Frontend: Next.js + React
 * ├── Offline: Service Workers + IndexedDB
 * ├── AI Layer: Abacus.AI + Local models
 * ├── Backend: Supabase/Neon Postgres
 * ├── Auth: Session-based
 * └── Maps: Leaflet.js (this component)
 */

import React, { useEffect, useRef, useState } from 'react';
import L from 'leaflet';
import 'leaflet/dist/leaflet.css';

const EmergencyMapComponent = ({ stadium = 'metlife', offline = true }) => {
  const mapRef = useRef(null);
  const mapInstanceRef = useRef(null);
  const [isOffline, setIsOffline] = useState(offline);
  const [selectedRoute, setSelectedRoute] = useState(null);

  // Stadium coordinates
  const stadiumCoords = {
    metlife: [40.813, -74.074],
    // Add more stadiums as needed
  };

  // Emergency routes configuration
  const routes = {
    north: {
      coords: [[40.8142, -74.0732], [40.8146, -74.0740], [40.8151, -74.0748]],
      color: '#22c55e',
      label: 'Route A - North Gates',
      capacity: 10000
    },
    south: {
      coords: [[40.8118, -74.0748], [40.8123, -74.0740], [40.8128, -74.0732]],
      color: '#22c55e',
      label: 'Route B - South Gates',
      capacity: 10000
    },
    east: {
      coords: [[40.8130, -74.0725], [40.8133, -74.0732], [40.8136, -74.0739]],
      color: '#eab308',
      label: 'Route C - East Service (CONGESTED)',
      capacity: 5000
    },
    west: {
      coords: [[40.8130, -74.0755], [40.8127, -74.0762], [40.8124, -74.0769]],
      color: '#22c55e',
      label: 'Route D - West Service',
      capacity: 5000
    }
  };

  // Emergency markers
  const markers = [
    { coords: [40.8137, -74.0742], title: 'North Gate Exit', details: 'Primary evacuation point - Capacity 10,000', icon: '🚪' },
    { coords: [40.8124, -74.0742], title: 'South Gate Exit', details: 'Primary evacuation point - Capacity 10,000', icon: '🚪' },
    { coords: [40.8131, -74.0730], title: 'East Service Exit', details: 'Secondary route - Currently congested', icon: '⚠️' },
    { coords: [40.8131, -74.0754], title: 'West Service Exit', details: 'Emergency access route - Capacity 5,000', icon: '🚪' },
    { coords: [40.8130, -74.0740], title: 'Central Command', details: 'Emergency coordination center', icon: '🏢' },
    { coords: [40.8135, -74.0736], title: 'Medical Station North', details: 'First aid and triage center', icon: '🏥' },
    { coords: [40.8126, -74.0746], title: 'Medical Station South', details: 'First aid and triage center', icon: '🏥' }
  ];

  useEffect(() => {
    if (!mapRef.current || mapInstanceRef.current) return;

    // Initialize map
    const map = L.map(mapRef.current).setView(stadiumCoords[stadium], 15);

    // Add OpenStreetMap tile layer (cacheable for offline use)
    L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
      attribution: '© OpenStreetMap contributors',
      maxZoom: 19,
    }).addTo(map);

    // Draw routes
    Object.entries(routes).forEach(([routeId, route]) => {
      const polyline = L.polyline(route.coords, {
        color: route.color,
        weight: 6,
        opacity: 0.7
      }).addTo(map);
      
      polyline.bindPopup(`<strong>${route.label}</strong><br/>Capacity: ${route.capacity}`);
    });

    // Add markers
    markers.forEach(marker => {
      const customIcon = L.divIcon({
        className: 'custom-marker',
        html: `<div style="font-size: 24px;">${marker.icon}</div>`,
        iconSize: [30, 30]
      });

      L.marker(marker.coords, { icon: customIcon })
        .addTo(map)
        .bindPopup(`<strong>${marker.title}</strong><br/>${marker.details}`);
    });

    // Geolocation
    if (navigator.geolocation) {
      navigator.geolocation.getCurrentPosition(
        (pos) => {
          const { latitude, longitude } = pos.coords;
          L.circleMarker([latitude, longitude], {
            radius: 8,
            color: '#38bdf8',
            fillColor: '#38bdf8',
            fillOpacity: 0.5
          })
            .addTo(map)
            .bindPopup('<strong>Your Location</strong><br/>Live via Geolocation API');
        },
        () => console.log('Geolocation denied')
      );
    }

    mapInstanceRef.current = map;

    // Cleanup
    return () => {
      if (mapInstanceRef.current) {
        mapInstanceRef.current.remove();
        mapInstanceRef.current = null;
      }
    };
  }, [stadium]);

  const toggleOffline = () => {
    setIsOffline(!isOffline);
    // Trigger offline mode simulation or real network toggle
    console.log(`Offline mode: ${!isOffline ? 'ON' : 'OFF'}`);
  };

  return (
    <div className="emergency-map-container">
      <div className="controls-bar">
        <div className={`offline-status ${isOffline ? 'active' : ''}`}>
          <div className="status-dot"></div>
          <span>Offline Mode: {isOffline ? 'Active' : 'Inactive'}</span>
        </div>
        <button onClick={toggleOffline} className="toggle-btn">
          Toggle Network Mode
        </button>
      </div>
      
      <div ref={mapRef} style={{ height: '600px', width: '100%' }} />

      <div className="route-sidebar">
        <h3>Emergency Routes</h3>
        {Object.entries(routes).map(([id, route]) => (
          <div 
            key={id}
            className={`route-card ${selectedRoute === id ? 'active' : ''}`}
            onClick={() => setSelectedRoute(id)}
          >
            <h4>{route.label}</h4>
            <p>Capacity: {route.capacity}</p>
          </div>
        ))}
      </div>

      <style jsx>{`
        .emergency-map-container {
          position: relative;
          width: 100%;
        }
        .controls-bar {
          display: flex;
          justify-content: space-between;
          padding: 16px;
          background: #1e293b;
          border-bottom: 1px solid #334155;
        }
        .offline-status {
          display: flex;
          align-items: center;
          gap: 10px;
          color: #f1f5f9;
        }
        .offline-status.active .status-dot {
          background: #22c55e;
          animation: pulse 2s infinite;
        }
        .status-dot {
          width: 10px;
          height: 10px;
          border-radius: 50%;
          background: #ef4444;
        }
        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.5; }
        }
        .toggle-btn {
          padding: 10px 24px;
          background: #38bdf8;
          color: #0f172a;
          border: none;
          border-radius: 8px;
          cursor: pointer;
          font-weight: 600;
        }
        .toggle-btn:hover {
          background: #0ea5e9;
        }
        .route-sidebar {
          position: absolute;
          right: 0;
          top: 70px;
          width: 300px;
          background: #1e293b;
          padding: 20px;
          max-height: 500px;
          overflow-y: auto;
        }
        .route-sidebar h3 {
          color: #38bdf8;
          margin-bottom: 16px;
        }
        .route-card {
          background: rgba(15, 23, 42, 0.6);
          border: 1px solid #334155;
          border-radius: 8px;
          padding: 12px;
          margin-bottom: 12px;
          cursor: pointer;
          transition: all 0.3s ease;
        }
        .route-card:hover {
          border-color: #38bdf8;
        }
        .route-card.active {
          border-color: #22c55e;
          background: rgba(34, 197, 94, 0.1);
        }
        .route-card h4 {
          color: #f1f5f9;
          font-size: 0.95rem;
          margin-bottom: 6px;
        }
        .route-card p {
          color: #94a3b8;
          font-size: 0.85rem;
        }
        @media (max-width: 768px) {
          .route-sidebar {
            position: relative;
            width: 100%;
            max-height: none;
          }
        }
      `}</style>
    </div>
  );
};

export default EmergencyMapComponent;

/**
 * USAGE IN NEXT.JS:
 * 
 * import EmergencyMapComponent from '@/components/EmergencyMapComponent';
 * 
 * function EmergencyDashboard() {
 *   return (
 *     <div>
 *       <h1>MetLife Stadium Emergency Response</h1>
 *       <EmergencyMapComponent stadium="metlife" offline={true} />
 *     </div>
 *   );
 * }
 * 
 * OFFLINE INTEGRATION:
 * - Add Service Worker to cache map tiles
 * - Store route data in IndexedDB
 * - Use Abacus.AI for edge AI processing
 * - Sync with Supabase when online
 * 
 * COMPLEXITY: Low-Medium
 * INTEGRATION TIME: 2-4 hours
 * TEAM REQUIRED: 1 frontend dev
 */
