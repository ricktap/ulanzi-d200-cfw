//! Microsoft Teams.
//!
//! Meeting controls use the *third-party app API* of new Teams: a WebSocket on
//! 127.0.0.1:8124 (Teams → Settings → Privacy → Manage API → enable). The first
//! command sent during a meeting makes Teams ask to pair; it then hands out a
//! token that is stored and reused.
//!
//! Presence (available / away ...) is not in that API; it goes through
//! Microsoft Graph with a device-code sign-in (Presence.ReadWrite).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ------------------------------------------------------------------ local meeting API

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Meeting {
    pub in_meeting: bool,
    pub muted: bool,
    pub video_on: bool,
    pub hand_raised: bool,
    pub blurred: bool,
    pub recording: bool,
    pub sharing: bool,
    pub can_toggle_mute: bool,
    pub can_toggle_video: bool,
}

pub enum TeamsEvent {
    /// Connected (or not) to Teams; `paired` once a token exists.
    Link { connected: bool, paired: bool, error: Option<String> },
    Meeting(Meeting),
    Token(String),
}

/// Parse a message from Teams. Public for tests.
pub fn parse_message(v: &Value) -> Option<TeamsEvent> {
    if let Some(t) = v.get("tokenRefresh").and_then(Value::as_str) {
        return Some(TeamsEvent::Token(t.to_string()));
    }
    let mu = v.get("meetingUpdate")?;
    let st = mu.get("meetingState").cloned().unwrap_or(Value::Null);
    let perm = mu.get("meetingPermissions").cloned().unwrap_or(Value::Null);
    let b = |o: &Value, k: &str| o.get(k).and_then(Value::as_bool).unwrap_or(false);
    Some(TeamsEvent::Meeting(Meeting {
        in_meeting: b(&st, "isInMeeting"),
        muted: b(&st, "isMuted"),
        video_on: b(&st, "isVideoOn"),
        hand_raised: b(&st, "isHandRaised"),
        blurred: b(&st, "isBackgroundBlurred"),
        recording: b(&st, "isRecordingOn"),
        sharing: b(&st, "isSharing"),
        can_toggle_mute: b(&perm, "canToggleMute"),
        can_toggle_video: b(&perm, "canToggleVideo"),
    }))
}

pub struct TeamsLink {
    cmds: Sender<String>,
    stop: Arc<AtomicBool>,
}

pub const ACTIONS: [&str; 6] = ["toggle-mute", "toggle-video", "toggle-hand", "toggle-background-blur", "leave-call", "toggle-ui-chat"];

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl TeamsLink {
    /// Keep a connection to Teams in the background. `port` is 8124 normally.
    pub fn start(port: u16, token: Option<String>, on: impl Fn(TeamsEvent) + Send + 'static) -> TeamsLink {
        let (tx, rx) = mpsc::channel::<String>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        std::thread::spawn(move || {
            let mut token = token;
            let mut req = 0u64;
            let mut last_err: Option<String> = None;
            while !stop2.load(Ordering::SeqCst) {
                match session(port, &mut token, &rx, &stop2, &on, &mut req) {
                    Ok(()) => {}
                    Err(e) => {
                        if last_err.as_deref() != Some(e.as_str()) {
                            on(TeamsEvent::Link { connected: false, paired: token.is_some(), error: Some(e.clone()) });
                            last_err = Some(e);
                        }
                    }
                }
                // Drop commands that arrived while disconnected; retry later.
                while rx.try_recv().is_ok() {}
                for _ in 0..50 {
                    if stop2.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });
        TeamsLink { cmds: tx, stop }
    }

    /// Send an action such as "toggle-mute".
    pub fn send(&self, action: &str) {
        let _ = self.cmds.send(action.to_string());
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn session(
    port: u16,
    token: &mut Option<String>,
    cmds: &Receiver<String>,
    stop: &AtomicBool,
    on: &impl Fn(TeamsEvent),
    req: &mut u64,
) -> Result<(), String> {
    let mut url = format!(
        "ws://127.0.0.1:{port}?protocol-version=2.0.0&manufacturer={}&device={}&app={}&app-version={}",
        url_encode("d200fw"),
        url_encode("Ulanzi D200"),
        url_encode("D200 Deck"),
        env!("CARGO_PKG_VERSION")
    );
    if let Some(t) = token.as_ref() {
        url += &format!("&token={}", url_encode(t));
    }
    let stream = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_secs(2))
        .map_err(|_| "Teams is not running, or its third-party API is off".to_string())?;
    let (mut ws, _) = tungstenite::client(url.as_str(), stream).map_err(|e| format!("Teams API: {e}"))?;
    ws.get_ref().set_read_timeout(Some(Duration::from_millis(150))).ok();
    on(TeamsEvent::Link { connected: true, paired: token.is_some(), error: None });
    let mut last_ping = Instant::now();
    loop {
        if stop.load(Ordering::SeqCst) {
            let _ = ws.close(None);
            return Ok(());
        }
        loop {
            match cmds.try_recv() {
                Ok(action) => {
                    *req += 1;
                    let (action, params) = match action.as_str() {
                        "toggle-ui-chat" => ("toggle-ui", json!({ "type": "chat" })),
                        a => (a, json!({})),
                    };
                    let msg = json!({ "action": action, "parameters": params, "requestId": *req }).to_string();
                    ws.send(tungstenite::Message::text(msg)).map_err(|e| e.to_string())?;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        if last_ping.elapsed() > Duration::from_secs(20) {
            last_ping = Instant::now();
            ws.send(tungstenite::Message::Ping(Vec::new().into())).map_err(|e| e.to_string())?;
        }
        match ws.read() {
            Ok(tungstenite::Message::Text(t)) => {
                if let Ok(v) = serde_json::from_str::<Value>(&t) {
                    match parse_message(&v) {
                        Some(TeamsEvent::Token(t)) => {
                            *token = Some(t.clone());
                            on(TeamsEvent::Token(t));
                            on(TeamsEvent::Link { connected: true, paired: true, error: None });
                        }
                        Some(ev) => on(ev),
                        None => {}
                    }
                }
            }
            Ok(tungstenite::Message::Close(_)) => return Err("Teams closed the connection".into()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => return Err(format!("Teams API: {e}")),
        }
    }
}

// ------------------------------------------------------------------ Graph presence

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tokens {
    pub refresh_token: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub expires_at: u64,
    #[serde(default)]
    pub account: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Presence {
    /// Available, AvailableIdle, Away, BeRightBack, Busy, BusyIdle, DoNotDisturb, Offline, PresenceUnknown
    pub availability: String,
    pub activity: String,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn form(url: &str, fields: &[(&str, &str)]) -> Result<Value, String> {
    let r = ureq::post(url).timeout(Duration::from_secs(15)).send_form(fields);
    match r {
        Ok(resp) => resp.into_json::<Value>().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(_, resp)) => resp.into_json::<Value>().map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval: u64,
    pub expires_in: u64,
}

/// Presence for the Teams status key; mail and calendar (sender/subject and
/// meeting times only, no bodies) for the wide display's cards.
const SCOPE: &str = "Presence.ReadWrite Mail.ReadBasic Calendars.Read User.Read offline_access";

fn authority(tenant: &str) -> String {
    let t = tenant.trim();
    format!("https://login.microsoftonline.com/{}/oauth2/v2.0", if t.is_empty() { "organizations" } else { t })
}

pub fn start_device_code(client_id: &str, tenant: &str) -> Result<DeviceCode, String> {
    if client_id.trim().is_empty() {
        return Err("Enter the client ID of your Entra app registration first.".into());
    }
    let v = form(&format!("{}/devicecode", authority(tenant)), &[("client_id", client_id.trim()), ("scope", SCOPE)])?;
    if let Some(e) = v.get("error_description").and_then(Value::as_str) {
        return Err(e.lines().next().unwrap_or(e).to_string());
    }
    Ok(DeviceCode {
        user_code: v["user_code"].as_str().unwrap_or_default().to_string(),
        verification_uri: v["verification_uri"].as_str().unwrap_or("https://microsoft.com/devicelogin").to_string(),
        device_code: v["device_code"].as_str().unwrap_or_default().to_string(),
        interval: v["interval"].as_u64().unwrap_or(5),
        expires_in: v["expires_in"].as_u64().unwrap_or(900),
    })
}

fn tokens_from(v: &Value, old_refresh: Option<&str>) -> Result<Tokens, String> {
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        return Err(e.to_string());
    }
    let access = v["access_token"].as_str().ok_or("no access token")?.to_string();
    Ok(Tokens {
        refresh_token: v["refresh_token"].as_str().or(old_refresh).unwrap_or_default().to_string(),
        access_token: access,
        expires_at: now() + v["expires_in"].as_u64().unwrap_or(3600).saturating_sub(120),
        account: String::new(),
    })
}

/// Block until the user finished signing in (or it expired). Call on a thread.
pub fn wait_device_code(client_id: &str, tenant: &str, dc: &DeviceCode) -> Result<Tokens, String> {
    let deadline = Instant::now() + Duration::from_secs(dc.expires_in);
    let mut interval = dc.interval.max(2);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_secs(interval));
        let v = form(
            &format!("{}/token", authority(tenant)),
            &[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id.trim()),
                ("device_code", &dc.device_code),
            ],
        )?;
        match v.get("error").and_then(Value::as_str) {
            Some("authorization_pending") => continue,
            Some("slow_down") => interval += 5,
            Some(_) => {
                return Err(v["error_description"].as_str().unwrap_or("sign-in failed").lines().next().unwrap_or("").to_string())
            }
            None => {
                let mut t = tokens_from(&v, None)?;
                t.account = me(&t.access_token).unwrap_or_default();
                return Ok(t);
            }
        }
    }
    Err("the sign-in code expired".into())
}

fn refresh(client_id: &str, tenant: &str, t: &Tokens) -> Result<Tokens, String> {
    let v = form(
        &format!("{}/token", authority(tenant)),
        &[("grant_type", "refresh_token"), ("client_id", client_id.trim()), ("refresh_token", &t.refresh_token), ("scope", SCOPE)],
    )?;
    let mut n = tokens_from(&v, Some(&t.refresh_token))?;
    n.account = t.account.clone();
    Ok(n)
}

/// A valid access token, refreshing when needed. Returns updated tokens if they changed.
pub fn access(client_id: &str, tenant: &str, t: &Tokens) -> Result<(String, Option<Tokens>), String> {
    if !t.access_token.is_empty() && now() < t.expires_at {
        return Ok((t.access_token.clone(), None));
    }
    let n = refresh(client_id, tenant, t)?;
    Ok((n.access_token.clone(), Some(n)))
}

fn graph_get(token: &str, path: &str) -> Result<Value, String> {
    match ureq::get(&format!("https://graph.microsoft.com/v1.0{path}"))
        .set("Authorization", &format!("Bearer {token}"))
        .set("Prefer", "outlook.timezone=\"UTC\"")
        .timeout(Duration::from_secs(10))
        .call()
    {
        Ok(r) => r.into_json::<Value>().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(code, r)) => Err(format!("Graph {code}: {}", r.into_string().unwrap_or_default())),
        Err(e) => Err(e.to_string()),
    }
}

fn graph_post(token: &str, path: &str, body: Value) -> Result<(), String> {
    match ureq::post(&format!("https://graph.microsoft.com/v1.0{path}"))
        .set("Authorization", &format!("Bearer {token}"))
        .timeout(Duration::from_secs(10))
        .send_json(body)
    {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(code, r)) => Err(format!("Graph {code}: {}", r.into_string().unwrap_or_default())),
        Err(e) => Err(e.to_string()),
    }
}

fn me(token: &str) -> Result<String, String> {
    let v = graph_get(token, "/me?$select=userPrincipalName")?;
    Ok(v["userPrincipalName"].as_str().unwrap_or_default().to_string())
}

pub fn get_presence(token: &str) -> Result<Presence, String> {
    let v = graph_get(token, "/me/presence")?;
    Ok(Presence {
        availability: v["availability"].as_str().unwrap_or("PresenceUnknown").to_string(),
        activity: v["activity"].as_str().unwrap_or("").to_string(),
    })
}

/// Set a preferred presence ("Away", "BeRightBack", "Busy", "DoNotDisturb",
/// "Offline", "Available") for 8 hours, or `None` to go back to automatic.
pub fn set_presence(token: &str, availability: Option<&str>) -> Result<(), String> {
    match availability {
        None => graph_post(token, "/me/presence/clearUserPreferredPresence", json!({})),
        Some(a) => {
            let activity = match a {
                "Offline" => "OffWork",
                other => other,
            };
            graph_post(
                token,
                "/me/presence/setUserPreferredPresence",
                json!({ "availability": a, "activity": activity, "expirationDuration": "PT8H" }),
            )
        }
    }
}

// ------------------------------------------------------------------ mail & calendar

#[derive(Debug, Clone, PartialEq)]
pub struct MailMsg {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mail {
    pub unread: u32,
    pub latest: Option<MailMsg>,
}

fn forbidden(e: &str) -> String {
    if e.contains("Graph 403") || e.contains("Graph 401") {
        "sign in again under Settings → Teams to allow mail and calendar".into()
    } else {
        e.to_string()
    }
}

/// Unread count of the inbox and the newest unread message.
pub fn get_mail(token: &str) -> Result<Mail, String> {
    let inbox = graph_get(token, "/me/mailFolders/inbox?$select=unreadItemCount").map_err(|e| forbidden(&e))?;
    let unread = inbox["unreadItemCount"].as_u64().unwrap_or(0) as u32;
    let latest = if unread > 0 {
        // $orderby properties must also appear in $filter.
        let q = "/me/mailFolders/inbox/messages?$filter=receivedDateTime%20ge%201900-01-01T00:00:00Z%20and%20isRead%20eq%20false\
                 &$orderby=receivedDateTime%20desc&$top=1&$select=id,subject,from,webLink";
        let v = graph_get(token, q).map_err(|e| forbidden(&e))?;
        v["value"].get(0).map(|m| MailMsg {
            id: m["id"].as_str().unwrap_or_default().to_string(),
            from: m["from"]["emailAddress"]["name"].as_str().or(m["from"]["emailAddress"]["address"].as_str()).unwrap_or("").to_string(),
            subject: m["subject"].as_str().unwrap_or("").to_string(),
            link: m["webLink"].as_str().map(str::to_string),
        })
    } else {
        None
    };
    Ok(Mail { unread, latest })
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalEvent {
    pub id: String,
    pub subject: String,
    /// Unix seconds.
    pub start: i64,
    pub end: i64,
    pub join: Option<String>,
    pub teams: bool,
}

fn utc(s: &str) -> Option<i64> {
    let s = s.trim_end_matches('Z');
    let s = s.split('.').next()?;
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").ok().map(|d| d.and_utc().timestamp())
}

/// Meetings from an hour ago to 12 hours ahead (no all-day or cancelled ones).
pub fn get_events(token: &str) -> Result<Vec<CalEvent>, String> {
    let now = chrono::Utc::now();
    let fmt = |d: chrono::DateTime<chrono::Utc>| d.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let q = format!(
        "/me/calendarView?startDateTime={}&endDateTime={}&$orderby=start/dateTime&$top=20\
         &$select=id,subject,start,end,isCancelled,isAllDay,onlineMeeting,onlineMeetingProvider,showAs",
        fmt(now - chrono::Duration::hours(1)),
        fmt(now + chrono::Duration::hours(12))
    );
    let v = graph_get(token, &q).map_err(|e| forbidden(&e))?;
    Ok(v["value"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|e| !e["isCancelled"].as_bool().unwrap_or(false) && !e["isAllDay"].as_bool().unwrap_or(false))
        .filter(|e| e["showAs"].as_str() != Some("free"))
        .filter_map(|e| {
            Some(CalEvent {
                id: e["id"].as_str()?.to_string(),
                subject: e["subject"].as_str().unwrap_or("(no title)").to_string(),
                start: utc(e["start"]["dateTime"].as_str()?)?,
                end: utc(e["end"]["dateTime"].as_str()?)?,
                join: e["onlineMeeting"]["joinUrl"].as_str().map(str::to_string),
                teams: e["onlineMeetingProvider"].as_str() == Some("teamsForBusiness"),
            })
        })
        .collect())
}

/// Short label and colour for an availability.
pub fn presence_style(a: &str) -> (&'static str, [u8; 3]) {
    match a {
        "Available" | "AvailableIdle" => ("Available", [34, 197, 94]),
        "Busy" | "BusyIdle" => ("Busy", [239, 68, 68]),
        "DoNotDisturb" => ("Do not disturb", [220, 38, 38]),
        "Away" | "BeRightBack" => (if a == "Away" { "Away" } else { "Be right back" }, [234, 179, 8]),
        "Offline" => ("Offline", [107, 114, 128]),
        _ => ("Unknown", [107, 114, 128]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_times() {
        assert_eq!(utc("2026-11-10T11:00:00.0000000"), Some(1794308400));
        assert_eq!(utc("2026-11-10T11:00:00Z"), Some(1794308400));
        assert!(utc("nope").is_none());
    }

    #[test]
    fn parses_meeting_update() {
        let v: Value = serde_json::from_str(
            r#"{"meetingUpdate":{"meetingState":{"isMuted":true,"isVideoOn":false,"isInMeeting":true,"isHandRaised":false},
                "meetingPermissions":{"canToggleMute":true,"canToggleVideo":true}}}"#,
        )
        .unwrap();
        match parse_message(&v) {
            Some(TeamsEvent::Meeting(m)) => {
                assert!(m.in_meeting && m.muted && !m.video_on && m.can_toggle_mute);
            }
            _ => panic!("expected meeting"),
        }
        let t: Value = serde_json::from_str(r#"{"tokenRefresh":"abc"}"#).unwrap();
        assert!(matches!(parse_message(&t), Some(TeamsEvent::Token(s)) if s == "abc"));
        assert!(parse_message(&json!({"response":"Success","requestId":1})).is_none());
    }
}

#[cfg(test)]
mod live {
    /// Network: `cargo test -- --ignored graph_error_live`
    #[test]
    #[ignore]
    fn graph_error_live() {
        let e = super::start_device_code("00000000-0000-0000-0000-000000000001", "organizations").err().unwrap();
        println!("error: {e}");
        assert!(!e.is_empty());
        assert!(super::start_device_code("", "organizations").is_err());
    }
}
