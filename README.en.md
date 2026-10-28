<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/readme/hero-en-dark.jpg">
  <img src="docs/readme/hero-en-light.jpg" alt="Wakuwaku: a Windows taskbar that knows all your AI sessions. She also happens to be a pet." width="100%">
</picture>

<p align="center">
  <a href="https://github.com/zhoupengjie/Wakuwaku/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/zhoupengjie/Wakuwaku?style=flat-square&amp;label=release&amp;color=ff7eb3"></a>
  <img alt="Windows 11" src="https://img.shields.io/badge/Windows-11-0078d4?style=flat-square">
  <img alt="About 11 MB" src="https://img.shields.io/badge/one%20exe-about%2011%20MB-8b5cf6?style=flat-square">
  <a href="LICENSE"><img alt="MIT" src="https://img.shields.io/badge/license-MIT-34d27b?style=flat-square"></a>
</p>

<p align="center">
  <a href="https://github.com/zhoupengjie/Wakuwaku/releases/latest"><picture><source media="(prefers-color-scheme: dark)" srcset="docs/readme/btn-download-en-dark.png"><img src="docs/readme/btn-download-en-light.png" alt="Download for Windows" height="44"></picture></a>
  <a href="docs/GUIDE.en.md"><picture><source media="(prefers-color-scheme: dark)" srcset="docs/readme/btn-guide-en-dark.png"><img src="docs/readme/btn-guide-en-light.png" alt="User guide" height="44"></picture></a>
  <a href="README.md"><picture><source media="(prefers-color-scheme: dark)" srcset="docs/readme/btn-lang-en-dark.png"><img src="docs/readme/btn-lang-en-light.png" alt="简体中文" height="44"></picture></a>
</p>

<p align="center"><sub>The name is Anya's "waku waku" from SPY×FAMILY: excited, can't wait. AI works, and she watches, all eager.</sub></p>

## What she does

<table>
<tr>
<td width="50%" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/readme/card-sessions-en-dark.jpg"><img src="docs/readme/card-sessions-en-light.jpg" alt="The capsule opened: three sessions at work, each with its step and time" width="100%"></picture>
<p><b>She knows all your AI sessions</b><br>What each one is doing, how far it has got and for how long, right in the capsule. Click one to jump to its window: a terminal, VS Code, or the Claude desktop app.</p>
</td>
<td width="50%" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/readme/card-ask-en-dark.jpg"><img src="docs/readme/card-ask-en-light.jpg" alt="An approval on the taskbar: allow, always allow, deny" width="100%"></picture>
<p><b>Needs your OK? Click it on the taskbar</b><br>Allow, always allow or deny; answer its questions; approve a plan. The terminal still asks too, and whichever answers first counts.</p>
</td>
</tr>
<tr>
<td width="50%" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/readme/card-ring-en-dark.jpg"><img src="docs/readme/card-ring-en-light.jpg" alt="Done: a ring of colour round the capsule" width="100%"></picture>
<p><b>Done, and the capsule lights up</b><br>A ring of colour runs round it and a small dot stays on her, without getting in your way. Rest the pointer on it for the files changed and what Claude said.</p>
</td>
<td width="50%" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/readme/card-island-en-dark.jpg"><img src="docs/readme/card-island-en-light.jpg" alt="The island at the top of the screen, open: sessions, the system monitor, the weather" width="100%"></picture>
<p><b>Or just an island</b><br>Keep your own taskbar and she lives in an island at the top of the screen. Next to tools that run the taskbar themselves, like MyDockFinder, she moves there on her own.</p>
</td>
</tr>
<tr>
<td colspan="2" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/readme/card-taskbar-en-dark.jpg"><img src="docs/readme/card-taskbar-en-light.jpg" alt="Her taskbar: Start, pinned programs, the weather, mail, the tray, the keyboard, the clock" width="100%"></picture>
<p><b>As easy as Windows 11's own</b><br>Start, pinned programs, window thumbnails, the tray, the input method, quick settings and the clock, all with Windows' own icons and flyouts. Plugins can sit on it too.</p>
</td>
</tr>
<tr>
<td colspan="2" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/readme/card-desktop-en-dark.jpg"><img src="docs/readme/card-desktop-en-light.jpg" alt="A board on the desktop: Claude today, mail, stocks, the system monitor, a pomodoro, a countdown" width="100%"></picture>
<p><b>Plugins, and widgets on your desktop</b><br>Today's usage and tokens, the weather, stocks, a pomodoro, a countdown, your mail: on the taskbar, in the capsule or on the desktop, a few of them together on a board. One line of PowerShell makes a plugin of your own.</p>
</td>
</tr>
</table>

And more: new mail shows up in the capsule first, and you can hand a letter to Claude Code, Codex or Antigravity to sum up and draft a reply (whether it goes out is up to you). Any pet from [codex-pets.net](https://codex-pets.net); pull her up and she hops onto your desktop. Mica, clear or solid, dark or light, and the capsule and the island the way you like them. About 11 MB, one exe, nothing to install, free and open source.

<sub>The pictures are her real pages, rendered by [`tools/preview`](tools/preview/README.md); the sessions, mail and numbers are made up for the demo, and the backgrounds are the repository's own [wallpapers](wallpapers/).</sub>

## The AI tools she works with

| | Status | Approve on her | Questions | Stats | Mail |
| --- | :---: | :---: | :---: | :---: | :---: |
| Claude Code | ✓ | Allow · Always allow · Deny · plans | ✓ | ✓ with tokens | ✓ |
| Codex | ✓ | Allow · Deny | | ✓ with tokens | ✓ |
| DeepSeek Harness | ✓ | Allow · Deny | ✓ | ✓ with tokens | |
| OpenCode | ✓ | Allow · Always allow · Deny | | ✓ no tokens yet | |
| ZCode | ✓ | Allow · Deny | | ✓ no tokens yet | |
| Antigravity | ✓ | Allow · Always allow · Deny | | ✓ with tokens | ✓ |
| oh-my-pi (omp) | ✓ | Plan reviews only | | ✓ with tokens | |
| Reasonix | ✓ | | | ✓ tokens in 2.x | |

Status: she follows its sessions working, waiting for you, done, failed. Stats: its turns and time counted in Today. Mail: you can hand it a letter to sum up and draft a reply. A blank cell means the tool gives her no way to do it, so you answer that in the tool itself. How to connect each one, the file it writes and what to look out for: [User guide → Install](docs/GUIDE.en.md#install).

## Get started

1. Download `wakuwaku-<version>-windows-x64.exe` from [Releases](https://github.com/zhoupengjie/Wakuwaku/releases/latest) and run it from any folder. The exe isn't code-signed: if SmartScreen stops it, click More info → Run anyway.
2. The settings open by themselves the first time, with a welcome card at the top: click **Connect** next to the agents you use (the Agent page has Connect all too). If one needs a step more (Codex wants you to trust her hooks in its `/hooks`), the card says so. For Claude Code you can install the plugin instead: `/plugin marketplace add zhoupengjie/wakuwaku`, then `/plugin install wakuwaku@wakuwaku`.
3. Get to work. She tells you when an AI needs your OK, finishes, or fails.

Done with her? Remove her connections in the settings, quit her, and delete the exe and the `wakuwaku-data` folder next to it; see [Uninstall](docs/GUIDE.en.md#uninstall).

## See her at work

https://github.com/user-attachments/assets/7814d71a-77f0-43af-97a6-210d2cbb31dc

<p><sub>The video's captions are in Chinese.</sub></p>

## Docs

- [User guide](docs/GUIDE.en.md): installing and connecting each agent, her homes, the approval panel, plugins, mail, updates, crash reports, uninstalling, how she works
- [Writing plugins](docs/PLUGINS.md): a .zip of web files and pictures, run in a sandbox; or [send data from a script](docs/WIDGETS.md) (in Chinese)
- [Development](docs/DEVELOPMENT.md) (in Chinese): running from source, tests, how each part is built

## License and credits

- The code is under the [MIT License](LICENSE).
- The pet sprites come from [codex-pets.net](https://codex-pets.net) and belong to their authors. **Except the default pet, they are not in this repository or the exe**; you download them in the settings.
- The default pet, [Claude小姐](https://codex-pets.net/#/pets/claude-chan), is pixel art by zhoupengjie: an unofficial fan work of the Claude小姐 character by Bilibili creator [ZipZipPipe](https://space.bilibili.com/4168597) (the character's original creator). It is in this repository (`pets/claude-chan/`) and carried in the exe: the first run puts her in place with nothing to download.
- The 16 look directions follow codex-pets.net's mapping (clockwise from straight up, one step per 22.5°).
