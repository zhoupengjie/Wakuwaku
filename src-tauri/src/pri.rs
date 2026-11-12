// Files a Windows resource index (a .pri) holds within itself, by their
// names: Windows' own pictures that are in no file of their own, as the
// Task View button's (Settings' Taskview_Light.svg and Taskview_Dark.svg,
// in Windows.UI.SettingsAppThreshold.pri: taskbar/mod.rs task_view_icons).
//
// Read as the format is laid out (after chausner/PriTools' PriFormat,
// Apache-2.0): a table of sections; a schema section names each resource
// (scopes and items, as a path); a resource map gives each item its
// candidates, one of them the first; a candidate kept within the file
// points at a blob of a data item section. Only that: a resource's first
// candidate, when it is data kept within the file. Only the sections
// needed are read, by seeking: the file is 17 MB.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const SCHEMA: [&[u8; 16]; 2] = [b"[mrm_hschema]  \0", b"[mrm_hschemaex] "];
const MAP: [&[u8; 16]; 2] = [b"[mrm_res_map__]\0", b"[mrm_res_map2_]\0"];
const DATA: &[u8; 16] = b"[mrm_dataitem] \0";

struct Toc {
    id: [u8; 16],
    // Where its content starts in the file, and how long it is.
    at: u64,
    len: u64,
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn read_at(f: &mut File, at: u64, len: usize) -> Option<Vec<u8>> {
    f.seek(SeekFrom::Start(at)).ok()?;
    let mut buf = vec![0; len];
    f.read_exact(&mut buf).ok()?;
    Some(buf)
}

fn toc(f: &mut File) -> Option<Vec<Toc>> {
    let head = read_at(f, 0, 32)?;
    if !head.starts_with(b"mrm_pri") && !head.starts_with(b"mrm_prif") {
        return None;
    }
    let toc_at = u32_at(&head, 16)? as u64;
    let start = u32_at(&head, 20)? as u64;
    let count = u16_at(&head, 24)? as usize;
    let table = read_at(f, toc_at, count * 32)?;
    (0..count)
        .map(|i| {
            let e = &table[i * 32..i * 32 + 32];
            // The section's own header (32 bytes) and trailer (8) round its content.
            let at = start + u32_at(e, 24)? as u64 + 32;
            let len = (u32_at(e, 28)? as u64).checked_sub(40)?;
            Some(Toc { id: e[..16].try_into().ok()?, at, len })
        })
        .collect()
}

// A schema section's items by their index, each by its whole path
// ("Files\SystemSettings\Assets\Taskview_Light.svg"), the root's empty name
// left out.
fn item_names(b: &[u8], extended: bool) -> Option<HashMap<u16, String>> {
    if b.is_empty() {
        return Some(HashMap::new());
    }
    let unique_len = u16_at(b, 2)? as usize;
    let name_len = u16_at(b, 4)? as usize;
    let mut at = 8;
    let mut names_x = false;
    if extended {
        names_x = match b.get(at..at + 16)? {
            x if x == b"[def_hnamesx]  \0" => true,
            x if x == b"[def_hnames]   \0" => false,
            _ => return None,
        };
        at += 16;
    }
    let scopes = u32_at(b, at + 12)? as usize;
    let items = u32_at(b, at + 16)? as usize;
    at += 20 + (unique_len + name_len) * 2 + 2;
    let unicode_len = u32_at(b, at + 16)? as usize;
    let ascii_len = if names_x { u32_at(b, at + 24)? as usize } else { 0 };
    at += if names_x { 28 } else { 24 };
    let nodes = scopes + items;
    let infos = at;
    let unicode = infos + nodes * 12 + scopes * 8 + items * 2;
    let ascii = unicode + unicode_len * 2;
    if b.len() < ascii + ascii_len {
        return None;
    }
    struct Node {
        parent: usize,
        named: bool,
        scope: bool,
        index: u16,
        name: String,
    }
    let mut all = Vec::with_capacity(nodes);
    for i in 0..nodes {
        let n = infos + i * 12;
        let flags = b[n + 7];
        let offset = u16_at(b, n + 8)? as usize | ((flags as usize & 0xF) << 16);
        let named = u16_at(b, n + 2)? != 0;
        let name = if !named {
            String::new()
        } else if flags & 0x20 != 0 {
            let s = b.get(ascii + offset..ascii + ascii_len)?;
            String::from_utf8_lossy(&s[..s.iter().position(|&c| c == 0)?]).into_owned()
        } else {
            let s = b.get(unicode + offset * 2..ascii)?;
            let units: Vec<u16> = s.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
            String::from_utf16_lossy(&units)
        };
        all.push(Node { parent: u16_at(b, n)? as usize, named, scope: flags & 0x10 != 0, index: u16_at(b, n + 10)?, name });
    }
    let path = |mut i: usize| {
        let mut parts = Vec::new();
        for _ in 0..64 {
            let node = all.get(i)?;
            if node.named {
                parts.push(node.name.as_str());
            }
            if node.parent == i || node.parent >= all.len() {
                break;
            }
            i = node.parent;
        }
        parts.reverse();
        Some(parts.join("\\"))
    };
    Some(all.iter().enumerate().filter(|(_, n)| !n.scope).filter_map(|(i, n)| Some((n.index, path(i)?))).collect())
}

// A resource map's first candidate for each item asked for, when it is data
// kept within the file: (its data item section, the item there).
fn first_data(b: &[u8], wanted: &HashMap<u16, usize>) -> Option<HashMap<usize, (usize, usize)>> {
    let env_len = u16_at(b, 0)? as usize;
    let schema_ref_len = u16_at(b, 6)? as usize;
    let types = u16_at(b, 10)? as usize;
    let to_groups = u16_at(b, 12)? as usize;
    let groups = u16_at(b, 14)? as usize;
    let infos = u32_at(b, 16)? as usize;
    let candidates = u32_at(b, 20)? as usize;
    let large_len = u32_at(b, 28)? as usize;
    let mut at = 32 + env_len + schema_ref_len;
    let type_table: Vec<u32> = (0..types).map(|i| u32_at(b, at + i * 8 + 4)).collect::<Option<_>>()?;
    at += types * 8;
    let mut to_group: Vec<(usize, usize)> = (0..to_groups).map(|i| Some((u16_at(b, at + i * 4)? as usize, u16_at(b, at + i * 4 + 2)? as usize))).collect::<Option<_>>()?;
    at += to_groups * 4;
    let mut group: Vec<(usize, usize)> = (0..groups).map(|i| Some((u16_at(b, at + i * 4)? as usize, u16_at(b, at + i * 4 + 2)? as usize))).collect::<Option<_>>()?;
    at += groups * 4;
    let mut info: Vec<usize> = (0..infos).map(|i| Some(u16_at(b, at + i * 4 + 2)? as usize)).collect::<Option<_>>()?;
    at += infos * 4;
    // The large table: more of each, in 32 bits.
    if large_len != 0 {
        let l = b.get(at..at + large_len)?;
        let (n_to, n_groups, n_infos) = (u32_at(l, 0)? as usize, u32_at(l, 4)? as usize, u32_at(l, 8)? as usize);
        let mut p = 12;
        for _ in 0..n_to {
            to_group.push((u32_at(l, p)? as usize, u32_at(l, p + 4)? as usize));
            p += 8;
        }
        for _ in 0..n_groups {
            group.push((u32_at(l, p)? as usize, u32_at(l, p + 4)? as usize));
            p += 8;
        }
        for _ in 0..n_infos {
            info.push(u32_at(l, p + 4)? as usize);
            p += 8;
        }
    }
    at += large_len;
    let cands = at;
    if b.len() < cands + candidates * 8 {
        return None;
    }
    let mut found = HashMap::new();
    for &(first_item, g) in &to_group {
        let (size, first_info) = group.get(g).copied().unwrap_or((1, g.wrapping_sub(group.len())));
        for i in first_info..first_info.saturating_add(size) {
            let item = first_item + (i - first_info);
            let Some(&want) = u16::try_from(item).ok().and_then(|item| wanted.get(&item)) else { continue };
            let c = cands + *info.get(i)? * 8;
            let Some(&kind) = b.get(c + 1).and_then(|&t| type_table.get(t as usize)) else { continue };
            // Kept within the file (1), as data (2: EmbeddedData).
            if b[c] == 1 && kind == 2 {
                found.insert(want, (u16_at(b, c + 6)? as usize, u16_at(b, c + 4)? as usize));
            }
        }
    }
    Some(found)
}

// The data of a data item section's item.
fn data_item(f: &mut File, sec: &Toc, item: usize) -> Option<Vec<u8>> {
    let head = read_at(f, sec.at, 12)?;
    let strings = u16_at(&head, 4)? as usize;
    let blobs = u16_at(&head, 6)? as usize;
    let data = 12 + strings * 4 + blobs * 8;
    let (offset, len) = if item < strings {
        let e = read_at(f, sec.at + 12 + item as u64 * 4, 4)?;
        (u16_at(&e, 0)? as u64, u16_at(&e, 2)? as usize)
    } else if item < strings + blobs {
        let e = read_at(f, sec.at + 12 + strings as u64 * 4 + (item - strings) as u64 * 8, 8)?;
        (u32_at(&e, 0)? as u64, u32_at(&e, 4)? as usize)
    } else {
        return None;
    };
    if data as u64 + offset + len as u64 > sec.len {
        return None;
    }
    read_at(f, sec.at + data as u64 + offset, len)
}

// The files the index holds within itself, by their names (a path's end,
// as "Files/SystemSettings/Assets/Taskview_Light.svg", in any case and
// either slash), each as named; None for one not there.
pub fn embedded(file: &Path, names: &[&str]) -> Vec<Option<Vec<u8>>> {
    let mut out = vec![None; names.len()];
    let _ = (|| -> Option<()> {
        let mut f = File::open(file).ok()?;
        let toc = toc(&mut f)?;
        let wanted: Vec<String> = names.iter().map(|n| n.replace('/', "\\").to_lowercase()).collect();
        for map in toc.iter().filter(|t| MAP.iter().any(|id| **id == t.id)) {
            let b = read_at(&mut f, map.at, map.len as usize)?;
            let schema = toc.get(u16_at(&b, 4)? as usize)?;
            let Some(extended) = SCHEMA.iter().position(|id| **id == schema.id) else { continue };
            let names = item_names(&read_at(&mut f, schema.at, schema.len as usize)?, extended == 1)?;
            let mine: HashMap<u16, usize> = names
                .iter()
                .filter_map(|(&item, name)| {
                    let name = name.to_lowercase();
                    let i = wanted.iter().position(|w| name == *w || name.ends_with(&format!("\\{w}")))?;
                    Some((item, i))
                })
                .collect();
            if mine.is_empty() {
                continue;
            }
            for (i, (sec, item)) in first_data(&b, &mine)? {
                let Some(sec) = toc.get(sec).filter(|t| t.id == *DATA) else { continue };
                if out[i].is_none() {
                    out[i] = data_item(&mut f, sec, item);
                }
            }
        }
        Some(())
    })();
    out
}

#[cfg(all(test, windows))]
mod tests {
    use super::embedded;
    use std::path::Path;

    // Settings' Task View pictures, where this Windows has them (11).
    #[test]
    fn settings_task_view_pictures() {
        let file = Path::new(r"C:\Windows\SystemResources\Windows.UI.SettingsAppThreshold\Windows.UI.SettingsAppThreshold.pri");
        if !file.exists() {
            return;
        }
        let got = embedded(file, &["Files/SystemSettings/Assets/Taskview_Light.svg", "Files/SystemSettings/Assets/Taskview_Dark.svg", "Files/No/Such.svg"]);
        for svg in &got[..2] {
            let svg = String::from_utf8(svg.clone().expect("found")).unwrap();
            assert!(svg.trim_start().starts_with("<svg") && svg.trim_end().ends_with("</svg>"), "{svg}");
        }
        assert_ne!(got[0], got[1]);
        assert!(got[2].is_none());
    }

    #[test]
    fn not_an_index() {
        let file = std::env::temp_dir().join("wakuwaku-not-a-pri.pri");
        std::fs::write(&file, b"nothing of the kind").unwrap();
        assert_eq!(embedded(&file, &["a.svg"]), vec![None]);
        assert_eq!(embedded(Path::new("Z:\\no\\such.pri"), &["a.svg"]), vec![None]);
    }
}
