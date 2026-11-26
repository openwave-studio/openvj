//! OSC input, and the link to openDJ that uses it.
//!
//! [`OscInput`] is one UDP socket on a given port with a reader thread; it also
//! sends from that port, so apps that answer to the sender (openDJ does) reach it.
//! Other OSC users (control input on Resolume's port 7000) open their own.
//! [`OpenDj`] finds openDJ (on this computer at its default port 9000, and over
//! Bonjour as `openDJ._osc._udp`), subscribes to its beat, phrase, track and cue
//! events, and keeps subscribing every few seconds so a restarted openDJ is
//! picked up again. Nothing here runs on the render path: the app drains the
//! decoded events once a frame.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use openvj_core::opendj::DjEvent;
use rosc::{OscMessage, OscPacket, OscType};

/// openDJ answers to the port a subscription came from, so the link uses a port of
/// its own picked by the system, and Resolume's 7000 stays free for control input.
const LINK_PORT: u16 = 0;
/// openDJ's OSC port when nothing else was set.
const OPENDJ_PORT: u16 = 9000;
/// The events openVJ follows.
const PATTERNS: [&str; 4] = ["/beat", "/phrase", "/deck/*/track", "/deck/*/cue"];
/// A control openVJ asks for, so a quiet openDJ (decks stopped) still answers.
const HEARTBEAT: &str = "/master/crossfader";
/// How often openVJ (re)subscribes and asks for the heartbeat.
const RESUBSCRIBE: Duration = Duration::from_secs(3);
/// openDJ counts as gone when nothing came from it for this long.
const TIMEOUT: Duration = Duration::from_secs(7);

/// One UDP socket listening for OSC, with a reader thread.
pub struct OscInput {
    socket: UdpSocket,
    rx: Receiver<(OscMessage, SocketAddr)>,
    stop: Arc<AtomicBool>,
}

impl OscInput {
    /// Listens on `port`, or on any free port when it's taken or 0.
    pub fn open(port: u16) -> io::Result<Self> {
        let socket =
            UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)).or_else(|_| UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)))?;
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        let reader = socket.try_clone()?;
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();
        let quit = stop.clone();
        std::thread::Builder::new().name("osc-in".into()).spawn(move || {
            let mut buf = vec![0u8; rosc::decoder::MTU];
            while !quit.load(Ordering::Relaxed) {
                let Ok((n, from)) = reader.recv_from(&mut buf) else { continue };
                let Ok((_, packet)) = rosc::decoder::decode_udp(&buf[..n]) else { continue };
                let mut messages = Vec::new();
                flatten(packet, &mut messages);
                for m in messages {
                    if tx.send((m, from)).is_err() {
                        return;
                    }
                }
            }
        })?;
        Ok(Self { socket, rx, stop })
    }

    pub fn send(&self, message: OscMessage, to: SocketAddr) {
        if let Ok(bytes) = rosc::encoder::encode(&OscPacket::Message(message)) {
            let _ = self.socket.send_to(&bytes, to);
        }
    }

    /// The messages received since the last call, with who sent them.
    pub fn drain(&self) -> Vec<(OscMessage, SocketAddr)> {
        self.rx.try_iter().collect()
    }
}

impl Drop for OscInput {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn flatten(packet: OscPacket, out: &mut Vec<OscMessage>) {
    match packet {
        OscPacket::Message(m) => out.push(m),
        OscPacket::Bundle(b) => b.content.into_iter().for_each(|p| flatten(p, out)),
    }
}

/// Decodes one of openDJ's event messages; None for anything else.
pub fn dj_event(m: &OscMessage) -> Option<DjEvent> {
    let int = |i: usize| match m.args.get(i)? {
        OscType::Int(v) => Some(*v as i64),
        OscType::Long(v) => Some(*v),
        OscType::Float(v) => Some(*v as i64),
        OscType::Double(v) => Some(*v as i64),
        _ => None,
    };
    let float = |i: usize| match m.args.get(i)? {
        OscType::Float(v) => Some(*v),
        OscType::Double(v) => Some(*v as f32),
        OscType::Int(v) => Some(*v as f32),
        _ => None,
    };
    let text = |i: usize| match m.args.get(i) {
        Some(OscType::String(s)) => s.clone(),
        _ => String::new(),
    };
    let count = |i: usize| int(i).filter(|v| *v >= 0).map(|v| v as u32);
    match m.addr.as_str() {
        "/beat" => Some(DjEvent::Beat { beat: count(0)?, bar: count(1).unwrap_or(0), bpm: float(2).unwrap_or(0.0) }),
        "/phrase" => {
            let name = text(0);
            (!name.is_empty()).then(|| DjEvent::Phrase { name, bars: count(1).unwrap_or(0) })
        }
        address => {
            let rest = address.strip_prefix("/deck/")?;
            let (deck, what) = rest.split_once('/')?;
            let deck: u32 = deck.parse().ok()?;
            match what {
                "track" => Some(DjEvent::Track {
                    deck,
                    title: text(0),
                    artist: text(1),
                    bpm: float(2).unwrap_or(0.0),
                    key: text(3),
                }),
                "cue" => Some(DjEvent::Cue { deck, cue: count(0)? }),
                _ => None,
            }
        }
    }
}

fn message(addr: &str, args: Vec<OscType>) -> OscMessage {
    OscMessage { addr: addr.into(), args }
}

/// The link to openDJ: finds it, subscribes, and turns its messages into events.
pub struct OpenDj {
    input: OscInput,
    /// Where openDJ may be listening: this computer's default port, plus what Bonjour found.
    targets: Vec<SocketAddr>,
    bonjour: Option<(mdns_sd::ServiceDaemon, mdns_sd::Receiver<mdns_sd::ServiceEvent>)>,
    /// Bonjour instances by full name, so a removed one can be dropped.
    found: HashMap<String, Vec<SocketAddr>>,
    last_subscribe: Option<Instant>,
    /// When something last came from openDJ, and from where.
    heard: Option<(Instant, SocketAddr)>,
}

impl OpenDj {
    pub fn start() -> io::Result<Self> {
        let input = OscInput::open(LINK_PORT)?;
        let bonjour = mdns_sd::ServiceDaemon::new().ok().and_then(|d| {
            let rx = d.browse("_osc._udp.local.").ok()?;
            Some((d, rx))
        });
        Ok(Self {
            input,
            targets: vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), OPENDJ_PORT)],
            bonjour,
            found: HashMap::new(),
            last_subscribe: None,
            heard: None,
        })
    }

    /// Where openDJ was last heard from, while it's connected.
    pub fn connected(&self) -> Option<SocketAddr> {
        self.heard.filter(|(t, _)| t.elapsed() < TIMEOUT).map(|(_, from)| from)
    }

    /// Call once a frame: keeps the subscription alive and returns openDJ's events.
    pub fn poll(&mut self) -> Vec<DjEvent> {
        self.browse();
        if self.last_subscribe.is_none_or(|t| t.elapsed() >= RESUBSCRIBE) {
            self.last_subscribe = Some(Instant::now());
            self.subscribe();
        }
        let mut events = Vec::new();
        for (m, from) in self.input.drain() {
            let from_dj = self.targets.iter().any(|t| t.ip() == from.ip() && (t.port() == from.port() || is_local(t)));
            if !from_dj {
                continue;
            }
            self.heard = Some((Instant::now(), from));
            if let Some(e) = dj_event(&m) {
                events.push(e);
            }
        }
        events
    }

    /// Unsubscribes then subscribes again (openDJ keeps every pattern it's sent, so this
    /// doesn't pile up), and asks for the heartbeat control.
    fn subscribe(&self) {
        let patterns: Vec<OscType> = PATTERNS.iter().map(|p| OscType::String((*p).into())).collect();
        for &to in &self.targets {
            self.input.send(message("/unsubscribe", patterns.clone()), to);
            self.input.send(message("/subscribe", patterns.clone()), to);
            self.input.send(message(HEARTBEAT, vec![]), to);
        }
    }

    fn browse(&mut self) {
        let Some((_, rx)) = &self.bonjour else { return };
        let mut changed = false;
        while let Ok(event) = rx.try_recv() {
            match event {
                mdns_sd::ServiceEvent::ServiceResolved(s) if is_opendj(&s.fullname) => {
                    let addrs = s
                        .addresses
                        .iter()
                        .map(|ip| ip.to_ip_addr())
                        .filter(IpAddr::is_ipv4)
                        .map(|ip| SocketAddr::new(ip, s.port))
                        .collect();
                    self.found.insert(s.fullname.clone(), addrs);
                    changed = true;
                }
                mdns_sd::ServiceEvent::ServiceRemoved(_, name) => {
                    changed |= self.found.remove(&name).is_some();
                }
                _ => {}
            }
        }
        if changed {
            let mut targets = vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), OPENDJ_PORT)];
            for a in self.found.values().flatten() {
                if !targets.contains(a) {
                    targets.push(*a);
                }
            }
            self.targets = targets;
            self.last_subscribe = None;
        }
    }
}

impl Drop for OpenDj {
    fn drop(&mut self) {
        let patterns: Vec<OscType> = PATTERNS.iter().map(|p| OscType::String((*p).into())).collect();
        for &to in &self.targets {
            self.input.send(message("/unsubscribe", patterns.clone()), to);
        }
        if let Some((daemon, _)) = &self.bonjour {
            let _ = daemon.shutdown();
        }
    }
}

/// openDJ's Bonjour instance is named "openDJ" (a second copy may be "openDJ (2)").
fn is_opendj(fullname: &str) -> bool {
    fullname.to_ascii_lowercase().starts_with("opendj")
}

/// openDJ on this computer may answer from another port than the one it listens on.
fn is_local(addr: &SocketAddr) -> bool {
    addr.ip().is_loopback()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(addr: &str, args: Vec<OscType>) -> OscMessage {
        message(addr, args)
    }

    #[test]
    fn decodes_opendj_events() {
        assert_eq!(
            dj_event(&m("/beat", vec![OscType::Int(1), OscType::Int(5), OscType::Float(124.0)])),
            Some(DjEvent::Beat { beat: 1, bar: 5, bpm: 124.0 })
        );
        assert_eq!(
            dj_event(&m("/phrase", vec![OscType::String("Drop".into()), OscType::Int(16)])),
            Some(DjEvent::Phrase { name: "Drop".into(), bars: 16 })
        );
        assert_eq!(
            dj_event(&m(
                "/deck/2/track",
                vec![
                    OscType::String("Title".into()),
                    OscType::String("Artist".into()),
                    OscType::Float(128.0),
                    OscType::String("8A".into())
                ]
            )),
            Some(DjEvent::Track {
                deck: 2,
                title: "Title".into(),
                artist: "Artist".into(),
                bpm: 128.0,
                key: "8A".into()
            })
        );
        assert_eq!(dj_event(&m("/deck/1/cue", vec![OscType::Int(3)])), Some(DjEvent::Cue { deck: 1, cue: 3 }));
        assert_eq!(dj_event(&m("/master/crossfader", vec![OscType::Float(0.0)])), None);
        assert_eq!(dj_event(&m("/deck/x/track", vec![])), None);
        assert_eq!(dj_event(&m("/beat", vec![])), None);
    }

    #[test]
    fn bonjour_names() {
        assert!(is_opendj("openDJ._osc._udp.local."));
        assert!(is_opendj("openDJ (2)._osc._udp.local."));
        assert!(!is_opendj("TouchOSC._osc._udp.local."));
    }

    /// A stand-in for openDJ on a local port: openVJ subscribes, then receives events.
    #[test]
    fn subscribes_and_receives_from_a_local_opendj() {
        let dj = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        dj.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut link = OpenDj::start().unwrap();
        link.bonjour = None;
        link.targets = vec![dj.local_addr().unwrap()];
        assert!(link.poll().is_empty());
        let mut buf = [0u8; 1536];
        let mut subscriber = None;
        for _ in 0..3 {
            let (n, from) = dj.recv_from(&mut buf).unwrap();
            let (_, p) = rosc::decoder::decode_udp(&buf[..n]).unwrap();
            if let OscPacket::Message(msg) = p
                && msg.addr == "/subscribe"
            {
                assert_eq!(msg.args.len(), PATTERNS.len());
                subscriber = Some(from);
            }
        }
        let to = subscriber.expect("openVJ subscribed");
        let beat = rosc::encoder::encode(&OscPacket::Message(m(
            "/beat",
            vec![OscType::Int(2), OscType::Int(3), OscType::Float(126.0)],
        )))
        .unwrap();
        dj.send_to(&beat, (Ipv4Addr::LOCALHOST, to.port())).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut got = Vec::new();
        while got.is_empty() && Instant::now() < deadline {
            got = link.poll();
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(got, vec![DjEvent::Beat { beat: 2, bar: 3, bpm: 126.0 }]);
        assert!(link.connected().is_some());
    }
}
