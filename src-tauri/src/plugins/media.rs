// What is playing on this computer, and its buttons: the song (or the
// video) in the app that has the sound (Spotify, Media Player, a browser,
// 网易云音乐, QQ 音乐), its artist and album, playing or paused, how far
// along; and previous, play or pause, next, pressed on her cards
// (plugins.rs act). Windows keeps all this for every app that plays, as the
// volume flyout shows it (the system media transport controls,
// Windows.Media.Control): she asks Windows for the session it counts as
// current, the one the flyout shows, and never the apps themselves. Windows
// tells her as it changes (another song, paused, played on), and she looks
// again at once (plugins::poke); between, every few seconds while something
// plays, for how far along it is, and seldom while nothing does.
//
// Its words are in her language (没有在播放, 暂停). On another system
// nothing plays here, and the Plugins page says so.
use std::time::Duration;

use serde_json::{json, Value};

use super::{Job, Now, Out, Wait};

const COLOR: &str = "#ff7ab6";
// How often it looks: while something plays (how far along it is), while
// it is paused, and with nothing playing; with nobody about, seldom. A
// widget stays past the longer wait.
const PLAYING: Wait = Wait { busy: Duration::from_secs(5), idle: Duration::from_secs(20) };
const PAUSED: Wait = Wait { busy: Duration::from_secs(15), idle: Duration::from_secs(60) };
const NOTHING: Wait = Wait { busy: Duration::from_secs(20), idle: Duration::from_secs(60) };
const TTL_PLAYING: u64 = 60;
const TTL: u64 = 180;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    Playing,
    Paused,
    #[default]
    Stopped,
}

// What the current session says of itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playing {
    // The app's id as Windows has it (its AppUserModelId: "Spotify.exe",
    // "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify", "Chrome").
    pub app: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub state: State,
    pub position_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub can_prev: bool,
    pub can_next: bool,
    pub can_toggle: bool,
}

// Windows' media sessions, or a test's: the current one read (None with
// nothing playing), a button pressed.
pub trait Player: Send {
    fn read(&mut self) -> Result<Option<Playing>, String>;
    fn press(&mut self, action: &str) -> Result<(), String>;
}

pub struct Media {
    lang: &'static str,
    // Opened at its first turn, not as she starts (sync makes every plugin
    // without asking anything of the system); Err: why there is none here.
    player: Option<Result<Box<dyn Player>, String>>,
    open: fn() -> Result<Box<dyn Player>, String>,
}

struct Words {
    none: &'static str,
    untitled: &'static str,
    paused: &'static str,
    stopped: &'static str,
}

const ZH: Words = Words { none: "没有在播放", untitled: "未知曲目", paused: "暂停", stopped: "停止" };
const EN: Words = Words { none: "Nothing playing", untitled: "Untitled", paused: "Paused", stopped: "Stopped" };

impl Media {
    pub fn new(lang: &'static str) -> Media {
        Media { lang, player: None, open: smtc::open }
    }

    // With a player given (a test's).
    #[cfg(test)]
    pub fn with(lang: &'static str, player: Box<dyn Player>) -> Media {
        Media { lang, player: Some(Ok(player)), open: || Err("none".into()) }
    }

    fn words(&self) -> &'static Words {
        if self.lang == "zh" {
            &ZH
        } else {
            &EN
        }
    }

    fn player(&mut self) -> Result<&mut dyn Player, String> {
        let open = self.open;
        match self.player.get_or_insert_with(open) {
            Ok(p) => Ok(p.as_mut()),
            Err(why) => Err(why.clone()),
        }
    }

    // Nothing playing: said so, with no buttons.
    fn none(&self) -> Value {
        json!({ "id": "media", "icon": "music", "color": COLOR, "label": self.words().none, "value": "—", "ttl": TTL, "detail": { "sizes": ["s", "m", "l"], "rows": [{ "kind": "state", "value": "none" }] } })
    }

    // What is playing: the title as the widget's name, the artist (else
    // the app) its value, how far along its second line and its ring; the
    // whole of it in rows for the cards, and its three buttons (one the
    // player has not, greyed).
    fn widget(&self, p: &Playing) -> Value {
        let w = self.words();
        let title = if p.title.trim().is_empty() { w.untitled } else { p.title.trim() };
        let app = app_name(&p.app);
        let artist = p.artist.trim();
        let value = if artist.is_empty() { app.as_str() } else { artist };
        let mut rows = vec![json!({ "kind": "track", "subject": title, "text": artist })];
        if !p.album.trim().is_empty() {
            rows.push(json!({ "kind": "album", "text": p.album.trim() }));
        }
        rows.push(json!({ "kind": "app", "name": app }));
        let state = match p.state {
            State::Playing => "playing",
            State::Paused => "paused",
            State::Stopped => "stopped",
        };
        rows.push(json!({ "kind": "state", "value": state }));
        let mut detail = json!({ "sizes": ["s", "m", "l"], "rows": rows });
        let times = match (p.position_ms, p.duration_ms) {
            (Some(at), Some(of)) if of > 0 => Some(format!("{} / {}", clock(at.min(of)), clock(of))),
            (Some(at), _) => Some(clock(at)),
            _ => None,
        };
        let extra = match (p.state, times) {
            (State::Playing, Some(t)) => t,
            (State::Playing, None) => String::new(),
            (State::Paused, Some(t)) => format!("{} · {t}", w.paused),
            (State::Paused, None) => w.paused.to_string(),
            (State::Stopped, _) => w.stopped.to_string(),
        };
        if !extra.is_empty() {
            detail["extra"] = json!(extra);
        }
        if let (Some(at), Some(of)) = (p.position_ms, p.duration_ms) {
            if of > 0 {
                detail["series"] = json!({ "points": [at.min(of) / 1000], "hi": of / 1000, "lo": 0, "unit": "s" });
            }
        }
        let toggle = if p.state == State::Playing { "pause" } else { "play" };
        detail["actions"] = json!([
            { "id": "prev", "icon": "prev", "off": !p.can_prev },
            { "id": "toggle", "icon": toggle, "off": !p.can_toggle },
            { "id": "next", "icon": "next", "off": !p.can_next },
        ]);
        let ttl = if p.state == State::Playing { TTL_PLAYING } else { TTL };
        json!({ "id": "media", "icon": "music", "color": COLOR, "label": title, "value": value, "ttl": ttl, "detail": detail })
    }
}

impl Job for Media {
    fn turn(&mut self, _now: Now, out: &mut dyn Out) -> Wait {
        let read = match self.player() {
            Ok(player) => player.read(),
            Err(why) => Err(why),
        };
        match read {
            Ok(Some(p)) => {
                let wait = match p.state {
                    State::Playing => PLAYING,
                    State::Paused => PAUSED,
                    State::Stopped => NOTHING,
                };
                out.post(self.widget(&p));
                wait
            }
            Ok(None) => {
                out.post(self.none());
                NOTHING
            }
            Err(why) => {
                out.say(why);
                out.post(self.none());
                NOTHING
            }
        }
    }

    // A button pressed: to the player; what it did, the turn after shows.
    fn act(&mut self, action: &str, _now: Now, out: &mut dyn Out) {
        let pressed = match self.player() {
            Ok(player) => player.press(action),
            Err(why) => Err(why),
        };
        if let Err(why) = pressed {
            out.say(why);
        }
    }
}

// A time as a player shows it: m:ss, or h:mm:ss past an hour.
fn clock(ms: u64) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

// The app's name from its id: a packaged app's entry ("…!Spotify"), else
// the app's own part of its family name ("Microsoft.ZuneMusic_…"); an
// unpackaged one's exe without ".exe", or the name it gave ("Chrome"). The
// well-known ones by the name people know them by.
pub fn app_name(aumid: &str) -> String {
    let id = aumid.trim();
    let name = match id.split_once('!') {
        Some((family, entry)) => {
            let app = family.split('_').next().unwrap_or(family);
            let app = app.rsplit('.').next().unwrap_or(app);
            let entry = entry.rsplit('.').next().unwrap_or(entry);
            if entry.is_empty() || entry.eq_ignore_ascii_case("app") {
                app
            } else {
                entry
            }
        }
        None => {
            let bare = id.rsplit(['\\', '/']).next().unwrap_or(id);
            bare.strip_suffix(".exe").or_else(|| bare.strip_suffix(".EXE")).unwrap_or(bare)
        }
    };
    let lower = name.to_ascii_lowercase();
    let known = match lower.as_str() {
        "spotify" | "spotifymusic" => "Spotify",
        "zunemusic" => "Media Player",
        "zunevideo" => "Films & TV",
        "applemusicwin" | "applemusic" => "Apple Music",
        "cloudmusic" => "网易云音乐",
        "qqmusic" => "QQ音乐",
        "kugou" => "酷狗音乐",
        "kwmusic" => "酷我音乐",
        "bilibili" => "哔哩哔哩",
        "chrome" => "Chrome",
        "msedge" => "Edge",
        "firefox" => "Firefox",
        "potplayermini64" | "potplayermini" | "potplayer64" | "potplayer" => "PotPlayer",
        "vlc" => "VLC",
        "mpv" => "mpv",
        "foobar2000" => "foobar2000",
        "musicbee" => "MusicBee",
        "itunes" => "iTunes",
        _ => "",
    };
    if known.is_empty() {
        name.to_string()
    } else {
        known.to_string()
    }
}

// How far along a song is now: where the player last said, plus the time
// since while it plays (Windows keeps the moment it said so; a player says
// so every few seconds, or on a seek); never past its end.
pub fn advanced(position_ms: u64, duration_ms: Option<u64>, since_ms: u64, playing: bool) -> u64 {
    // A moment too long ago (a clock set, a sleep) is not counted on.
    let since = if playing && since_ms < 86_400_000 { since_ms } else { 0 };
    let at = position_ms.saturating_add(since);
    duration_ms.filter(|d| *d > 0).map_or(at, |d| at.min(d))
}

// --- Windows' media sessions ----------------------------------------------------------------
#[cfg(windows)]
mod smtc {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use windows::Foundation::TypedEventHandler;
    use windows::Media::Control::{GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager, GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status};

    use super::{Player, Playing, State};

    // 1601 to 1970, in Windows' 100 ns ticks (a DateTime's UniversalTime).
    const EPOCH: i64 = 116_444_736_000_000_000;

    pub struct Smtc {
        manager: Manager,
        // Windows said the current session changed (its own thread): the
        // next read listens to the new one.
        changed: Arc<AtomicBool>,
        // The session listened to, and the three tokens of its events.
        heard: Option<(Session, [i64; 3])>,
    }

    fn words(e: windows::core::Error) -> String {
        let message = e.message();
        if message.trim().is_empty() {
            format!("{:?}", e.code())
        } else {
            message.trim().to_string()
        }
    }

    // Word from Windows (on a thread of its own): she looks again at once.
    fn poked() -> windows::core::Result<()> {
        super::super::poke("media");
        Ok(())
    }

    // Windows' sessions, and word of the current one changing.
    pub fn open() -> Result<Box<dyn Player>, String> {
        let manager = Manager::RequestAsync().map_err(words)?.join().map_err(words)?;
        let changed = Arc::new(AtomicBool::new(true));
        let flag = changed.clone();
        manager
            .CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
                flag.store(true, Ordering::Relaxed);
                poked()
            }))
            .map_err(words)?;
        Ok(Box::new(Smtc { manager, changed, heard: None }))
    }

    impl Smtc {
        // The current session, listened to for its changes (the song, its
        // state, how far along), the one before let go.
        fn session(&mut self) -> Result<Option<Session>, String> {
            // None: Windows answers with nothing where no app plays.
            let Ok(session) = self.manager.GetCurrentSession() else {
                self.unhear();
                return Ok(None);
            };
            let same = self.heard.as_ref().is_some_and(|(s, _)| s == &session);
            if !same || self.changed.swap(false, Ordering::Relaxed) {
                self.unhear();
                let tokens = [
                    session.MediaPropertiesChanged(&TypedEventHandler::new(|_, _| poked())).map_err(words)?,
                    session.PlaybackInfoChanged(&TypedEventHandler::new(|_, _| poked())).map_err(words)?,
                    session.TimelinePropertiesChanged(&TypedEventHandler::new(|_, _| poked())).map_err(words)?,
                ];
                self.heard = Some((session.clone(), tokens));
            }
            Ok(Some(session))
        }

        fn unhear(&mut self) {
            if let Some((s, [a, b, c])) = self.heard.take() {
                let _ = s.RemoveMediaPropertiesChanged(a);
                let _ = s.RemovePlaybackInfoChanged(b);
                let _ = s.RemoveTimelinePropertiesChanged(c);
            }
        }
    }

    impl Player for Smtc {
        fn read(&mut self) -> Result<Option<Playing>, String> {
            let Some(session) = self.session()? else { return Ok(None) };
            let info = session.GetPlaybackInfo().map_err(words)?;
            let status = info.PlaybackStatus().map_err(words)?;
            // Closed: the app is there with nothing in it.
            if status == Status::Closed {
                return Ok(None);
            }
            let state = match status {
                Status::Playing => State::Playing,
                Status::Paused => State::Paused,
                _ => State::Stopped,
            };
            let controls = info.Controls().map_err(words)?;
            let can_prev = controls.IsPreviousEnabled().unwrap_or(false);
            let can_next = controls.IsNextEnabled().unwrap_or(false);
            let can_toggle = controls.IsPlayPauseToggleEnabled().unwrap_or(false) || controls.IsPlayEnabled().unwrap_or(false) || controls.IsPauseEnabled().unwrap_or(false);
            let props = session.TryGetMediaPropertiesAsync().map_err(words)?.join().map_err(words)?;
            let title = props.Title().map(|s| s.to_string()).unwrap_or_default();
            let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
            let album = props.AlbumTitle().map(|s| s.to_string()).unwrap_or_default();
            // Nothing playing and nothing named: as good as none.
            if state == State::Stopped && title.trim().is_empty() {
                return Ok(None);
            }
            let app = session.SourceAppUserModelId().map(|s| s.to_string()).unwrap_or_default();
            let (mut position_ms, mut duration_ms) = (None, None);
            if let Ok(timeline) = session.GetTimelineProperties() {
                let ticks = |t: Result<windows::Foundation::TimeSpan, _>| t.ok().map(|t| t.Duration).filter(|d| *d >= 0).map(|d| (d / 10_000) as u64);
                let start = ticks(timeline.StartTime()).unwrap_or(0);
                duration_ms = ticks(timeline.EndTime()).map(|end| end.saturating_sub(start)).filter(|d| *d > 0);
                if let Some(at) = ticks(timeline.Position()) {
                    let said_at = timeline.LastUpdatedTime().map(|t| t.UniversalTime).unwrap_or(0);
                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as i64 / 100 + EPOCH).unwrap_or(said_at);
                    let since = ((now - said_at) / 10_000).max(0) as u64;
                    position_ms = Some(super::advanced(at.saturating_sub(start), duration_ms, since, state == State::Playing));
                }
            }
            Ok(Some(Playing { app, title, artist, album, state, position_ms, duration_ms, can_prev, can_next, can_toggle }))
        }

        fn press(&mut self, action: &str) -> Result<(), String> {
            let Some(session) = self.session()? else { return Ok(()) };
            let done = match action {
                "prev" => session.TrySkipPreviousAsync(),
                "next" => session.TrySkipNextAsync(),
                "toggle" => session.TryTogglePlayPauseAsync(),
                "play" => session.TryPlayAsync(),
                "pause" => session.TryPauseAsync(),
                _ => return Ok(()),
            };
            match done.map_err(words)?.join() {
                Ok(_) => Ok(()),
                Err(e) => Err(words(e)),
            }
        }
    }
}

#[cfg(not(windows))]
mod smtc {
    use super::Player;

    pub fn open() -> Result<Box<dyn Player>, String> {
        Err("Windows only: what is playing is read from Windows' media sessions".into())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::record;
    use super::*;

    // A player that answers as told, and remembers what was pressed.
    struct Fake {
        answers: Vec<Result<Option<Playing>, String>>,
        pressed: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        refuse: bool,
    }

    impl Player for Fake {
        fn read(&mut self) -> Result<Option<Playing>, String> {
            if self.answers.len() > 1 {
                self.answers.remove(0)
            } else {
                self.answers.first().cloned().unwrap_or(Ok(None))
            }
        }
        fn press(&mut self, action: &str) -> Result<(), String> {
            self.pressed.lock().unwrap().push(action.to_string());
            if self.refuse {
                Err("the player would not".into())
            } else {
                Ok(())
            }
        }
    }

    fn media(lang: &'static str, answers: Vec<Result<Option<Playing>, String>>) -> (Media, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let pressed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        (Media::with(lang, Box::new(Fake { answers, pressed: pressed.clone(), refuse: false })), pressed)
    }

    fn song() -> Playing {
        Playing { app: "Spotify.exe".into(), title: "Yellow".into(), artist: "Coldplay".into(), album: "Parachutes".into(), state: State::Playing, position_ms: Some(62_000), duration_ms: Some(269_000), can_prev: true, can_next: true, can_toggle: true }
    }

    // Playing: the song as the widget's name, its artist the value, how far
    // along as its line and its ring, the app and the state in its rows,
    // three buttons with pause in the middle; looked at every 5 seconds.
    #[test]
    fn a_song_playing_reads_as_the_player_has_it() {
        let (mut m, _) = media("zh", vec![Ok(Some(song()))]);
        let (posts, said, wait) = record(&mut m, 0);
        assert_eq!((posts.len(), said, wait), (1, None, PLAYING));
        let w = &posts[0];
        assert_eq!((w["id"].as_str(), w["label"].as_str(), w["value"].as_str(), w["ttl"].as_u64()), (Some("media"), Some("Yellow"), Some("Coldplay"), Some(60)));
        let d = &w["detail"];
        assert_eq!(d["extra"], "1:02 / 4:29");
        assert_eq!(d["series"], json!({ "points": [62], "hi": 269, "lo": 0, "unit": "s" }));
        assert_eq!(d["rows"], json!([{ "kind": "track", "subject": "Yellow", "text": "Coldplay" }, { "kind": "album", "text": "Parachutes" }, { "kind": "app", "name": "Spotify" }, { "kind": "state", "value": "playing" }]));
        assert_eq!(d["actions"], json!([{ "id": "prev", "icon": "prev", "off": false }, { "id": "toggle", "icon": "pause", "off": false }, { "id": "next", "icon": "next", "off": false }]));
    }

    // Paused: 暂停 before the times, play in the middle, looked at less
    // often; a button the player has not is off. No artist: the app stands
    // in. No title: 未知曲目. Nothing playing: said so, no buttons, seldom
    // looked at. Stopped with nothing known: 停止.
    #[test]
    fn paused_nothing_and_what_is_missing() {
        let mut paused = song();
        paused.state = State::Paused;
        paused.can_prev = false;
        paused.artist = String::new();
        let (mut m, _) = media("zh", vec![Ok(Some(paused))]);
        let (posts, _, wait) = record(&mut m, 0);
        assert_eq!(wait, PAUSED);
        assert_eq!((posts[0]["value"].as_str(), posts[0]["ttl"].as_u64()), (Some("Spotify"), Some(180)));
        assert_eq!(posts[0]["detail"]["extra"], "暂停 · 1:02 / 4:29");
        assert_eq!(posts[0]["detail"]["actions"][0]["off"], true);
        assert_eq!(posts[0]["detail"]["actions"][1]["icon"], "play");
        let (mut m, _) = media("en", vec![Ok(Some(Playing { title: "  ".into(), state: State::Paused, ..Default::default() }))]);
        let (posts, _, _) = record(&mut m, 0);
        assert_eq!((posts[0]["label"].as_str(), posts[0]["detail"]["extra"].as_str()), (Some("Untitled"), Some("Paused")));
        assert!(posts[0]["detail"].get("series").is_none());
        let (mut m, _) = media("en", vec![Ok(None)]);
        let (posts, said, wait) = record(&mut m, 0);
        assert_eq!((said, wait), (None, NOTHING));
        assert_eq!((posts[0]["label"].as_str(), posts[0]["value"].as_str()), (Some("Nothing playing"), Some("—")));
        assert!(posts[0]["detail"].get("actions").is_none());
        assert_eq!(posts[0]["detail"]["rows"][0]["value"], "none");
        let (mut m, _) = media("zh", vec![Ok(Some(Playing { title: "x".into(), state: State::Stopped, ..Default::default() }))]);
        assert_eq!(record(&mut m, 0).0[0]["detail"]["extra"], "停止");
    }

    // The player failing is said on the Plugins page, and nothing playing
    // shown; where there is no player at all (another system), the same.
    #[test]
    fn trouble_is_said_and_nothing_shown() {
        let (mut m, _) = media("zh", vec![Err("no sessions".into())]);
        let (posts, said, wait) = record(&mut m, 0);
        assert_eq!((posts[0]["label"].as_str(), said.as_deref(), wait), (Some("没有在播放"), Some("no sessions"), NOTHING));
        let mut none = Media { lang: "en", player: None, open: || Err("Windows only".into()) };
        let (posts, said, _) = record(&mut none, 0);
        assert_eq!((posts.len(), said.as_deref()), (1, Some("Windows only")));
    }

    // A button pressed goes to the player as named; the player declining
    // is said.
    #[test]
    fn buttons_go_to_the_player() {
        let (mut m, pressed) = media("zh", vec![Ok(Some(song()))]);
        let now = Now { ms: 0, local: 0 };
        let mut out = super::super::tests::Record::default();
        m.act("toggle", now, &mut out);
        m.act("next", now, &mut out);
        assert_eq!(*pressed.lock().unwrap(), ["toggle", "next"]);
        assert!(out.said.is_none());
        let mut m = Media::with("zh", Box::new(Fake { answers: vec![], pressed: pressed.clone(), refuse: true }));
        m.act("prev", now, &mut out);
        assert_eq!(out.said.as_deref(), Some("the player would not"));
    }

    // Apps by the names people know: a packaged app's entry or family, an
    // exe without its ".exe", the well-known ones as they are called.
    #[test]
    fn apps_are_named_as_people_know_them() {
        assert_eq!(app_name("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"), "Spotify");
        assert_eq!(app_name("Spotify.exe"), "Spotify");
        assert_eq!(app_name("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"), "Media Player");
        assert_eq!(app_name("AppleInc.AppleMusicWin_nzyj5cx40ttqa!App"), "Apple Music");
        assert_eq!(app_name("cloudmusic.exe"), "网易云音乐");
        assert_eq!(app_name("QQMusic.exe"), "QQ音乐");
        assert_eq!(app_name("Chrome"), "Chrome");
        assert_eq!(app_name("MSEdge"), "Edge");
        assert_eq!(app_name("PotPlayerMini64.exe"), "PotPlayer");
        assert_eq!(app_name("C:\\Tools\\MyPlayer.exe"), "MyPlayer");
        assert_eq!(app_name("Some.Vendor.Thing_abc!Thing.Main"), "Main");
        assert_eq!(app_name(""), "");
    }

    // Windows' own sessions, on this machine (what is playing, if
    // anything): by hand, cargo test -- --ignored smtc.
    #[test]
    #[ignore]
    #[cfg(windows)]
    fn smtc_answers_on_this_machine() {
        let mut player = super::smtc::open().expect("the session manager");
        let read = player.read();
        eprintln!("smtc: {read:?}");
        assert!(read.is_ok());
        let read_again = player.read();
        assert_eq!(read.map(|p| p.map(|p| p.title)), read_again.map(|p| p.map(|p| p.title)));
    }

    // Times as a player shows them; how far along, counted on while it plays.
    #[test]
    fn times_and_how_far_along() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(62_000), "1:02");
        assert_eq!(clock(3_725_000), "1:02:05");
        assert_eq!(advanced(60_000, Some(200_000), 5_000, true), 65_000);
        assert_eq!(advanced(60_000, Some(200_000), 5_000, false), 60_000, "paused: where it was");
        assert_eq!(advanced(199_000, Some(200_000), 5_000, true), 200_000, "never past the end");
        assert_eq!(advanced(60_000, None, 5_000, true), 65_000);
        assert_eq!(advanced(60_000, Some(200_000), 90_000_000, true), 60_000, "a moment too long ago");
    }
}
