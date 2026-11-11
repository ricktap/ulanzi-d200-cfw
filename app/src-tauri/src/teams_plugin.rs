//! com.deckling.teams: Microsoft Teams meetings (local API) and Microsoft 365
//! (presence, mail, calendar through Graph). `teams.rs` holds the protocol code.
//!
//! Everything the deck shows goes out as signals; the core's cards and the built-in
//! widgets read them back with the readers at the bottom of this file.

use crate::apps::TeamsLinkState;
use crate::config::{AppKind, Config, TeamsCfg};
use crate::plugins::{Host, Plugin, PluginMsg};
use crate::teams::{self, CalEvent, Mail, MailMsg, Meeting, Presence, TeamsEvent, TeamsLink, Tokens};
use crate::bind::Signals;
use serde_json::{json, Map, Value};
use std::time::Duration;

pub const ID: &str = "com.deckling.teams";
/// How many upcoming events go out as `calendar.<n>.*`.
const EVENTS: usize = 10;

/// Messages from the plugin's threads, and the sign-in from the UI.
pub enum TeamsMsg {
    Link(u64, TeamsEvent),
    Presence(Result<(Presence, Option<Tokens>), String>),
    Mail(Result<(Mail, Option<Tokens>), String>),
    Events(Result<(Vec<CalEvent>, Option<Tokens>), String>),
    /// Graph sign-in finished (Some) or sign-out (None).
    SignedIn(Option<Tokens>),
}

#[derive(Default)]
pub struct Teams {
    cfg: TeamsCfg,
    loaded: bool,
    link: Option<TeamsLink>,
    gen: u64,
    link_state: TeamsLinkState,
    meeting: Option<Meeting>,
    graph: Option<Tokens>,
    presence: Option<Presence>,
    presence_err: Option<String>,
    presence_busy: bool,
    mail: Option<Mail>,
    mail_err: Option<String>,
    mail_busy: bool,
    events: Vec<CalEvent>,
    events_busy: bool,
    wants_presence: bool,
    wants_mail: bool,
    wants_events: bool,
}

impl Teams {
    fn save_graph(&mut self, t: Option<Tokens>, host: &Host) {
        let path = host.dir.join("teams-auth.json");
        match &t {
            Some(t) => {
                let _ = crate::config::write_private(&path, &serde_json::to_string(t).unwrap_or_default());
            }
            None => {
                let _ = std::fs::remove_file(&path);
            }
        }
        self.graph = t;
    }

    fn start_link(&mut self, host: &mut Host) {
        if let Some(t) = self.link.take() {
            t.stop();
        }
        self.gen += 1;
        self.link_state = TeamsLinkState::default();
        self.meeting = None;
        let token = std::fs::read_to_string(host.dir.join("teams.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v["token"].as_str().map(str::to_string));
        self.link_state.paired = token.is_some();
        let Some(tx) = host.sender() else { return };
        let gen = self.gen;
        self.link = Some(TeamsLink::start(self.cfg.port, token, move |ev| tx.send(TeamsMsg::Link(gen, ev))));
    }

    fn stop_link(&mut self) {
        if let Some(t) = self.link.take() {
            t.stop();
        }
        self.gen += 1;
        self.link_state = TeamsLinkState::default();
        self.meeting = None;
    }

    /// Refresh presence; with `change`, first set it ("toggle", "auto" or an availability).
    fn poll_presence(&mut self, change: Option<String>, host: &Host) {
        if self.presence_busy && change.is_none() {
            return;
        }
        let (Some(tokens), Some(tx)) = (self.graph.clone(), host.sender()) else { return };
        self.presence_busy = true;
        let tc = self.cfg.clone();
        let current = self.presence.clone();
        std::thread::spawn(move || {
            let r = (|| {
                let (token, newer) = teams::access(&tc.client_id, &tc.tenant, &tokens)?;
                if let Some(to) = change {
                    let target = match to.as_str() {
                        "auto" => None,
                        "toggle" => {
                            let now = match current {
                                Some(p) => p,
                                None => teams::get_presence(&token)?,
                            };
                            if now.availability.starts_with("Available") {
                                Some(tc.away.clone())
                            } else {
                                None
                            }
                        }
                        other => Some(other.to_string()),
                    };
                    teams::set_presence(&token, target.as_deref())?;
                    std::thread::sleep(Duration::from_millis(800));
                }
                Ok((teams::get_presence(&token)?, newer))
            })();
            tx.send(TeamsMsg::Presence(r));
        });
    }

    fn poll_graph(&mut self, mail: bool, host: &Host) {
        let (Some(tokens), Some(tx)) = (self.graph.clone(), host.sender()) else { return };
        if mail {
            self.mail_busy = true;
        } else {
            self.events_busy = true;
        }
        let tc = self.cfg.clone();
        std::thread::spawn(move || {
            let r = teams::access(&tc.client_id, &tc.tenant, &tokens);
            if mail {
                tx.send(TeamsMsg::Mail(r.and_then(|(t, n)| teams::get_mail(&t).map(|m| (m, n)))));
            } else {
                tx.send(TeamsMsg::Events(r.and_then(|(t, n)| teams::get_events(&t).map(|e| (e, n)))));
            }
        });
    }

    /// Everything as signals.
    fn publish(&self, host: &mut Host) {
        let mut out: Vec<(String, Option<Value>)> = vec![
            ("connected".into(), Some(json!(self.link_state.connected))),
            ("paired".into(), Some(json!(self.link_state.paired))),
            ("error".into(), self.link_state.error.clone().map(Value::from)),
            ("signed_in".into(), Some(json!(self.graph.is_some()))),
            ("account".into(), self.graph.as_ref().map(|g| json!(g.account))),
            ("presence".into(), self.presence.as_ref().map(|p| json!(p.availability))),
            ("presence.activity".into(), self.presence.as_ref().map(|p| json!(p.activity))),
            ("presence.error".into(), self.presence_err.clone().map(Value::from)),
            ("mail.unread".into(), self.mail.as_ref().map(|m| json!(m.unread))),
            ("mail.error".into(), self.mail_err.clone().map(Value::from)),
            ("calendar.count".into(), Some(json!(self.events.len().min(EVENTS)))),
        ];
        let m = self.meeting.as_ref();
        out.push(("in_meeting".into(), m.map(|x| json!(x.in_meeting))));
        out.push(("mic_on".into(), m.map(|x| json!(x.in_meeting && !x.muted))));
        out.push(("video".into(), m.map(|x| json!(x.video_on))));
        for (k, f) in [
            ("muted", m.map(|x| x.muted)),
            ("hand_raised", m.map(|x| x.hand_raised)),
            ("blurred", m.map(|x| x.blurred)),
            ("recording", m.map(|x| x.recording)),
            ("sharing", m.map(|x| x.sharing)),
            ("can_toggle_mute", m.map(|x| x.can_toggle_mute)),
            ("can_toggle_video", m.map(|x| x.can_toggle_video)),
        ] {
            out.push((format!("meeting.{k}"), f.map(Value::from)));
        }
        let l = self.mail.as_ref().and_then(|m| m.latest.as_ref());
        out.push(("mail.latest.id".into(), l.map(|x| json!(x.id))));
        out.push(("mail.latest.from".into(), l.map(|x| json!(x.from))));
        out.push(("mail.latest.subject".into(), l.map(|x| json!(x.subject))));
        out.push(("mail.latest.link".into(), l.and_then(|x| x.link.clone()).map(Value::from)));
        for (k, v) in out {
            match v {
                Some(v) => host.publish(&k, v),
                None => host.clear(&k),
            };
        }
        for i in 0..EVENTS {
            host.clear_prefix(&format!("calendar.{i}."));
        }
        for (i, e) in self.events.iter().take(EVENTS).enumerate() {
            let p = format!("calendar.{i}");
            host.publish(&format!("{p}.id"), json!(e.id));
            host.publish(&format!("{p}.subject"), json!(e.subject));
            host.publish(&format!("{p}.start"), json!(e.start));
            host.publish(&format!("{p}.end"), json!(e.end));
            host.publish(&format!("{p}.teams"), json!(e.teams));
            if let Some(j) = &e.join {
                host.publish(&format!("{p}.join"), json!(j));
            }
        }
    }
}

impl Plugin for Teams {
    fn id(&self) -> &'static str {
        ID
    }

    fn configure(&mut self, cfg: &Config, host: &mut Host) {
        if !self.loaded {
            self.loaded = true;
            self.graph = std::fs::read_to_string(host.dir.join("teams-auth.json")).ok().and_then(|s| serde_json::from_str(&s).ok());
        }
        let port_changed = cfg.settings.teams.port != self.cfg.port;
        self.cfg = cfg.settings.teams.clone();
        let app = cfg.status_key.app.as_str();
        self.wants_presence = cfg.uses(&[AppKind::TeamsPresence], |_| false);
        self.wants_mail = cfg.settings.cards.email || app == "email";
        self.wants_events = cfg.settings.cards.meetings || app == "meeting";
        let wanted = cfg.uses(&[AppKind::TeamsMute, AppKind::TeamsCamera], |_| false) || cfg.uses_action(&format!("{ID}:meeting"));
        if wanted && (self.link.is_none() || port_changed) {
            self.start_link(host);
        } else if !wanted && self.link.is_some() {
            self.stop_link();
        }
        self.publish(host);
    }

    fn tick(&mut self, ticks: u64, _cfg: &Config, host: &mut Host) {
        if self.graph.is_none() {
            return;
        }
        if ticks % 30 == 1 && self.wants_presence {
            self.poll_presence(None, host);
        }
        if !self.mail_busy && ticks % 60 == 3 && self.wants_mail {
            self.poll_graph(true, host);
        }
        if !self.events_busy && ticks % 120 == 5 && self.wants_events {
            self.poll_graph(false, host);
        }
    }

    fn on_message(&mut self, msg: PluginMsg, host: &mut Host) {
        let Ok(msg) = msg.downcast::<TeamsMsg>() else { return };
        match *msg {
            TeamsMsg::Link(gen, _) if gen != self.gen => return,
            TeamsMsg::Link(_, ev) => match ev {
                TeamsEvent::Link { connected, paired, error } => {
                    self.link_state = TeamsLinkState { connected, paired, error };
                    if !connected {
                        self.meeting = None;
                    }
                }
                TeamsEvent::Meeting(m) => self.meeting = Some(m),
                TeamsEvent::Token(t) => {
                    let _ = crate::config::write_private(&host.dir.join("teams.json"), &json!({ "token": t }).to_string());
                    self.link_state.paired = true;
                }
            },
            TeamsMsg::Presence(r) => {
                self.presence_busy = false;
                match r {
                    Ok((p, tokens)) => {
                        if let Some(t) = tokens {
                            self.save_graph(Some(t), host);
                        }
                        self.presence = Some(p);
                        self.presence_err = None;
                    }
                    Err(e) => self.presence_err = Some(e),
                }
            }
            TeamsMsg::Mail(r) => {
                self.mail_busy = false;
                match r {
                    Ok((m, tokens)) => {
                        if let Some(t) = tokens {
                            self.save_graph(Some(t), host);
                        }
                        self.mail = Some(m);
                        self.mail_err = None;
                    }
                    Err(e) => self.mail_err = Some(e),
                }
            }
            TeamsMsg::Events(r) => {
                self.events_busy = false;
                match r {
                    Ok((ev, tokens)) => {
                        if let Some(t) = tokens {
                            self.save_graph(Some(t), host);
                        }
                        self.events = ev;
                    }
                    Err(e) => eprintln!("calendar: {e}"),
                }
            }
            TeamsMsg::SignedIn(t) => {
                let signed_in = t.is_some();
                self.save_graph(t, host);
                self.presence = None;
                self.presence_err = None;
                if !signed_in {
                    self.mail = None;
                    self.events.clear();
                }
                if signed_in {
                    self.poll_presence(None, host);
                }
            }
        }
        self.publish(host);
    }

    fn on_action(&mut self, name: &str, params: &Map<String, Value>, host: &mut Host) -> Option<Result<(), String>> {
        let p = |k: &str| params.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Some(match name {
            "meeting" => match &self.link {
                Some(link) if self.link_state.connected => {
                    link.send(&p("command"));
                    Ok(())
                }
                Some(_) => Err(self.link_state.error.clone().unwrap_or_else(|| "Teams is not connected".into())),
                None => Err("Teams is not set up".into()),
            },
            "presence" => {
                if self.graph.is_none() {
                    Err("Sign in to Microsoft 365 under Settings → Teams first".into())
                } else {
                    self.poll_presence(Some(p("to")), host);
                    Ok(())
                }
            }
            _ => return None,
        })
    }

    fn status(&self) -> Value {
        json!({
            "teams": self.link_state,
            "teamsOn": self.link.is_some(),
            "meeting": self.meeting,
            "signedIn": self.graph.is_some(),
            "account": self.graph.as_ref().map(|g| g.account.clone()),
            "presence": self.presence.as_ref().map(|p| p.availability.clone()),
            "presenceError": self.presence_err,
            "unread": self.mail.as_ref().map(|m| m.unread),
            "mailError": self.mail_err,
            "meetings": self.events.len(),
        })
    }
}

// ------------------------------------------------------------------ what the core reads

fn get<'a>(s: &'a Signals, name: &str) -> Option<&'a Value> {
    s.get(&format!("{ID}:{name}"))
}

fn flag(s: &Signals, name: &str) -> bool {
    get(s, name).and_then(Value::as_bool).unwrap_or(false)
}

fn text(s: &Signals, name: &str) -> Option<String> {
    get(s, name).and_then(Value::as_str).map(str::to_string)
}

pub fn link_state(s: &Signals) -> TeamsLinkState {
    TeamsLinkState { connected: flag(s, "connected"), paired: flag(s, "paired"), error: text(s, "error") }
}

pub fn meeting(s: &Signals) -> Option<Meeting> {
    get(s, "in_meeting")?;
    let f = |k: &str| flag(s, &format!("meeting.{k}"));
    Some(Meeting {
        in_meeting: flag(s, "in_meeting"),
        muted: f("muted"),
        video_on: flag(s, "video"),
        hand_raised: f("hand_raised"),
        blurred: f("blurred"),
        recording: f("recording"),
        sharing: f("sharing"),
        can_toggle_mute: f("can_toggle_mute"),
        can_toggle_video: f("can_toggle_video"),
    })
}

pub fn signed_in(s: &Signals) -> bool {
    flag(s, "signed_in")
}

pub fn presence(s: &Signals) -> Option<Presence> {
    Some(Presence { availability: text(s, "presence")?, activity: text(s, "presence.activity").unwrap_or_default() })
}

pub fn presence_error(s: &Signals) -> Option<String> {
    text(s, "presence.error")
}

pub fn mail(s: &Signals) -> Option<Mail> {
    let unread = get(s, "mail.unread")?.as_u64().unwrap_or(0) as u32;
    let latest = text(s, "mail.latest.id").map(|id| MailMsg {
        id,
        from: text(s, "mail.latest.from").unwrap_or_default(),
        subject: text(s, "mail.latest.subject").unwrap_or_default(),
        link: text(s, "mail.latest.link"),
    });
    Some(Mail { unread, latest })
}

pub fn events(s: &Signals) -> Vec<CalEvent> {
    let n = get(s, "calendar.count").and_then(Value::as_u64).unwrap_or(0) as usize;
    (0..n.min(EVENTS))
        .filter_map(|i| {
            let p = |k: &str| format!("calendar.{i}.{k}");
            Some(CalEvent {
                id: text(s, &p("id"))?,
                subject: text(s, &p("subject")).unwrap_or_default(),
                start: get(s, &p("start"))?.as_i64()?,
                end: get(s, &p("end"))?.as_i64()?,
                join: text(s, &p("join")),
                teams: flag(s, &p("teams")),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the plugin publishes, the core's readers get back unchanged.
    #[test]
    fn signals_round_trip() {
        let mut t = Teams {
            link_state: TeamsLinkState { connected: true, paired: true, error: None },
            meeting: Some(Meeting { in_meeting: true, muted: true, video_on: false, hand_raised: true, can_toggle_mute: true, ..Default::default() }),
            presence: Some(Presence { availability: "Busy".into(), activity: "InACall".into() }),
            mail: Some(Mail { unread: 3, latest: Some(MailMsg { id: "m1".into(), from: "Ana".into(), subject: "Hi".into(), link: None }) }),
            events: vec![CalEvent { id: "e1".into(), subject: "Standup".into(), start: 100, end: 200, join: Some("https://x".into()), teams: true }],
            ..Default::default()
        };
        t.graph = Some(Tokens { account: "me@x".into(), ..Default::default() });
        let mut s = Signals::default();
        t.publish(&mut Host::new(ID, &mut s, "d200".into()));
        assert_eq!(link_state(&s), t.link_state);
        assert_eq!(meeting(&s), t.meeting);
        assert_eq!(presence(&s), t.presence);
        assert_eq!(mail(&s), t.mail);
        assert_eq!(events(&s), t.events);
        assert!(signed_in(&s));
        assert_eq!(s.get(&format!("{ID}:mic_on")), Some(&json!(false)), "muted means the mic is off");
        // signing out clears the account data
        t.on_message(Box::new(TeamsMsg::SignedIn(None)), &mut Host::new(ID, &mut s, "d200".into()));
        assert!(!signed_in(&s) && mail(&s).is_none() && events(&s).is_empty());
    }

    #[test]
    fn actions_need_a_connection() {
        let mut t = Teams::default();
        let mut s = Signals::default();
        let mut h = Host::new(ID, &mut s, "d200".into());
        let p = json!({"command": "toggle-mute"}).as_object().cloned().unwrap();
        assert_eq!(t.on_action("meeting", &p, &mut h), Some(Err("Teams is not set up".into())));
        assert!(t.on_action("presence", &Map::new(), &mut h).unwrap().is_err());
        assert_eq!(t.on_action("nope", &Map::new(), &mut h), None);
    }
}
