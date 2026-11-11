// What WebView2 is started with for her pages, and where it keeps its files
// (every window of hers is made with both).
use std::path::PathBuf;

use crate::{gpu, island, Shared};

// What WebView2 is started with: wry's own (its Office and PDF parts and
// SmartScreen off); her two pages in one renderer process, not one each
// (same origin, same show; the second was 90 MB and its own idle); the
// network service in its browser process, not a process of its own (#14;
// the storage service's process has no such switch any more: it stays);
// WebRTC's non-proxied UDP off, so a plugin cannot reach the network by
// WebRTC past its frame's Content-Security-Policy and past her proxy
// (the plugins review, finding 2; she uses WebRTC nowhere herself, and
// with no proxy set this leaves it no way out by UDP); and rendering on the
// graphics card that drives her display (gpu.rs, #13). WebView2 takes them
// once, for all her windows (another set refused): worked out as the first
// is made, her home's display then the one chosen (homeDisplay, #104) or
// the primary one; chosen anew while she runs, the card stays until she is
// next started. A self-update changes
// them across versions safely because the new exe makes its WebView2 only
// after the old one has ended (updates.rs wait_after_update, hand_over).
// WAKUWAKU_BROWSER_ARGS, more of them (to try one).
pub fn browser_args(sh: &Shared) -> &'static str {
    static ARGS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ARGS.get_or_init(|| {
        let mut args = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --enable-features=NetworkServiceInProcess2 --renderer-process-limit=1 --force-webrtc-ip-handling-policy=disable_non_proxied_udp".to_string();
        match island::display(sh).and_then(gpu::switch_for) {
            Some((switch, display)) => {
                sh.log(&format!("webview: rendering on the card driving {display} ({switch})"));
                args = format!("{args} {switch}");
            }
            None => sh.log("webview: its card left to Chromium (her display's card not told, or it renders nothing)"),
        }
        if let Ok(more) = std::env::var("WAKUWAKU_BROWSER_ARGS") {
            args = format!("{args} {more}");
        }
        args
    })
}

// Where WebView2 keeps its own files for her pages: in her data folder
// (portable, as the rest), not Windows' shared place for the app id. Two
// copies of her sharing that place with different browser arguments cannot
// both start (WebView2 refuses the second, 0x8007139F): an older build
// beside this one, as in a seamless swap, or a test copy.
pub fn webview_dir(sh: &Shared) -> PathBuf {
    sh.dir.join("webview")
}
