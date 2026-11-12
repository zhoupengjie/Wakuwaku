// desk.html's one script: the desktop's page, a window on each display with
// widgets (desk.ts), and the drop of a widget from her picker onto it
// (desk-drop.ts), once the desktop is made; each made apart (entry.ts).
// What desk.ts hands back is put on window too, for what looks in from
// outside alone (the preview's scenes, /debug/eval).
import { deskPage } from './desk.ts'
import { deskDrop } from './desk-drop.ts'
import { expose, made } from './entry.ts'

const Desk = made(deskPage)
expose({ Desk })
if (Desk?.inner) made(() => deskDrop(Desk.inner))
