//! Small per-machine preferences kept in openVJ's settings folder.

use std::path::PathBuf;

/// openVJ's settings folder: `~/Library/Application Support/openVJ` on macOS,
/// `%APPDATA%\openVJ` on Windows, `$XDG_CONFIG_HOME/openvj` elsewhere.
pub fn settings_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        Some(home?.join("Library/Application Support/openVJ"))
    } else if cfg!(windows) {
        Some(PathBuf::from(std::env::var_os("APPDATA")?).join("openVJ"))
    } else {
        Some(
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| home.map(|h| h.join(".config")))?
                .join("openvj"),
        )
    }
}

/// The layouts in the LAYOUT menu, in menu order (see ui/layouts.slint).
const LAYOUTS: [&str; 3] = ["mixer", "player", "synthesis"];

fn prefs_file() -> Option<PathBuf> {
    Some(settings_dir()?.join("preferences.json"))
}

fn read() -> serde_json::Map<String, serde_json::Value> {
    prefs_file()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| match v {
            serde_json::Value::Object(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default()
}

fn layout_index(name: &str) -> i32 {
    LAYOUTS.iter().position(|l| *l == name).unwrap_or(0) as i32
}

/// The layout used last time (0 Mixer, 1 Player, 2 Synthesis); Mixer the first time.
pub fn layout() -> i32 {
    read().get("layout").and_then(|v| v.as_str()).map_or(0, layout_index)
}

/// Remembers the layout for next launch, keeping any other preferences in the file.
pub fn set_layout(index: i32) {
    let Some(path) = prefs_file() else { return };
    let mut prefs = read();
    let name = LAYOUTS.get(index.max(0) as usize).copied().unwrap_or(LAYOUTS[0]);
    prefs.insert("layout".into(), name.into());
    let bytes = serde_json::to_vec_pretty(&serde_json::Value::Object(prefs)).unwrap_or_default();
    let result = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&path, bytes));
    if let Err(e) = result {
        eprintln!("openVJ: could not save preferences to {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_names_map_to_menu_order() {
        assert_eq!(layout_index("mixer"), 0);
        assert_eq!(layout_index("player"), 1);
        assert_eq!(layout_index("synthesis"), 2);
        assert_eq!(layout_index("something-else"), 0);
    }
}
