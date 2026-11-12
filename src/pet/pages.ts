// What the page's big scripts show the others, set as each one runs and read
// where it is used. The island's (island.ts: Island, and Isle for the picker
// and the capsule's panel), the settings' (settings.ts), the picker's
// (picker.ts), the capsule's panel (capsule-panel.ts), her own window's
// (pet.ts: HerTalk) and the desktop's (desk.ts: Desk). Each is null until its
// script has run; a script that runs before another (the island before the
// settings, the picker before the capsule's panel) reaches it only once the
// page is up, as before. The scripts still run in the order their pages list
// them: importing one of these from its own file would run it first. Their
// types come from those files (import type: nothing of theirs runs).
//
// Each is put on window too, for what looks in from outside alone: the
// preview's scenes and playground, and /debug/eval. The pages' own scripts
// never read them there.
import type { CapsulePanelFace } from './capsule-panel.ts'
import type { DeskFace } from './desk.ts'
import type { IslandFace, IsleFace } from './island.ts'
import type { HerTalkFace } from './pet.ts'
import type { PickerFace } from './picker.ts'
import type { SettingsFace } from './settings.ts'

interface Faces {
  Island: IslandFace
  Isle: IsleFace
  // On her own window's page, isOpen alone (always false).
  Settings: Partial<SettingsFace>
  // None unless the island is on (the picker), and the picker there (its panel).
  Picker: PickerFace | null
  CapsulePanel: CapsulePanelFace | null
  HerTalk: HerTalkFace
  Desk: DeskFace
}

export let Island: Faces['Island'] = null
export let Isle: Faces['Isle'] = null
export let Settings: Faces['Settings'] = null
export let Picker: Faces['Picker'] = null
export let CapsulePanel: Faces['CapsulePanel'] = null
export let HerTalk: Faces['HerTalk'] = null
export let Desk: Faces['Desk'] = null

export function provide<K extends keyof Faces>(name: K, face: Faces[K]): void {
  switch (name) {
    case 'Island':
      Island = face as Faces['Island']
      break
    case 'Isle':
      Isle = face as Faces['Isle']
      break
    case 'Settings':
      Settings = face as Faces['Settings']
      break
    case 'Picker':
      Picker = face as Faces['Picker']
      break
    case 'CapsulePanel':
      CapsulePanel = face as Faces['CapsulePanel']
      break
    case 'HerTalk':
      HerTalk = face as Faces['HerTalk']
      break
    case 'Desk':
      Desk = face as Faces['Desk']
      break
    default:
      throw new Error(`no face ${name}`)
  }
  ;(window as unknown as Record<string, unknown>)[name] = face
}
