const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('settings', {
  get: () => ipcRenderer.invoke('settings:get'),
  set: patch => ipcRenderer.invoke('settings:set', patch),
  fetchPet: ref => ipcRenderer.invoke('settings:fetch', ref),
  hooks: action => ipcRenderer.invoke('settings:hooks', action),
  login: on => ipcRenderer.invoke('settings:login', on),
  openSite: () => ipcRenderer.invoke('settings:open-site'),
  onChanged: fn => ipcRenderer.on('settings:changed', (_, snapshot) => fn(snapshot)),
})
