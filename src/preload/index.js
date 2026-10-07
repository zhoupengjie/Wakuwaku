const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('pet', {
  onUpdate: fn => ipcRenderer.on('pet:update', (_, data) => fn(data)),
  onReact: fn => ipcRenderer.on('pet:react', (_, reaction) => fn(reaction)),
  onCursor: fn => ipcRenderer.on('pet:cursor', (_, at) => fn(at)),
  onDrag: fn => ipcRenderer.on('pet:drag', (_, dx) => fn(dx)),
  onDragEnd: fn => ipcRenderer.on('pet:drag-end', () => fn()),
  hover: isOver => ipcRenderer.send('pet:hover', isOver),
  dragStart: () => ipcRenderer.send('pet:drag-start'),
  dragEnd: () => ipcRenderer.send('pet:drag-end'),
  menu: () => ipcRenderer.send('pet:menu'),
  walk: (dx, ms) => ipcRenderer.invoke('pet:walk', dx, ms),
  walkStop: () => ipcRenderer.send('pet:walk-stop'),
})
