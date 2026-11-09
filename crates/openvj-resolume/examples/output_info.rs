//! Prints what a Resolume Advanced Output file imports as.
//!
//! `cargo run -p openvj-resolume --example output_info -- AdvancedOutput.xml`

use openvj_core::ScreenLayer;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: output_info <AdvancedOutput.xml or preset>");
        std::process::exit(2);
    };
    let imported = match openvj_resolume::import_output_file(path.as_ref()) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }
    };
    let setup = &imported.setup;
    let [w, h] = setup.composition_size;
    println!("{} ({w}×{h}), saved by {}", setup.name, imported.saved_by.as_deref().unwrap_or("?"));
    for screen in &setup.screens {
        let [sw, sh] = screen.size();
        println!("  {} [{:?} \"{}\" {sw}×{sh}]", screen.name, screen.device.kind, screen.device.name);
        for layer in &screen.layers {
            match layer {
                ScreenLayer::Slice(s) => {
                    let warp = if s.warp.is_identity() {
                        "no warp".to_owned()
                    } else {
                        format!("{}×{} {:?} warp", s.warp.cols, s.warp.rows, s.warp.mode)
                    };
                    let shape = if s.shape.is_some() { ", polygon" } else { "" };
                    let mask = if s.mask.is_some() { ", masked" } else { "" };
                    println!("    slice {}: input {:?}, {warp}{shape}{mask}", s.name, s.input[0]);
                }
                ScreenLayer::Mask(m) => println!("    mask {} ({} contours)", m.name, m.contours.len()),
            }
        }
    }
    for note in &imported.notes {
        println!("  note: {note}");
    }
}
