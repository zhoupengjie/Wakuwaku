const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('home', {
  get: () => ipcRenderer.invoke('home:get'),
  set: patch => ipcRenderer.invoke('home:set', patch),
  fetchPet: ref => ipcRenderer.invoke('home:fetch', ref),
  gallery: query => ipcRenderer.invoke('home:gallery', query),
  hooks: action => ipcRenderer.invoke('home:hooks', action),
  login: on => ipcRenderer.invoke('home:login', on),
  answer: (id, choice) => ipcRenderer.invoke('home:answer', id, choice),
  dismiss: id => ipcRenderer.invoke('home:dismiss', id),
  showPet: on => ipcRenderer.invoke('home:show-pet', on),
  copy: text => ipcRenderer.invoke('home:copy', text),
  open: where => ipcRenderer.invoke('home:open-site', where),
  onChanged: fn => ipcRenderer.on('home:changed', (_, snapshot) => fn(snapshot)),
})
