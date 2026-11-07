use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // The source tree she is built in (data.rs: an exe run from its target
    // folder keeps its data in the tree), unless this is a release asset
    // (scripts/build-release.ps1 sets WAKUWAKU_ASSET=1): one runs only from
    // where it was put, and carries no path of the machine that built it.
    println!("cargo:rerun-if-env-changed=WAKUWAKU_ASSET");
    if std::env::var_os("WAKUWAKU_ASSET").is_none() {
        let tree = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
        println!("cargo:rustc-env=WAKUWAKU_SOURCE_TREE={tree}");
    }
    words();
    tauri_build::build();
    // The examples that make Tauri windows (examples/embed_spike.rs) linked
    // with her exe's resource too: tauri-build gives it to the bins alone,
    // and without its manifest (Common Controls 6) Windows will not start
    // one (0xC0000139, an entry point of comctl32's not found).
    let resource = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("resource.lib");
    if resource.exists() {
        println!("cargo:rustc-link-arg-examples={}", resource.display());
    }
}

// The words the program says itself are written where the pages' are, in
// src/shared/i18n.js, and nowhere else (i18n.rs): the keys its source names
// (a key in quotes, or a prefix it builds keys from, format!("mood.{mood}")),
// with their Chinese and English, into OUT_DIR/words.rs, sorted by key.
fn words() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let js = manifest.join("..").join("src").join("shared").join("i18n.js");
    let src = manifest.join("src");
    println!("cargo:rerun-if-changed={}", js.display());
    println!("cargo:rerun-if-changed={}", src.display());
    let text = fs::read_to_string(&js).unwrap_or_else(|e| panic!("{}: {e}", js.display()));
    let table = strings_of(&text).unwrap_or_else(|e| panic!("src/shared/i18n.js: {e}"));
    let (zh, en) = (&table["zh"], &table["en"]);
    let (mut named, mut prefixes) = (BTreeSet::new(), BTreeSet::new());
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    for file in files {
        let code = fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        names_in(&code, &mut named, &mut prefixes);
    }
    let mut out = String::from("// Written by build.rs from src/shared/i18n.js: (key, Chinese, English), by key.\npub static WORDS: &[(&str, &str, &str)] = &[\n");
    for (key, english) in en {
        if named.contains(key) || prefixes.iter().any(|p: &String| key.starts_with(p.as_str())) {
            let chinese = zh.get(key).unwrap_or(english);
            out.push_str(&format!("    ({key:?}, {chinese:?}, {english:?}),\n"));
        }
    }
    out.push_str("];\n");
    let dir = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    fs::write(dir.join("words.rs"), out).expect("OUT_DIR/words.rs written");
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

// In a Rust file, each "a.b" in quotes (a key, if i18n.js has it: one the
// program only hands to the pages costs a line here, no more), and each
// format!("a.{...}") in a t( call, a prefix keys are built from.
fn names_in(code: &str, named: &mut BTreeSet<String>, prefixes: &mut BTreeSet<String>) {
    let is_key = |s: &str| s.contains('.') && s.starts_with(|c: char| c.is_ascii_lowercase()) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.');
    for (at, _) in code.match_indices('"') {
        let rest = &code[at + 1..];
        if let Some(end) = rest.find('"') {
            if is_key(&rest[..end]) {
                named.insert(rest[..end].to_string());
            }
        }
    }
    for (at, _) in code.match_indices("format!(\"") {
        // Only in a call of t( on its line: the keys built for the pages
        // to say (format!("mail.{…}")) are theirs.
        let line = &code[code[..at].rfind('\n').map_or(0, |n| n + 1)..];
        if !line[..line.find('\n').unwrap_or(line.len())].contains("t(") {
            continue;
        }
        let rest = &code[at + 9..];
        if let Some(end) = rest.find('{') {
            let prefix = &rest[..end];
            if prefix.ends_with('.') && is_key(&format!("{prefix}x")) {
                prefixes.insert(prefix.to_string());
            }
        }
    }
}

// i18n.js's STRINGS: { zh: { 'key': 'words', ... }, en: { ... } }, comments
// skipped, each string in ' or " with JS's escapes. Anything else in it is
// an error here, so the table stays one both sides read.
fn strings_of(js: &str) -> Result<BTreeMap<String, BTreeMap<String, String>>, String> {
    let start = js.find("const STRINGS = {").ok_or("no const STRINGS = {")? + "const STRINGS = ".len();
    let mut p = Tokens { chars: js[start..].chars().collect(), at: 0, first_line: js[..start].matches('\n').count() + 1 };
    let mut table = BTreeMap::new();
    p.expect('{')?;
    while !p.next_is('}')? {
        let lang = p.word()?;
        p.expect(':')?;
        p.expect('{')?;
        let mut words = BTreeMap::new();
        while !p.next_is('}')? {
            let key = p.string()?;
            p.expect(':')?;
            let value = p.string()?;
            if words.insert(key.clone(), value).is_some() {
                return Err(format!("{lang}: {key} twice"));
            }
            if !p.next_is('}')? {
                p.expect(',')?;
            }
        }
        p.expect('}')?;
        table.insert(lang, words);
        if !p.next_is('}')? {
            p.expect(',')?;
        }
    }
    for lang in ["zh", "en"] {
        if !table.contains_key(lang) {
            return Err(format!("no {lang}"));
        }
    }
    Ok(table)
}

struct Tokens {
    chars: Vec<char>,
    at: usize,
    // i18n.js's line the first char is on, for errors.
    first_line: usize,
}

impl Tokens {
    fn skip(&mut self) {
        loop {
            while self.chars.get(self.at).is_some_and(|c| c.is_whitespace()) {
                self.at += 1;
            }
            match (self.chars.get(self.at), self.chars.get(self.at + 1)) {
                (Some('/'), Some('/')) => {
                    while self.chars.get(self.at).is_some_and(|&c| c != '\n') {
                        self.at += 1;
                    }
                }
                (Some('/'), Some('*')) => {
                    self.at += 2;
                    while self.at < self.chars.len() && !(self.chars[self.at] == '*' && self.chars.get(self.at + 1) == Some(&'/')) {
                        self.at += 1;
                    }
                    self.at += 2;
                }
                _ => return,
            }
        }
    }

    fn line(&self) -> usize {
        self.first_line + self.chars[..self.at.min(self.chars.len())].iter().filter(|&&c| c == '\n').count()
    }

    fn next_is(&mut self, c: char) -> Result<bool, String> {
        self.skip();
        match self.chars.get(self.at) {
            Some(&got) => Ok(got == c),
            None => Err("the end, inside STRINGS".into()),
        }
    }

    fn expect(&mut self, c: char) -> Result<(), String> {
        if self.next_is(c)? {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("line {}: {c:?} expected, {:?} found", self.line(), self.chars[self.at]))
        }
    }

    fn word(&mut self) -> Result<String, String> {
        self.skip();
        let from = self.at;
        while self.chars.get(self.at).is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_') {
            self.at += 1;
        }
        if from == self.at {
            return Err(format!("line {}: a name expected", self.line()));
        }
        Ok(self.chars[from..self.at].iter().collect())
    }

    fn string(&mut self) -> Result<String, String> {
        self.skip();
        let quote = match self.chars.get(self.at) {
            Some(&q) if q == '\'' || q == '"' => q,
            _ => return Err(format!("line {}: a string in ' or \" expected", self.line())),
        };
        self.at += 1;
        let mut out = String::new();
        loop {
            let c = *self.chars.get(self.at).ok_or("the end, inside a string")?;
            self.at += 1;
            match c {
                _ if c == quote => return Ok(out),
                '\n' => return Err(format!("line {}: a string across lines", self.line())),
                '\\' => {
                    let e = *self.chars.get(self.at).ok_or("the end, inside a string")?;
                    self.at += 1;
                    match e {
                        'n' => out.push('\n'),
                        't' => out.push('\t'),
                        'u' => {
                            let hex: String = self.chars.get(self.at..self.at + 4).ok_or("a short \\u")?.iter().collect();
                            self.at += 4;
                            let code = u32::from_str_radix(&hex, 16).map_err(|_| format!("line {}: \\u{hex}", self.line()))?;
                            out.push(char::from_u32(code).ok_or(format!("line {}: \\u{hex}", self.line()))?);
                        }
                        other => out.push(other),
                    }
                }
                _ => out.push(c),
            }
        }
    }
}
