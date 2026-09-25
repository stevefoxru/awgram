const CACHE = 'zpnet-shell-v11';
const SHELL = ['/', '/assets/app.css', '/assets/app.js', '/assets/icon.svg'];
self.addEventListener('install', event => event.waitUntil(caches.open(CACHE).then(cache => cache.addAll(SHELL))));
self.addEventListener('activate', event => event.waitUntil(caches.keys().then(keys => Promise.all(keys.filter(key => key !== CACHE).map(key => caches.delete(key))))));
self.addEventListener('fetch', event => {
  if (event.request.method !== 'GET' || new URL(event.request.url).pathname.startsWith('/api/')) return;
  event.respondWith(fetch(event.request).then(response => {
    const copy = response.clone();
    caches.open(CACHE).then(cache => cache.put(event.request, copy));
    return response;
  }).catch(() => caches.match(event.request).then(response => response || caches.match('/'))));
});
self.addEventListener('periodicsync', event => {
  if (event.tag !== 'awgram-notifications') return;
  event.waitUntil(fetch('/api/notifications/feed', {credentials:'include'}).then(r => r.ok ? r.json() : null).then(data => {
    const item = data?.items?.find(value => !value.read);
    if (!item) return;
    return self.registration.showNotification(item.title, {body:item.body, icon:'/assets/icon.svg', badge:'/assets/icon.svg', tag:`awgram-${item.id}`, data:{url:item.action_url||'/?view=notifications'}});
  }).catch(() => {}));
});
self.addEventListener('notificationclick', event => {
  event.notification.close();
  const target = event.notification.data?.url || '/?view=notifications';
  event.waitUntil(clients.matchAll({type:'window',includeUncontrolled:true}).then(list => {
    const opened=list[0]; if(opened){opened.navigate(target);return opened.focus()} return clients.openWindow(target);
  }));
});
