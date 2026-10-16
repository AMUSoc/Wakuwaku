// The phone page's service worker: what it may be added to the home screen
// by. The page itself and its files from the network first, the last copy
// kept for when she cannot be reached (the page then says so); her data
// (api/) and the pets' sheets never kept here.
const CACHE = 'wakuwaku-mobile-1'
const SHELL = ['./', 'app.js', 'i18n.js', 'sprite.js', 'status.js', 'style.css', 'manifest.webmanifest', 'icon-192.png', 'icon-512.png']

self.addEventListener('install', e => {
  e.waitUntil(
    caches
      .open(CACHE)
      .then(c => c.addAll(SHELL))
      .catch(() => {})
      .then(() => self.skipWaiting()),
  )
})

self.addEventListener('activate', e => {
  e.waitUntil(
    caches
      .keys()
      .then(keys => Promise.all(keys.filter(k => k !== CACHE).map(k => caches.delete(k))))
      .then(() => self.clients.claim()),
  )
})

self.addEventListener('fetch', e => {
  const url = new URL(e.request.url)
  if (e.request.method !== 'GET' || url.origin !== location.origin) return
  const name = url.pathname.slice(url.pathname.lastIndexOf('/mobile/') + '/mobile/'.length)
  if (name.startsWith('api/') || name.startsWith('sheet/') || url.search) return
  e.respondWith(
    fetch(e.request)
      .then(res => {
        if (res.ok) {
          const copy = res.clone()
          caches.open(CACHE).then(c => c.put(e.request, copy))
        }
        return res
      })
      .catch(() => caches.match(e.request).then(hit => hit || Response.error())),
  )
})
