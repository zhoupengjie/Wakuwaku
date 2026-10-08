const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('pet', {
  onUpdate: fn => ipcRenderer.on('pet:update', (_, data) => fn(data)),
  onReact: fn => ipcRenderer.on('pet:react', (_, reaction) => fn(reaction)),
  onAlert: fn => ipcRenderer.on('pet:alert', (_, alert) => fn(alert)),
  onCursor: fn => ipcRenderer.on('pet:cursor', (_, at) => fn(at)),
  onDrag: fn => ipcRenderer.on('pet:drag', (_, dx) => fn(dx)),
  onDragEnd: fn => ipcRenderer.on('pet:drag-end', () => fn()),
  onAsks: fn => ipcRenderer.on('pet:asks', (_, list) => fn(list)),
  onShift: fn => ipcRenderer.on('pet:shift', (_, px) => fn(px)),
  answer: (id, choice) => ipcRenderer.send('pet:answer', id, choice),
  dismiss: id => ipcRenderer.send('pet:dismiss', id),
  panel: measured => ipcRenderer.send('pet:panel', measured),
  keyboard: needs => ipcRenderer.send('pet:keyboard', needs),
  hover: isOver => ipcRenderer.send('pet:hover', isOver),
  dragStart: () => ipcRenderer.send('pet:drag-start'),
  dragEnd: () => ipcRenderer.send('pet:drag-end'),
  menu: () => ipcRenderer.send('pet:menu'),
  openSettings: () => ipcRenderer.send('pet:open-settings'),
  walk: (dx, ms) => ipcRenderer.invoke('pet:walk', dx, ms),
  walkStop: () => ipcRenderer.send('pet:walk-stop'),
  // The island's page: she broke free of the drop (her middle this far from
  // the cursor), the button was let go, the island should reach for her or
  // take her in.
  releaseHer: offset => ipcRenderer.send('island:release', offset),
  dropHer: () => ipcRenderer.send('island:drop'),
  onReach: fn => ipcRenderer.on('island:reach', (_, at) => fn(at)),
  onAbsorb: fn => ipcRenderer.on('island:absorb', (_, at) => fn(at)),
})
