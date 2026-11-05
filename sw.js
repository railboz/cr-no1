/* ============================================================
   Créno — service worker (PWA installable, hors-ligne)
   Stratégie :
   - pré-cache du shell de l'app à l'installation ;
   - navigations : réseau d'abord (mises à jour immédiates), cache en secours ;
   - autres ressources same-origin : cache d'abord + mise à jour en arrière-plan
     (les modifications d'app.js / styles.css sont donc prises au lancement suivant) ;
   - seules les réponses valides (2xx) sont mises en cache ;
   - requêtes externes (api.skello.io, relais…) : jamais interceptées.
   Pour publier une mise à jour de l'app : incrémentez CACHE.
   ============================================================ */
'use strict';

const CACHE = 'creno-v9';
const ASSETS = [
  './',
  './index.html',
  './styles.css',
  './app.js',
  './manifest.webmanifest',
  './icons/icon-192.png',
  './icons/icon-512.png',
  './icons/icon-maskable-192.png',
  './icons/icon-maskable-512.png',
];

self.addEventListener('install', e => {
  e.waitUntil(
    caches.open(CACHE)
      .then(c => c.addAll(ASSETS))
      .then(() => self.skipWaiting())
  );
});

self.addEventListener('activate', e => {
  e.waitUntil(
    caches.keys()
      .then(keys => Promise.all(keys.filter(k => k !== CACHE).map(k => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', e => {
  const req = e.request;
  if (req.method !== 'GET') return;
  const url = new URL(req.url);
  if (url.origin !== location.origin) return; /* Skello / relais : direct */

  /* Navigation : réseau d'abord, page en cache si hors-ligne */
  if (req.mode === 'navigate') {
    e.respondWith(
      fetch(req)
        .then(res => {
          if (res.ok) {
            const cp = res.clone();
            caches.open(CACHE).then(c => c.put('./index.html', cp));
          }
          return res;
        })
        .catch(() => caches.match('./index.html'))
    );
    return;
  }

  /* Ressources locales : cache d'abord */
  e.respondWith(
    caches.match(req).then(hit => {
      const net = fetch(req).then(res => {
        if (res.ok) {
          const cp = res.clone();
          caches.open(CACHE).then(c => c.put(req, cp));
        }
        return res;
      });
      if (hit) { net.catch(() => {}); return hit; } /* revalidation silencieuse */
      return net;
    })
  );
});
