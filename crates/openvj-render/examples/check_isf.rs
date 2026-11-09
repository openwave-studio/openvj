//! Loads ISF folders the way openVJ does and lists what it found.
//!
//! `cargo run -p openvj-render --example check_isf -- <folder>...` prints one line per
//! file that fails (`ERROR<TAB>path<TAB>message`) and a summary, and exits non-zero if any
//! file failed. `scripts/import-ghost-arcade.py` uses it to drop files that don't compile.

use openvj_core::effect_folders::Scanner;

fn main() {
    let dirs: Vec<_> = std::env::args_os().skip(1).map(std::path::PathBuf::from).collect();
    if dirs.is_empty() {
        eprintln!("usage: check_isf <folder>...");
        std::process::exit(2);
    }
    let r = Scanner::new(dirs, openvj_render::check_effect).scan().unwrap_or_default();
    for e in &r.errors {
        let first = e.message.lines().find(|l| !l.trim().is_empty()).unwrap_or_default().trim();
        println!("ERROR\t{}\t{first}", e.path.display());
    }
    println!("{} effects, {} generators, {} errors", r.effects.len(), r.generators.len(), r.errors.len());
    if !r.errors.is_empty() {
        std::process::exit(1);
    }
}
