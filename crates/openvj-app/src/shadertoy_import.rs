//! Import from Shadertoy: fetches a shader through the Shadertoy.com API with the VJ's own
//! key (or takes pasted Image code), converts it with [`openvj_core::shadertoy`] and saves it
//! into the `Shadertoy` subfolder of the effects folder, where hot reload picks it up.

use std::path::PathBuf;
use std::sync::Mutex;

use openvj_core::shadertoy::{self, Converted};

/// What the VJ asked to import.
pub struct Request {
    /// A Shadertoy link or id.
    pub link: String,
    /// Pasted Image tab code, used instead of the API when not empty.
    pub code: String,
}

/// A finished import, for the dialog.
pub struct Outcome {
    pub ok: bool,
    pub message: String,
}

/// The import running on a worker thread hands its result over here.
static FINISHED: Mutex<Option<Outcome>> = Mutex::new(None);

/// Runs `request` on a worker thread and calls `done` there when it's finished; the
/// result is then in [`take_finished`].
pub fn start(request: Request, done: impl FnOnce() + Send + 'static) {
    let key = api_key();
    std::thread::spawn(move || {
        let outcome = run(&request, &key);
        *FINISHED.lock().unwrap_or_else(|e| e.into_inner()) = Some(outcome);
        done();
    });
}

/// The result of the last import, once.
pub fn take_finished() -> Option<Outcome> {
    FINISHED.lock().unwrap_or_else(|e| e.into_inner()).take()
}

fn run(request: &Request, key: &str) -> Outcome {
    let converted = if request.code.trim().is_empty() {
        let Some(id) = shadertoy::parse_id(&request.link) else {
            return fail("Paste a Shadertoy link (shadertoy.com/view/…) or its id.");
        };
        if key.is_empty() {
            return fail(
                "Importing by link needs your Shadertoy API key (Settings → Media). Or paste the Image code below.",
            );
        }
        fetch(&id, key).and_then(|json| shadertoy::from_api_json(&json))
    } else {
        shadertoy::from_image_code(&request.code, "", &request.link)
    };
    match converted.and_then(|c| save(&c).map(|p| (c, p))) {
        Ok((c, path)) => {
            let kind = if c.is_effect { "Effects" } else { "Sources" };
            let mut message = format!("Imported \"{}\" into the {kind} tab ({}).", c.name, short(&path));
            for note in &c.notes {
                message += &format!("\n• {note}");
            }
            Outcome { ok: true, message }
        }
        Err(e) => fail(&e),
    }
}

fn fail(message: &str) -> Outcome {
    Outcome { ok: false, message: message.to_owned() }
}

fn short(path: &std::path::Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home.and_then(|h| path.strip_prefix(h).ok().map(|p| p.to_owned())) {
        Some(rel) => format!("~/{}", rel.display()),
        None => path.display().to_string(),
    }
}

/// Asks the Shadertoy API for one shader's JSON.
fn fetch(id: &str, key: &str) -> Result<String, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .user_agent("openVJ")
        .http_status_as_error(false)
        .build()
        .new_agent();
    let mut response =
        agent.get(&shadertoy::api_url(id, key)).call().map_err(|e| format!("Couldn't reach Shadertoy: {e}"))?;
    let status = response.status();
    let body = response
        .body_mut()
        .with_config()
        .limit(16 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| format!("Couldn't read Shadertoy's answer: {e}"))?;
    if !status.is_success() && !body.trim_start().starts_with('{') {
        return Err(format!("Shadertoy answered {status}. Try again later, or paste the code."));
    }
    Ok(body)
}

/// The folder imports go to.
pub fn folder() -> Option<PathBuf> {
    Some(openvj_core::effect_folders::user_dir()?.join("Shadertoy"))
}

/// Writes the converted file. Importing the same shader again replaces its file, since the
/// file name carries the shader's id.
fn save(c: &Converted) -> Result<PathBuf, String> {
    let dir = folder().ok_or("there's no home folder to save into")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    let mut path = dir.join(&c.file_name);
    // Pasted code without a link has no id: don't overwrite another shader of the same name.
    if c.id.is_empty() {
        let stem = c.file_name.trim_end_matches(".fs").to_owned();
        let mut n = 2;
        while path.exists() {
            path = dir.join(format!("{stem} {n}.fs"));
            n += 1;
        }
    }
    std::fs::write(&path, &c.source).map_err(|e| format!("Couldn't save {}: {e}", path.display()))?;
    Ok(path)
}

// ---- The API key, kept in openVJ's preferences file. ----

/// openVJ's settings folder: `~/Library/Application Support/openVJ` on macOS,
/// `%APPDATA%\openVJ` on Windows, `$XDG_CONFIG_HOME/openvj` elsewhere.
fn prefs_file() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let dir = if cfg!(target_os = "macos") {
        home?.join("Library/Application Support/openVJ")
    } else if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?).join("openVJ")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|h| h.join(".config")))?
            .join("openvj")
    };
    Some(dir.join("preferences.json"))
}

fn read_prefs() -> serde_json::Map<String, serde_json::Value> {
    prefs_file()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| match v {
            serde_json::Value::Object(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default()
}

const KEY_PREF: &str = "shadertoy_api_key";

/// The VJ's Shadertoy API key, or empty. `OPENVJ_SHADERTOY_KEY` overrides the saved one.
pub fn api_key() -> String {
    if let Ok(k) = std::env::var("OPENVJ_SHADERTOY_KEY")
        && !k.trim().is_empty()
    {
        return k.trim().to_owned();
    }
    read_prefs().get(KEY_PREF).and_then(|v| v.as_str()).unwrap_or_default().trim().to_owned()
}

/// Saves the key, keeping the other preferences in the file.
pub fn set_api_key(key: &str) -> Result<(), String> {
    let path = prefs_file().ok_or("there's no settings folder")?;
    let mut prefs = read_prefs();
    let key = key.trim();
    if key.is_empty() {
        prefs.remove(KEY_PREF);
    } else {
        prefs.insert(KEY_PREF.into(), key.into());
    }
    let bytes = serde_json::to_vec_pretty(&serde_json::Value::Object(prefs)).unwrap_or_default();
    path.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|_| std::fs::write(&path, bytes))
        .map_err(|e| format!("Couldn't save the key to {}: {e}", path.display()))
}

/// Opens a web page in the default browser.
pub fn open_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err(format!("not a web link: {url}"));
    }
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener).arg(url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_without_key_or_code_explains_what_to_do() {
        let o = run(&Request { link: "https://www.shadertoy.com/view/XsXXDn".into(), code: String::new() }, "");
        assert!(!o.ok && o.message.contains("API key"), "{}", o.message);
        let o = run(&Request { link: "nope".into(), code: String::new() }, "k");
        assert!(!o.ok && o.message.contains("link"), "{}", o.message);
        let o = run(&Request { link: String::new(), code: "void main() {}".into() }, "");
        assert!(!o.ok && o.message.contains("mainImage"), "{}", o.message);
    }
}
