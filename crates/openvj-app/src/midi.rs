//! MIDI input: listens on every MIDI input port and queues raw messages for the frame loop.

use std::sync::mpsc::{self, Receiver, Sender};

use midir::{Ignore, MidiInput, MidiInputConnection};

const CLIENT: &str = "openVJ";

/// A raw MIDI message and the index of the port it came from.
type Incoming = (usize, Vec<u8>);

pub struct MidiPorts {
    /// Open connections, by port name. Dropping one closes the port.
    conns: Vec<(String, MidiInputConnection<()>)>,
    rx: Receiver<Incoming>,
}

/// Names of the MIDI input ports on this machine, or `None` if the system has no MIDI
/// service (such as Linux without the ALSA sequencer).
pub fn port_names() -> Option<Vec<String>> {
    let input = MidiInput::new(CLIENT).ok()?;
    Some(input.ports().iter().filter_map(|p| input.port_name(p).ok()).collect())
}

impl MidiPorts {
    /// Connects to every input port. Ports that fail to open are skipped and listed in the
    /// returned errors.
    pub fn open_all() -> (Self, Vec<String>) {
        let (tx, rx) = mpsc::channel();
        let mut conns = Vec::new();
        let mut errors = Vec::new();
        let count = MidiInput::new(CLIENT).map(|i| i.port_count()).unwrap_or(0);
        for i in 0..count {
            match connect(i, conns.len(), tx.clone()) {
                Ok(c) => conns.push(c),
                Err(e) => errors.push(e),
            }
        }
        (Self { conns, rx }, errors)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.conns.iter().map(|(n, _)| n.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.conns.is_empty()
    }

    /// Messages received since the last call, with the name of their port.
    pub fn drain(&self) -> Vec<(&str, Vec<u8>)> {
        self.rx.try_iter().filter_map(|(port, msg)| Some((self.conns.get(port)?.0.as_str(), msg))).collect()
    }
}

/// Opens port `index` (in the system's list); messages are tagged with `slot`.
fn connect(index: usize, slot: usize, tx: Sender<Incoming>) -> Result<(String, MidiInputConnection<()>), String> {
    // midir needs one MidiInput per connection.
    let mut input = MidiInput::new(CLIENT).map_err(|e| e.to_string())?;
    input.ignore(Ignore::All);
    let port = input.ports().into_iter().nth(index).ok_or("port went away")?;
    let name = input.port_name(&port).unwrap_or_else(|_| format!("MIDI port {}", index + 1));
    let conn = input
        .connect(&port, "openvj-in", move |_, msg, _| drop(tx.send((slot, msg.to_vec()))), ())
        .map_err(|e| format!("{name}: {e}"))?;
    Ok((name, conn))
}
