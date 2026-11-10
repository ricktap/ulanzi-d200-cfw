//! Now playing / transport for Spotify and Apple Music (macOS, AppleScript).
//!
//! macOS 15.4+ no longer lets third-party apps read the system-wide "Now
//! Playing" info, so this talks to the two players directly. Neither app is
//! launched by polling (`application "X" is running` is checked first).

use std::io::Read;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub struct NowPlaying {
    pub player: String, // "Spotify" | "Music"
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub position: f64,
    pub duration: f64,
    /// Spotify: artwork URL. Music: empty (exported separately).
    pub art_url: String,
}

impl NowPlaying {
    /// Identifies the track, for artwork caching.
    pub fn track_key(&self) -> String {
        format!("{}|{}|{}|{}", self.player, self.title, self.artist, self.album)
    }
}

const POLL_SCRIPT: &str = r#"
set sp to ""
if application "Spotify" is running then
  try
    tell application "Spotify"
      set sp to "Spotify" & tab & (player state as text) & tab & (name of current track) & tab & (artist of current track) & tab & (album of current track) & tab & (player position as text) & tab & ((duration of current track) / 1000 as text) & tab & (artwork url of current track)
    end tell
  end try
  if sp contains (tab & "playing" & tab) then return sp
end if
if application "Music" is running then
  try
    tell application "Music"
      if player state is not stopped then
        set mu to "Music" & tab & (player state as text) & tab & (name of current track) & tab & (artist of current track) & tab & (album of current track) & tab & (player position as text) & tab & (duration of current track as text) & tab & ""
        if mu contains (tab & "playing" & tab) or sp is "" then return mu
      end if
    end tell
  end try
end if
return sp
"#;

fn num(s: &str) -> f64 {
    // AppleScript formats numbers with the user's locale ("12,5" in German).
    s.trim().replace(',', ".").parse().unwrap_or(0.0)
}

/// Parse the tab-separated line from POLL_SCRIPT.
pub fn parse(line: &str) -> Option<NowPlaying> {
    let f: Vec<&str> = line.trim_end_matches(['\n', '\r']).split('\t').collect();
    if f.len() < 7 || f[0].is_empty() {
        return None;
    }
    let state = f[1].trim();
    if state == "stopped" {
        return None;
    }
    Some(NowPlaying {
        player: f[0].to_string(),
        playing: state == "playing",
        title: f[2].to_string(),
        artist: f[3].to_string(),
        album: f[4].to_string(),
        position: num(f[5]),
        duration: num(f[6]),
        art_url: f.get(7).map(|s| s.trim().to_string()).unwrap_or_default(),
    })
}

fn osascript(script: &str) -> Option<String> {
    let out = Command::new("osascript").args(["-e", script]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

/// Whether there is a player to poll: macOS, or the test seam on any OS.
pub fn available() -> bool {
    cfg!(target_os = "macos") || std::env::var_os("D200_MEDIA_FILE").is_some()
}

/// Current track of Spotify or Music. `None`: nothing playing or paused, or not macOS.
pub fn poll() -> Option<NowPlaying> {
    // Test seam: read the player line from a file instead of asking the players.
    if let Some(f) = std::env::var_os("D200_MEDIA_FILE") {
        return parse(&std::fs::read_to_string(f).ok()?);
    }
    if !cfg!(target_os = "macos") {
        return None;
    }
    parse(&osascript(POLL_SCRIPT)?)
}

/// "playpause", "play", "pause", "next", "previous" for `player` (or the
/// first running one; Music is started if nothing runs, like the play key).
pub fn command(player: Option<&str>, cmd: &str) -> Result<(), String> {
    let verb = match cmd {
        "playpause" | "toggle" => "playpause",
        "play" => "play",
        "pause" => "pause",
        "next" => "next track",
        "previous" | "prev" => "previous track",
        other => return Err(format!("unknown media command '{other}'")),
    };
    if let Some(f) = std::env::var_os("D200_MEDIA_FILE") {
        // Test seam: record the command next to the fake player file.
        let _ = std::fs::write(std::path::PathBuf::from(f).with_extension("cmd"), cmd);
        return Ok(());
    }
    let target = match player {
        Some(p) => p.to_string(),
        None => {
            let running = |app: &str| osascript(&format!("application \"{app}\" is running")).is_some_and(|s| s.trim() == "true");
            if running("Spotify") {
                "Spotify".into()
            } else {
                "Music".into()
            }
        }
    };
    osascript(&format!("tell application \"{target}\" to {verb}"))
        .map(|_| ())
        .ok_or_else(|| format!("could not control {target} (allow D200 Deck under Privacy & Security → Automation)"))
}

/// Album art bytes: Spotify from its URL, Music exported through AppleScript.
pub fn artwork(np: &NowPlaying) -> Option<Vec<u8>> {
    if np.player == "Spotify" && np.art_url.starts_with("http") {
        let resp = ureq::get(&np.art_url).timeout(std::time::Duration::from_secs(8)).call().ok()?;
        let mut buf = Vec::new();
        resp.into_reader().take(5_000_000).read_to_end(&mut buf).ok()?;
        return Some(buf);
    }
    if np.player == "Music" {
        let path = std::env::temp_dir().join("d200deck-artwork.bin");
        let p = path.display().to_string().replace('"', "");
        let script = format!(
            r#"tell application "Music" to set d to data of artwork 1 of current track
set f to open for access (POSIX file "{p}") with write permission
set eof f to 0
write d to f
close access f"#
        );
        osascript(&script)?;
        let data = std::fs::read(&path).ok();
        let _ = std::fs::remove_file(&path);
        return data;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_player_lines() {
        let np = parse("Spotify\tplaying\tSong A\tArtist\tAlbum\t12,5\t200.0\thttps://i.scdn.co/image/x\n").unwrap();
        assert!(np.playing);
        assert_eq!(np.position, 12.5);
        assert_eq!(np.duration, 200.0);
        assert_eq!(np.art_url, "https://i.scdn.co/image/x");
        let np = parse("Music\tpaused\tT\tA\tB\t3\t180\t").unwrap();
        assert!(!np.playing);
        assert_eq!(np.player, "Music");
        assert!(parse("").is_none());
        assert!(parse("Spotify\tstopped\t\t\t\t0\t0\t").is_none());
    }
}
