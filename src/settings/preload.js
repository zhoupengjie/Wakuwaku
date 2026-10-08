const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('settings', {
  get: () => ipcRenderer.invoke('settings:get'),
  set: patch => ipcRenderer.invoke('settings:set', patch),
  fetchPet: ref => ipcRenderer.invoke('settings:fetch', ref),
  gallery: query => ipcRenderer.invoke('settings:gallery', query),
  hooks: action => ipcRenderer.invoke('settings:hooks', action),
  login: on => ipcRenderer.invoke('settings:login', on),
  answer: (id, choice) => ipcRenderer.invoke('settings:answer', id, choice),
  dismiss: id => ipcRenderer.invoke('settings:dismiss', id),
  showPet: on => ipcRenderer.invoke('settings:show-pet', on),
  copy: text => ipcRenderer.invoke('settings:copy', text),
  open: where => ipcRenderer.invoke('settings:open-site', where),
  onChanged: fn => ipcRenderer.on('settings:changed', (_, snapshot) => fn(snapshot)),
})
