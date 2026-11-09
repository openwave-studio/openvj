//! Prints what openVJ would map from a Resolume MIDI shortcut preset.
//!
//! `cargo run -p openvj-resolume --example midi_info -- "Circuit Tracks.xml" [set.avc]`
//!
//! With an `.avc`, paths resolve against that composition, as when the app imports both.
//! Without one, they resolve against an empty 8-layer, 16-column composition.

use openvj_core::Composition;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let Some(preset) = args.next() else {
        eprintln!("usage: midi_info <preset.xml> [composition.avc]");
        std::process::exit(2);
    };
    let avc = args.next().map(|p| openvj_resolume::import_file(p.as_ref()).unwrap_or_else(|e| fail(e)));
    let comp = avc.as_ref().map_or_else(|| Composition::new(1920, 1080, 8, 16), |a| a.composition.clone());
    let refs = avc.as_ref().map(|a| &a.refs);
    let imported = openvj_resolume::import_midi_preset_file(preset.as_ref(), &comp, refs).unwrap_or_else(|e| fail(e));
    println!("{}: {}", imported.map.name, imported.summary());
    for m in &imported.map.mappings {
        let range = if (m.min, m.max) != (0.0, 1.0) { format!(" {:.2}–{:.2}", m.min, m.max) } else { String::new() };
        println!("  {:<16} {:<7?} {:?}{range}", m.control.label(), m.behaviour, m.target);
    }
    for note in &imported.notes {
        println!("  note: {note}");
    }
}

fn fail(e: String) -> ! {
    eprintln!("could not import: {e}");
    std::process::exit(1);
}
