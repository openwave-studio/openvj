//! Prints what openVJ would import from a Resolume composition.
//!
//! `cargo run -p openvj-resolume --example avc_info -- path/to/composition.avc`

fn main() {
    let Some(path) = std::env::args_os().nth(1) else {
        eprintln!("usage: avc_info <composition.avc>");
        std::process::exit(2);
    };
    let imported = match openvj_resolume::import_file(path.as_ref()) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("could not import: {e}");
            std::process::exit(1);
        }
    };
    let (c, r) = (&imported.composition, &imported.report);
    println!("{} ({}x{}, {} BPM)", c.name, c.width, c.height, c.bpm);
    if let Some(v) = &r.saved_by {
        println!("saved by {v}");
    }
    println!("decks: {}", r.decks.join(", "));
    println!("{}", r.summary());
    for (i, g) in c.groups.iter().enumerate() {
        let members: Vec<String> = c.group_members(i).iter().map(|l| format!("L{}", l + 1)).collect();
        println!("group {}: {} {:>3.0}% {} [{}]", i + 1, g.name, g.opacity * 100.0, g.blend.name(), members.join(" "));
    }
    if c.layers.iter().any(|l| l.crossfade != openvj_core::CrossfadeSide::None) {
        println!("crossfader: {:.0}% ({})", c.crossfader.position * 100.0, c.crossfader.curve.name());
    }
    let mut c = c.clone();
    for d in 0..c.decks.len() {
        c.switch_deck(d);
        println!("deck {}: {} ({} columns)", d + 1, c.deck_name(), c.columns.len());
        for (i, layer) in c.layers.iter().enumerate().rev() {
            let fx: Vec<String> = layer.effects.iter().map(|e| e.name()).collect();
            let side = match layer.crossfade {
                openvj_core::CrossfadeSide::None => String::new(),
                side => format!(" side {}", side.label()),
            };
            println!(
                "  L{} {:<12} {:>3.0}% {:<9} fx: [{}]{side}",
                i + 1,
                layer.name,
                layer.opacity * 100.0,
                layer.blend.name(),
                fx.join(", ")
            );
            for (col, clip) in layer.clips.iter().enumerate() {
                if let Some(clip) = clip {
                    let mut play = vec![clip.play_mode.name().to_owned()];
                    if let Some(b) = clip.sync_beats {
                        play.push(format!("{b} beats"));
                    }
                    if let Some(s) = clip.length() {
                        play.push(format!("{s:.1} s"));
                    }
                    println!("      {:>2}: {} ({}) {}", col + 1, clip.name, clip.source.kind_label(), play.join(", "));
                }
            }
        }
    }
    for note in &r.notes {
        println!("  note: {note}");
    }
}
