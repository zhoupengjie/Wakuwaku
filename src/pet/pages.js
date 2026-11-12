// What the page's big scripts show the others, set as each one runs and read
// where it is used. The island's (island.js: Island, and Isle for the picker
// and the capsule's panel), the settings' (settings.js), the picker's
// (picker.js), the capsule's panel (capsule-panel.js), her own window's
// (pet.js: HerTalk) and the desktop's (desk.js: Desk). Each is null until its
// script has run; a script that runs before another (the island before the
// settings, the picker before the capsule's panel) reaches it only once the
// page is up, as before. The scripts still run in the order their pages list
// them: importing one of these from its own file would run it first.
//
// Each is put on window too, for what looks in from outside alone: the
// preview's scenes and playground, and /debug/eval. The pages' own scripts
// never read them there.
export let Island = null
export let Isle = null
export let Settings = null
export let Picker = null
export let CapsulePanel = null
export let HerTalk = null
export let Desk = null

export function provide(name, face) {
  switch (name) {
    case 'Island':
      Island = face
      break
    case 'Isle':
      Isle = face
      break
    case 'Settings':
      Settings = face
      break
    case 'Picker':
      Picker = face
      break
    case 'CapsulePanel':
      CapsulePanel = face
      break
    case 'HerTalk':
      HerTalk = face
      break
    case 'Desk':
      Desk = face
      break
    default:
      throw new Error(`no face ${name}`)
  }
  window[name] = face
}
