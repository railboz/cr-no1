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

const CACHE = 'creno-v14';
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

  /* Navigation : réseau d'abord (délai court : pas de radio bloquée), page en cache si hors-ligne */
  if (req.mode === 'navigate') {
    e.respondWith((async () => {
      try {
        const ctrl = new AbortController();
        const timer = setTimeout(() => ctrl.abort(), 4000);
        const res = await fetch(req, { signal: ctrl.signal });
        clearTimeout(timer);
        if (res.ok) {
          const cp = res.clone();
          caches.open(CACHE).then(c => c.put('./index.html', cp));
        }
        return res;
      } catch (e) {
        return (await caches.match('./index.html')) || Response.error();
      }
    })());
    return;
  }

  /* Ressources locales : cache d'abord, revalidation silencieuse si en ligne */
  e.respondWith(
    caches.match(req).then(hit => {
      if (hit) {
        if (navigator.onLine === false) return hit; /* hors-ligne : ne pas réveiller la radio */
        fetch(req).then(res => {
          if (res.ok) {
            const cp = res.clone();
            caches.open(CACHE).then(c => c.put(req, cp));
          }
        }).catch(() => {});
        return hit;
      }
      return fetch(req).then(res => {
        if (res.ok) {
          const cp = res.clone();
          caches.open(CACHE).then(c => c.put(req, cp));
        }
        return res;
      });
    })
  );
});
