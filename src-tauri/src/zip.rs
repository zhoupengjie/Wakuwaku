// Zips read and unpacked: LibreHardwareMonitor's download (lhm_setup.rs)
// and a plugin's package (addons/). Stored and deflated entries, each
// checked by its size and CRC; every name checked, no path out of the
// folder; no ZIP64 (neither is past a few hundred MB), no encryption, names
// in UTF-8 or ASCII. Moved here from lhm_setup.rs when plugins came.
use std::io::Read;
use std::path::{Path, PathBuf};

// How much one may hold: entries (its folders too), one entry unpacked, all
// of them unpacked.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub entries: usize,
    pub file: u64,
    pub all: u64,
}

// LibreHardwareMonitor's: generous, it is a few MB.
pub const LHM: Limits = Limits { entries: 5000, file: 256 << 20, all: 512 << 20 };

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    method: u16,
    crc: u32,
    packed: u64,
    pub size: u64,
    header: u64,
}

impl Entry {
    // A folder's entry: its name ends in a slash.
    pub fn is_folder(&self) -> bool {
        self.name.ends_with('/') || self.name.ends_with('\\')
    }
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

// Its entries, from its central directory (found from the end: the
// end-of-directory record and its comment), within LHM's limits.
pub fn entries(b: &[u8]) -> Result<Vec<Entry>, String> {
    entries_within(b, &LHM)
}

// The same, within the limits given.
pub fn entries_within(b: &[u8], limits: &Limits) -> Result<Vec<Entry>, String> {
    let from = b.len().saturating_sub(22 + 65535);
    let end = (from..b.len().saturating_sub(21)).rev().find(|&i| u32_at(b, i) == Some(0x0605_4b50)).ok_or("no zip directory")?;
    let count = usize::from(u16_at(b, end + 10).ok_or("cut short")?);
    let at = u32_at(b, end + 16).ok_or("cut short")? as usize;
    if count == 0xFFFF || at == 0xFFFF_FFFF {
        return Err("ZIP64".into());
    }
    if count > limits.entries {
        return Err(format!("{count} entries"));
    }
    let mut out = Vec::with_capacity(count);
    let mut p = at;
    let mut all = 0u64;
    for _ in 0..count {
        if u32_at(b, p) != Some(0x0201_4b50) {
            return Err("bad directory entry".into());
        }
        let flags = u16_at(b, p + 8).ok_or("cut short")?;
        let method = u16_at(b, p + 10).ok_or("cut short")?;
        let crc = u32_at(b, p + 16).ok_or("cut short")?;
        let packed = u32_at(b, p + 20).ok_or("cut short")?;
        let size = u32_at(b, p + 24).ok_or("cut short")?;
        let (n, e, c) = (usize::from(u16_at(b, p + 28).ok_or("cut short")?), usize::from(u16_at(b, p + 30).ok_or("cut short")?), usize::from(u16_at(b, p + 32).ok_or("cut short")?));
        let header = u32_at(b, p + 42).ok_or("cut short")?;
        let raw = b.get(p + 46..p + 46 + n).ok_or("cut short")?;
        if flags & 1 != 0 {
            return Err("encrypted".into());
        }
        if [packed, size, header].contains(&0xFFFF_FFFF) {
            return Err("ZIP64".into());
        }
        // Bit 11: UTF-8; otherwise code page 437, taken only when ASCII.
        let name = if flags & 0x800 != 0 || raw.is_ascii() { String::from_utf8(raw.to_vec()).map_err(|_| "a name not in UTF-8")? } else { return Err("a name not in ASCII nor UTF-8".into()) };
        all += u64::from(size);
        if u64::from(size) > limits.file || all > limits.all {
            return Err("too big unpacked".into());
        }
        out.push(Entry { name, method, crc, packed: u64::from(packed), size: u64::from(size), header: u64::from(header) });
        p += 46 + n + e + c;
    }
    Ok(out)
}

// A name as a path within the folder: its parts, none of them empty, "."
// or "..", none with a drive, a stream (:), a letter Windows refuses in
// a name, a device's name (CON, NUL, COM1…), nor ending in a dot or a
// space; None for any other. A folder's (ending in /) has its parts too.
pub fn safe(name: &str) -> Option<Vec<String>> {
    let name = name.replace('\\', "/");
    if name.starts_with('/') {
        return None;
    }
    let parts: Vec<&str> = name.trim_end_matches('/').split('/').collect();
    let device = |p: &str| {
        let stem = p.split('.').next().unwrap_or("").to_ascii_uppercase();
        matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$") || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit())
    };
    let ok = |p: &&str| !p.is_empty() && *p != "." && *p != ".." && !p.ends_with('.') && !p.ends_with(' ') && !device(p) && !p.chars().any(|c| c < ' ' || "<>:\"|?*".contains(c));
    (parts.iter().all(ok)).then(|| parts.iter().map(|p| p.to_string()).collect())
}

// One entry's bytes, inflated where deflated, checked by its size and CRC.
pub fn bytes_of(b: &[u8], e: &Entry) -> Result<Vec<u8>, String> {
    let h = e.header as usize;
    if u32_at(b, h) != Some(0x0403_4b50) {
        return Err(format!("{}: no local header", e.name));
    }
    let (n, x) = (usize::from(u16_at(b, h + 26).ok_or("cut short")?), usize::from(u16_at(b, h + 28).ok_or("cut short")?));
    let at = h + 30 + n + x;
    let packed = b.get(at..at + e.packed as usize).ok_or_else(|| format!("{}: cut short", e.name))?;
    let data = match e.method {
        0 => packed.to_vec(),
        8 => {
            let mut out = Vec::with_capacity(e.size as usize);
            flate2::read::DeflateDecoder::new(packed).take(e.size + 1).read_to_end(&mut out).map_err(|err| format!("{}: {err}", e.name))?;
            out
        }
        m => return Err(format!("{}: method {m}", e.name)),
    };
    if data.len() as u64 != e.size {
        return Err(format!("{}: {} bytes, not {}", e.name, data.len(), e.size));
    }
    if crc32fast::hash(&data) != e.crc {
        return Err(format!("{}: CRC", e.name));
    }
    Ok(data)
}

// How many leading parts every name loses: one folder that holds all, 1;
// else 0. Each name's parts, and whether it is a folder's.
pub fn strip_of(named: &[(Vec<String>, bool)]) -> usize {
    let top = named.first().map(|(p, _)| p[0].clone());
    usize::from(top.is_some_and(|top| named.iter().all(|(p, folder)| p[0] == top && (p.len() > 1 || *folder))))
}

// The whole zip unpacked into `dir` (made), every name safe or none of
// it; one folder that holds everything taken away, so its files land in
// `dir` itself. The files written.
pub fn extract(b: &[u8], dir: &Path) -> Result<usize, String> {
    let entries = entries(b)?;
    let mut named = Vec::with_capacity(entries.len());
    for e in &entries {
        let parts = safe(&e.name).ok_or_else(|| format!("a name not allowed: {:?}", e.name))?;
        named.push((parts, e.is_folder()));
    }
    let strip = strip_of(&named);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut written = 0;
    for (e, (parts, folder)) in entries.iter().zip(named) {
        let rel: PathBuf = parts.iter().skip(strip).collect();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let to = dir.join(&rel);
        if folder {
            std::fs::create_dir_all(&to).map_err(|err| format!("{}: {err}", to.display()))?;
            continue;
        }
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
        }
        std::fs::write(&to, bytes_of(b, e)?).map_err(|err| format!("{}: {err}", to.display()))?;
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
pub mod tests {
    // A zip as Windows' and .NET's makers write one: stored and deflated
    // entries, folders, a UTF-8 name (for the tests here, lhm_setup.rs's
    // and addons/'s).
    pub fn zip_of(files: &[(&str, &[u8], bool)]) -> Vec<u8> {
        use flate2::write::DeflateEncoder;
        use std::io::Write;
        let (mut out, mut dir) = (Vec::new(), Vec::new());
        for (name, data, deflate) in files {
            let packed = if *deflate {
                let mut e = DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(data).unwrap();
                e.finish().unwrap()
            } else {
                data.to_vec()
            };
            let (method, crc, at) = (if *deflate { 8u16 } else { 0 }, crc32fast::hash(data), out.len() as u32);
            let utf8 = if name.is_ascii() { 0u16 } else { 0x800 };
            let head = |sig: u32| {
                let mut h = sig.to_le_bytes().to_vec();
                if sig == 0x0201_4b50 {
                    h.extend(20u16.to_le_bytes());
                }
                h.extend(20u16.to_le_bytes());
                h.extend(utf8.to_le_bytes());
                h.extend(method.to_le_bytes());
                h.extend([0u8; 4]);
                h.extend(crc.to_le_bytes());
                h.extend((packed.len() as u32).to_le_bytes());
                h.extend((data.len() as u32).to_le_bytes());
                h.extend((name.len() as u16).to_le_bytes());
                h.extend(0u16.to_le_bytes());
                if sig == 0x0201_4b50 {
                    // its comment's length, disk, inner and outer attributes
                    h.extend([0u8; 10]);
                    h.extend(at.to_le_bytes());
                }
                h.extend(name.as_bytes());
                h
            };
            out.extend(head(0x0403_4b50));
            out.extend(&packed);
            dir.extend(head(0x0201_4b50));
        }
        let (at, len) = (out.len() as u32, dir.len() as u32);
        out.extend(dir);
        out.extend(0x0605_4b50u32.to_le_bytes());
        out.extend([0u8; 4]);
        out.extend((files.len() as u16).to_le_bytes());
        out.extend((files.len() as u16).to_le_bytes());
        out.extend(len.to_le_bytes());
        out.extend(at.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out
    }

    #[test]
    fn limits_are_kept() {
        let zip = zip_of(&[("a.txt", b"aaaa", true), ("b.txt", b"bbbb", false)]);
        assert_eq!(super::entries_within(&zip, &super::Limits { entries: 1, file: 100, all: 100 }), Err("2 entries".into()));
        assert_eq!(super::entries_within(&zip, &super::Limits { entries: 9, file: 3, all: 100 }), Err("too big unpacked".into()));
        assert_eq!(super::entries_within(&zip, &super::Limits { entries: 9, file: 4, all: 7 }), Err("too big unpacked".into()));
        assert_eq!(super::entries_within(&zip, &super::Limits { entries: 2, file: 4, all: 8 }).map(|e| e.len()), Ok(2));
    }
}
