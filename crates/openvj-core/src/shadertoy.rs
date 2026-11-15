//! Converts [Shadertoy](https://www.shadertoy.com) shaders into openVJ ISF files.
//!
//! The input is the JSON the Shadertoy API returns for one shader (`{"Shader": {"info": …,
//! "renderpass": […]}}`), or just the code of an Image tab. The output is the text of an ISF
//! `.fs` file: Buffer A–D become persistent passes in Shadertoy's order, then the Image
//! pass. The Shadertoy names themselves (`iTime`, `iResolution`, a `mainImage` without a
//! `main`…) are left in the code; the renderer understands them in any ISF file.
//!
//! Each pass keeps its own `mainImage` and helper functions: names that two passes both
//! define are renamed per pass with `#define`/`#undef`, so passes don't clash.
//!
//! Coordinates: openVJ's textures and `vv_FragNormCoord` have a top-left origin, Shadertoy's
//! a bottom-left one. Buffers are drawn and read in the same (unflipped) space, so they agree
//! with each other; only the Image pass flips its `fragCoord`, and a video input is copied
//! flipped into a first pass so it reads upright.

use serde_json::{Map, Value};

/// The texture that holds openVJ's audio in Shadertoy's layout: 512×2, the spectrum on the
/// first row and the waveform on the second. The renderer binds it to files that name it.
pub const AUDIO_TEXTURE: &str = "isf_shadertoy_audio";

/// Shadertoy's ids for the outputs of Buffer A–D.
const BUFFER_OUTPUT_IDS: [&str; 4] = ["4dXGR8", "XsXGR8", "4sXGR8", "XdfGR8"];
/// Old API files that name buffers by preview image instead.
const BUFFER_PREVIEWS: [&str; 4] = ["buffer00", "buffer01", "buffer02", "buffer03"];
const BUFFER_TARGETS: [&str; 4] = ["BufferA", "BufferB", "BufferC", "BufferD"];
/// The pass that copies the video input upright, for shaders that filter it.
const INPUT_TARGET: &str = "ShadertoyInput";

/// Shadertoy's licence for shaders that don't state their own.
pub const DEFAULT_LICENSE: &str =
    "CC BY-NC-SA 3.0 (Shadertoy's default) unless the author states otherwise in the code";

/// A converted shader.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    /// Shadertoy's id, empty for pasted code without a link.
    pub id: String,
    /// The shader's title.
    pub name: String,
    /// A file name for it (without folder), ending in `.fs`.
    pub file_name: String,
    /// The ISF file.
    pub source: String,
    /// Takes the clip's video as input (an effect) instead of making its own (a generator).
    pub is_effect: bool,
    /// What couldn't be carried over, for the VJ: "iChannel1 (keyboard) reads black".
    pub notes: Vec<String>,
}

/// Pulls a Shadertoy id out of a link (`https://www.shadertoy.com/view/XsXXDn`, an embed
/// link, an API link) or a bare id.
pub fn parse_id(text: &str) -> Option<String> {
    let t = text.trim().trim_end_matches('/');
    let candidate = match t.rfind("/view/").or_else(|| t.rfind("/embed/")).or_else(|| t.rfind("/shaders/")) {
        Some(i) => {
            let rest = &t[i..];
            let rest = &rest[rest[1..].find('/').map_or(0, |j| j + 2)..];
            rest.split(['?', '#', '/']).next().unwrap_or("")
        }
        None => t,
    };
    let ok = (5..=8).contains(&candidate.len()) && candidate.chars().all(|c| c.is_ascii_alphanumeric());
    ok.then(|| candidate.to_owned())
}

/// The Shadertoy page of a shader.
pub fn view_url(id: &str) -> String {
    format!("https://www.shadertoy.com/view/{id}")
}

/// The API request for one shader.
pub fn api_url(id: &str, key: &str) -> String {
    format!("https://www.shadertoy.com/api/v1/shaders/{id}?key={key}")
}

/// What a channel is fed with.
#[derive(Debug, Clone, PartialEq)]
enum Channel {
    Buffer(usize),
    Audio,
    Video,
    Unsupported(String),
}

#[derive(Debug, Clone)]
struct Pass {
    /// `Some(0..4)` for Buffer A–D, `None` for Image.
    buffer: Option<usize>,
    code: String,
    channels: [Option<Channel>; 4],
}

/// Converts the JSON the Shadertoy API returns for one shader.
pub fn from_api_json(json: &str) -> Result<Converted, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("Shadertoy's answer isn't valid JSON: {e}"))?;
    if let Some(err) = v.get("Error").and_then(Value::as_str) {
        return Err(api_error(err));
    }
    let shader = v.get("Shader").unwrap_or(&v);
    let info = shader.get("info").and_then(Value::as_object).cloned().unwrap_or_default();
    let text = |k: &str| info.get(k).and_then(Value::as_str).unwrap_or_default().trim().to_owned();
    let id = text("id");
    let passes_json = shader.get("renderpass").and_then(Value::as_array).ok_or("the shader has no render passes")?;

    let mut notes = Vec::new();
    let mut common = String::new();
    let mut passes = Vec::new();
    for p in passes_json {
        let ty = p.get("type").and_then(Value::as_str).unwrap_or("image");
        let name = p.get("name").and_then(Value::as_str).unwrap_or(ty);
        let code = p.get("code").and_then(Value::as_str).unwrap_or_default().to_owned();
        let buffer = match ty {
            "image" => None,
            "common" => {
                common = code;
                continue;
            }
            "buffer" => Some(buffer_index(p, name).ok_or_else(|| format!("unknown buffer \"{name}\""))?),
            "sound" => {
                notes.push("the Sound tab was left out (openVJ doesn't play shader sound)".into());
                continue;
            }
            "cubemap" => {
                notes.push(format!("the {name} pass was left out (cubemap passes aren't supported)"));
                continue;
            }
            other => {
                notes.push(format!("the {name} pass was left out (\"{other}\" passes aren't supported)"));
                continue;
            }
        };
        let mut channels: [Option<Channel>; 4] = Default::default();
        for input in p.get("inputs").and_then(Value::as_array).into_iter().flatten() {
            let Some(ch) = input.get("channel").and_then(Value::as_u64).filter(|c| *c < 4) else { continue };
            channels[ch as usize] = Some(channel(input));
        }
        passes.push(Pass { buffer, code, channels });
    }
    if !passes.iter().any(|p| p.buffer.is_none()) {
        return Err("the shader has no Image pass".into());
    }
    // Shadertoy's order: Buffer A, B, C, D, then Image.
    passes.sort_by_key(|p| p.buffer.unwrap_or(usize::MAX));

    let name = if text("name").is_empty() { id.clone() } else { text("name") };
    let meta = Meta { id, name, author: text("username"), description: text("description") };
    Ok(convert(meta, common, passes, notes))
}

/// Converts the code of a Shadertoy Image tab, pasted without the rest of the shader.
/// `link` (optional) is the shader's page or id, for the credit.
pub fn from_image_code(code: &str, name: &str, link: &str) -> Result<Converted, String> {
    if !code.contains("mainImage") {
        return Err("that doesn't look like Shadertoy code: there's no mainImage function".into());
    }
    let id = parse_id(link).unwrap_or_default();
    let name = match name.trim() {
        "" => title_from_comment(code)
            .unwrap_or_else(|| if id.is_empty() { "Shadertoy shader".to_owned() } else { format!("Shadertoy {id}") }),
        n => n.to_owned(),
    };
    let mut channels: [Option<Channel>; 4] = Default::default();
    let mut notes = Vec::new();
    for (i, ch) in channels.iter_mut().enumerate() {
        if uses_word(code, &format!("iChannel{i}")) {
            *ch = Some(Channel::Unsupported("pasted code doesn't say what it reads".into()));
            notes.push(format!(
                "iChannel{i} reads black: pasted code doesn't say what it reads (import by link to get buffers and audio)"
            ));
        }
    }
    let meta = Meta { id, name, author: String::new(), description: String::new() };
    let mut out = convert(meta, String::new(), vec![Pass { buffer: None, code: code.to_owned(), channels }], vec![]);
    out.notes = notes;
    Ok(out)
}

/// A title from the code's first line when it's a short `//` comment, as many shaders start.
fn title_from_comment(code: &str) -> Option<String> {
    let first = code.lines().map(str::trim).find(|l| !l.is_empty())?;
    let title = first.strip_prefix("//")?.trim().trim_matches(|c: char| c == '-' || c == '=' || c == '*').trim();
    let ok = !title.is_empty() && title.chars().count() <= 48 && !title.to_ascii_lowercase().contains("licen");
    ok.then(|| title.to_owned())
}

fn api_error(err: &str) -> String {
    let lower = err.to_ascii_lowercase();
    if lower.contains("not found") {
        "Shadertoy doesn't share this shader through its API: the author hasn't set it to \"Public + API\". \
         Use Paste code instead."
            .into()
    } else if lower.contains("key") {
        format!("Shadertoy didn't accept the API key ({err}). Check it in Settings → Sources.")
    } else {
        format!("Shadertoy answered: {err}")
    }
}

fn buffer_index(pass: &Value, name: &str) -> Option<usize> {
    let out_id =
        pass.get("outputs").and_then(Value::as_array).and_then(|o| o.first()).and_then(|o| o.get("id")).map(id_text);
    if let Some(i) = out_id.and_then(|id| BUFFER_OUTPUT_IDS.iter().position(|b| *b == id)) {
        return Some(i);
    }
    let letter = name.trim().strip_prefix("Buf").and_then(|n| n.chars().last())?;
    "ABCD".find(letter.to_ascii_uppercase())
}

/// Input and output ids are strings in the current API and numbers in old answers.
fn id_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => match n.as_u64() {
            // Old API: Buffer A–D are 257–260.
            Some(i @ 257..=260) => BUFFER_OUTPUT_IDS[(i - 257) as usize].to_owned(),
            _ => n.to_string(),
        },
        _ => String::new(),
    }
}

fn channel(input: &Value) -> Channel {
    let ctype = input.get("ctype").or_else(|| input.get("type")).and_then(Value::as_str).unwrap_or("");
    let src = input.get("src").or_else(|| input.get("filepath")).and_then(Value::as_str).unwrap_or("");
    match ctype {
        "buffer" => {
            let id = input.get("id").map(id_text).unwrap_or_default();
            BUFFER_OUTPUT_IDS
                .iter()
                .position(|b| *b == id)
                .or_else(|| BUFFER_PREVIEWS.iter().position(|b| src.contains(b)))
                .map_or_else(|| Channel::Unsupported("an unknown buffer".into()), Channel::Buffer)
        }
        "music" | "musicstream" | "mic" => Channel::Audio,
        "webcam" | "video" => Channel::Video,
        "keyboard" => Channel::Unsupported("keyboard".into()),
        "cubemap" => Channel::Unsupported("a cubemap".into()),
        "volume" => Channel::Unsupported("a 3D texture".into()),
        "texture" => Channel::Unsupported("a texture".into()),
        "" => Channel::Unsupported("an unknown input".into()),
        other => Channel::Unsupported(format!("\"{other}\"")),
    }
}

struct Meta {
    id: String,
    name: String,
    author: String,
    description: String,
}

fn convert(meta: Meta, common: String, passes: Vec<Pass>, mut notes: Vec<String>) -> Converted {
    let is_effect = passes.iter().any(|p| p.channels.iter().any(|c| c == &Some(Channel::Video)));

    // Names that more than one pass defines get a per-pass suffix.
    let decls: Vec<Decls> = passes.iter().map(|p| top_level_names(&p.code)).collect();
    let common_names = top_level_names(&common);
    let mut clashing: Vec<String> = Vec::new();
    for (i, d) in decls.iter().enumerate() {
        for n in &d.names {
            if n != "mainImage"
                && !clashing.contains(n)
                && !common_names.names.contains(n)
                && decls.iter().enumerate().any(|(j, o)| j != i && o.names.contains(n))
            {
                clashing.push(n.clone());
            }
        }
    }

    let mut glsl = String::new();
    if !common.trim().is_empty() {
        glsl += "// ---- Common ----\n";
        glsl += &common;
        glsl += "\n";
    }
    let mut entry_points = Vec::new();
    for (pass, d) in passes.iter().zip(&decls) {
        let label = pass.buffer.map_or("Image", |b| BUFFER_TARGETS[b]);
        let label_text = pass.buffer.map_or("Image".to_owned(), |b| format!("Buffer {}", &"ABCD"[b..=b]));
        glsl += &format!("\n// ---- {label_text} ----\n");
        let mut undef = Vec::new();
        let entry = format!("mainImage_{label}");
        glsl += &format!("#define mainImage {entry}\n");
        undef.push("mainImage".to_owned());
        for n in d.names.iter().filter(|n| clashing.contains(n)) {
            glsl += &format!("#define {n} {n}_{label}\n");
            undef.push(n.clone());
        }
        for (i, ch) in pass.channels.iter().enumerate() {
            let name = format!("iChannel{i}");
            let texture = match ch {
                Some(Channel::Buffer(b)) => {
                    if !passes.iter().any(|p| p.buffer == Some(*b)) {
                        notes.push(format!(
                            "{label_text}'s iChannel{i} reads a buffer the shader doesn't have, so it reads black"
                        ));
                    }
                    BUFFER_TARGETS[*b]
                }
                Some(Channel::Audio) => AUDIO_TEXTURE,
                Some(Channel::Video) => INPUT_TARGET,
                Some(Channel::Unsupported(what)) => {
                    if !what.contains("pasted") {
                        notes.push(format!("{label_text}'s iChannel{i} ({what}) isn't supported yet and reads black"));
                    }
                    "inputImage"
                }
                None => "inputImage",
            };
            // An unused, unbound channel stays undefined: the renderer gives it a fallback.
            if ch.is_some() || uses_word(&pass.code, &name) {
                glsl += &format!("#define {name} sampler2D({texture}, isf_sampler)\n");
                undef.push(name);
            }
        }
        glsl += &pass.code;
        glsl += "\n";
        for n in d.macros.iter().chain(&undef) {
            glsl += &format!("#undef {n}\n");
        }
        entry_points.push((pass.buffer, entry));
    }

    // The entry point: one function per pass, picked by PASSINDEX.
    let mut branches: Vec<String> = Vec::new();
    if is_effect {
        branches.push(
            "        st_color = IMG_NORM_PIXEL(inputImage, vec2(vv_FragNormCoord.x, 1.0 - vv_FragNormCoord.y));\n"
                .into(),
        );
    }
    for (buffer, entry) in &entry_points {
        branches.push(match buffer {
            Some(_) => format!("        {entry}(st_color, vv_FragNormCoord * RENDERSIZE);\n"),
            None => format!(
                "        {entry}(st_color, vec2(vv_FragNormCoord.x, 1.0 - vv_FragNormCoord.y) * RENDERSIZE);\n        st_color.a = 1.0;\n"
            ),
        });
    }
    glsl += "\n// ---- openVJ ----\nvoid main() {\n    vec4 st_color = vec4(0.0, 0.0, 0.0, 1.0);\n";
    let last = branches.len() - 1;
    for (k, body) in branches.iter().enumerate() {
        let head = match k {
            _ if last == 0 => "    {".to_owned(),
            0 => format!("    if (PASSINDEX == {k}) {{"),
            _ if k == last => "    else {".to_owned(),
            _ => format!("    else if (PASSINDEX == {k}) {{"),
        };
        glsl += &format!("{head}\n{body}    }}\n");
    }
    glsl += "    gl_FragColor = st_color;\n}\n";

    // The header.
    let mut pass_list = Vec::new();
    if is_effect {
        pass_list.push(serde_json::json!({ "TARGET": INPUT_TARGET }));
    }
    for (buffer, _) in &entry_points {
        if let Some(b) = buffer {
            pass_list.push(serde_json::json!({ "TARGET": BUFFER_TARGETS[*b], "PERSISTENT": true, "FLOAT": true }));
        } else {
            pass_list.push(serde_json::json!({}));
        }
    }
    let mut header = Map::new();
    let mut description = meta.description.clone();
    if !meta.id.is_empty() {
        if !description.is_empty() {
            description += "\n\n";
        }
        description += &format!("Imported from {}", view_url(&meta.id));
    }
    header.insert("DESCRIPTION".into(), description.into());
    let credit = match (meta.author.is_empty(), meta.id.is_empty()) {
        (false, _) => format!("{} by {} on Shadertoy", meta.name, meta.author),
        (true, false) => format!("{} on Shadertoy", meta.name),
        (true, true) => format!("{} (from Shadertoy)", meta.name),
    };
    header.insert("CREDIT".into(), credit.into());
    header.insert("LICENSE".into(), DEFAULT_LICENSE.into());
    header.insert("CATEGORIES".into(), serde_json::json!(["Shadertoy"]));
    header.insert("ISFVSN".into(), "2".into());
    let mut ovj = Map::new();
    let id_slug = if meta.id.is_empty() { slug(&meta.name) } else { meta.id.clone() };
    ovj.insert("ID".into(), format!("shadertoy_{id_slug}").into());
    ovj.insert("NAME".into(), meta.name.clone().into());
    if !meta.id.is_empty() {
        ovj.insert("SOURCE".into(), view_url(&meta.id).into());
    }
    header.insert("OPENVJ".into(), Value::Object(ovj));
    let inputs =
        if is_effect { serde_json::json!([{ "NAME": "inputImage", "TYPE": "image" }]) } else { serde_json::json!([]) };
    header.insert("INPUTS".into(), inputs);
    if pass_list.len() > 1 {
        header.insert("PASSES".into(), Value::Array(pass_list));
    }
    let header = serde_json::to_string_pretty(&Value::Object(header)).expect("JSON serialises");
    // A `*/` inside the header's strings would end the comment early.
    let header = header.replace("*/", "*\\/");
    let source = format!("/*{header}*/\n\n{glsl}");

    let base = sanitize_file_name(&meta.name);
    let file_name = if meta.id.is_empty() || base.contains(&meta.id) {
        format!("{base}.fs")
    } else {
        format!("{base} ({}).fs", meta.id)
    };
    Converted { id: meta.id, name: meta.name, file_name, source, is_effect, notes }
}

/// A file name from a shader title: no path separators or characters Finder dislikes.
fn sanitize_file_name(name: &str) -> String {
    let cleaned: String =
        name.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { ' ' } else { c }).collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned: String = cleaned.trim_matches('.').chars().take(80).collect();
    if cleaned.is_empty() { "Shadertoy shader".into() } else { cleaned }
}

fn slug(name: &str) -> String {
    let s: String =
        name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    let s = s.split('_').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("_");
    if s.is_empty() { "shader".into() } else { s }
}

/// Whether `word` appears in `src` as a whole identifier.
pub fn uses_word(src: &str, word: &str) -> bool {
    src.match_indices(word).any(|(i, _)| {
        let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
        !src[..i].ends_with(ident) && !src[i + word.len()..].starts_with(ident)
    })
}

/// What a pass's code declares at the top level.
#[derive(Debug, Default, PartialEq)]
struct Decls {
    /// Functions, global variables and constants, structs.
    names: Vec<String>,
    /// `#define`d macros, undefined again after the pass.
    macros: Vec<String>,
}

/// Finds the names a piece of GLSL declares outside any function: good enough for
/// Shadertoy code, not a full parser.
fn top_level_names(src: &str) -> Decls {
    let mut out = Decls::default();
    let push = |v: &mut Vec<String>, n: &str| {
        if !n.is_empty() && !v.iter().any(|x| x == n) {
            v.push(n.to_owned());
        }
    };
    let tokens = tokenize(src, &mut |name| push(&mut out.macros, name));
    let mut stmt: Vec<&str> = Vec::new();
    let mut depth = 0usize;
    for tok in &tokens {
        let t = tok.as_str();
        if depth > 0 {
            match t {
                "{" => depth += 1,
                "}" => depth -= 1,
                _ => {}
            }
            continue;
        }
        match t {
            "{" => {
                // A function body or a struct.
                if let Some(i) = stmt.iter().position(|t| *t == "struct") {
                    if let Some(n) = stmt.get(i + 1) {
                        push(&mut out.names, n);
                    }
                } else if let Some(i) = stmt.iter().position(|t| *t == "(")
                    && i > 0
                {
                    push(&mut out.names, stmt[i - 1]);
                }
                stmt.clear();
                depth = 1;
            }
            ";" => {
                for n in declared_variables(&stmt) {
                    push(&mut out.names, n);
                }
                stmt.clear();
            }
            _ => stmt.push(t),
        }
    }
    out
}

/// Names declared by a top-level statement such as `const float A = 1.0, B[2] = …;` or a
/// prototype `vec3 f(vec3 p);`.
fn declared_variables<'a>(stmt: &[&'a str]) -> Vec<&'a str> {
    let is_ident = |t: &str| t.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if stmt.first().is_some_and(|t| *t == "precision" || *t == "uniform" || *t == "in" || *t == "out") {
        return vec![];
    }
    let mut names = Vec::new();
    let mut depth = 0i32;
    let mut last_ident: Option<&str> = None;
    let mut idents_in_piece = 0;
    let mut in_init = false;
    let close_piece = |last: &mut Option<&'a str>, count: &mut i32, names: &mut Vec<&'a str>| {
        if *count >= 2
            && let Some(n) = last.take()
        {
            names.push(n);
        }
        *last = None;
        *count = 0;
    };
    for (k, t) in stmt.iter().enumerate() {
        match *t {
            "(" => {
                if depth == 0 && !in_init {
                    // A prototype: the name is the identifier before the parenthesis.
                    if k > 0 && idents_in_piece >= 2 && is_ident(stmt[k - 1]) {
                        names.push(stmt[k - 1]);
                    }
                    return names;
                }
                depth += 1;
            }
            "[" => depth += 1,
            ")" | "]" => depth -= 1,
            "=" if depth == 0 => {
                close_piece(&mut last_ident, &mut idents_in_piece, &mut names);
                in_init = true;
            }
            "," if depth == 0 => {
                if !in_init {
                    close_piece(&mut last_ident, &mut idents_in_piece, &mut names);
                }
                in_init = false;
                // The type carries over to the next name.
                idents_in_piece = 1;
            }
            _ if depth == 0 && !in_init && is_ident(t) => {
                last_ident = Some(t);
                idents_in_piece += 1;
            }
            _ => {}
        }
    }
    if !in_init {
        close_piece(&mut last_ident, &mut idents_in_piece, &mut names);
    }
    names
}

/// Splits GLSL into identifiers, numbers and single punctuation characters, skipping
/// comments and preprocessor lines (reporting `#define` names to `on_define`).
fn tokenize(src: &str, on_define: &mut dyn FnMut(&str)) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut line_start = true;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line_start = true;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        if c == '#' && line_start {
            // A preprocessor line, with backslash continuations.
            let start = i;
            while i < chars.len() && !(chars[i] == '\n' && chars[i - 1] != '\\') {
                i += 1;
            }
            let line: String = chars[start..i.min(chars.len())].iter().collect();
            let rest = line[1..].trim_start();
            if let Some(def) = rest.strip_prefix("define") {
                let name: String =
                    def.trim_start().chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                on_define(&name);
            }
            continue;
        }
        line_start = false;
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(chars[start..i].iter().collect());
        } else if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit)) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.') {
                i += 1;
            }
            out.push(chars[start..i].iter().collect());
        } else {
            out.push(c.to_string());
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isf::EffectDef;

    #[test]
    fn ids_from_links() {
        assert_eq!(parse_id("https://www.shadertoy.com/view/XsXXDn").as_deref(), Some("XsXXDn"));
        assert_eq!(parse_id("  https://www.shadertoy.com/view/XsXXDn/ ").as_deref(), Some("XsXXDn"));
        assert_eq!(parse_id("https://www.shadertoy.com/embed/XsXXDn?gui=true").as_deref(), Some("XsXXDn"));
        assert_eq!(parse_id("https://www.shadertoy.com/api/v1/shaders/XsXXDn?key=x").as_deref(), Some("XsXXDn"));
        assert_eq!(parse_id("4ttSWf").as_deref(), Some("4ttSWf"));
        assert_eq!(parse_id("https://example.com/"), None);
        assert_eq!(parse_id("not an id"), None);
    }

    #[test]
    fn finds_top_level_names() {
        let d = top_level_names(
            "#define PI 3.14159\n// float commented(;\nconst float A = 1.0, B[2] = float[2](1.0, 2.0);\n\
             struct Ray { vec3 o; vec3 d; };\nfloat hash(vec2 p) { float x = 1.0; return x; }\n\
             vec3 shade(Ray r);\nvoid mainImage(out vec4 c, in vec2 f) { c = vec4(hash(f)); }\n\
             mat2 rot(float a) { return mat2(cos(a), -sin(a), sin(a), cos(a)); }\nvec3 g;",
        );
        assert_eq!(d.macros, ["PI"]);
        assert_eq!(d.names, ["A", "B", "Ray", "hash", "shade", "mainImage", "rot", "g"]);
    }

    fn api(passes: Value) -> String {
        serde_json::json!({"Shader": {"ver": "0.1",
            "info": {"id": "AbCd12", "name": "Feedback / Test", "username": "someone", "description": "A test."},
            "renderpass": passes}})
        .to_string()
    }

    #[test]
    fn converts_a_single_pass_shader() {
        let json = api(serde_json::json!([{ "type": "image", "name": "Image", "inputs": [],
            "outputs": [{"id": "4dfGRr", "channel": 0}],
            "code": "void mainImage(out vec4 c, in vec2 f) { c = vec4(f / iResolution.xy, 0.5 + 0.5 * sin(iTime), 1.0); }" }]));
        let c = from_api_json(&json).unwrap();
        assert!(!c.is_effect);
        assert_eq!(c.file_name, "Feedback Test (AbCd12).fs");
        let def = EffectDef::parse(&c.source, "x").unwrap();
        assert_eq!(def.id, "shadertoy_AbCd12");
        assert_eq!(def.name, "Feedback / Test");
        assert_eq!(def.credit, "Feedback / Test by someone on Shadertoy");
        assert_eq!(def.source, "https://www.shadertoy.com/view/AbCd12");
        assert_eq!(def.categories, ["Shadertoy"]);
        assert_eq!(def.passes.len(), 1);
        assert!(def.inputs.is_empty());
        assert!(
            def.glsl
                .contains("mainImage_Image(st_color, vec2(vv_FragNormCoord.x, 1.0 - vv_FragNormCoord.y) * RENDERSIZE)")
        );
    }

    #[test]
    fn buffers_become_persistent_passes_in_order() {
        let helper = "float hash(float x) { return fract(sin(x) * 43758.5); }\n";
        let json = api(serde_json::json!([
            { "type": "image", "name": "Image",
              "inputs": [{"id": "4dXGR8", "ctype": "buffer", "channel": 0}, {"id": "XsXGR8", "ctype": "buffer", "channel": 1},
                         {"id": "xyz", "ctype": "keyboard", "channel": 3}],
              "code": format!("{helper}void mainImage(out vec4 c, in vec2 f) {{ c = texture(iChannel0, f / iResolution.xy) + texture(iChannel1, vec2(0.0)) * hash(1.0); }}") },
            { "type": "buffer", "name": "Buffer B", "outputs": [{"id": "XsXGR8", "channel": 0}],
              "inputs": [{"id": "4sXGR8", "ctype": "musicstream", "channel": 0}],
              "code": "void mainImage(out vec4 c, in vec2 f) { c = texture(iChannel0, vec2(0.1, 0.25)); }" },
            { "type": "buffer", "name": "Buffer A", "outputs": [{"id": "4dXGR8", "channel": 0}],
              "inputs": [{"id": "4dXGR8", "ctype": "buffer", "channel": 0}],
              "code": format!("{helper}void mainImage(out vec4 c, in vec2 f) {{ c = texture(iChannel0, f / iResolution.xy) * 0.99 + hash(iTime); }}") },
            { "type": "common", "name": "Common", "code": "#define SPEED 2.0\n" },
            { "type": "sound", "name": "Sound", "code": "vec2 mainSound(int s, float t) { return vec2(0.0); }" },
        ]));
        let c = from_api_json(&json).unwrap();
        let def = EffectDef::parse(&c.source, "x").unwrap();
        let targets: Vec<_> = def.passes.iter().map(|p| (p.target.clone(), p.persistent)).collect();
        assert_eq!(targets, [(Some("BufferA".into()), true), (Some("BufferB".into()), true), (None, false)]);
        let g = &def.glsl;
        assert!(g.find("// ---- Common").unwrap() < g.find("// ---- Buffer A").unwrap());
        assert!(g.find("// ---- Buffer A").unwrap() < g.find("// ---- Buffer B").unwrap());
        assert!(g.find("// ---- Buffer B").unwrap() < g.find("// ---- Image").unwrap());
        // Both A and Image define hash: each gets its own.
        assert!(g.contains("#define hash hash_BufferA") && g.contains("#define hash hash_Image"));
        assert!(g.contains("#define iChannel0 sampler2D(isf_shadertoy_audio, isf_sampler)"));
        assert!(g.contains("#define iChannel1 sampler2D(BufferB, isf_sampler)"));
        assert!(
            g.contains("if (PASSINDEX == 0) {\n        mainImage_BufferA(st_color, vv_FragNormCoord * RENDERSIZE);")
        );
        assert!(c.notes.iter().any(|n| n.contains("Sound tab")), "{:?}", c.notes);
        assert!(c.notes.iter().any(|n| n.contains("iChannel3 (keyboard)")), "{:?}", c.notes);
    }

    #[test]
    fn video_input_makes_an_effect() {
        let json = api(serde_json::json!([{ "type": "image", "name": "Image",
            "inputs": [{"id": 30, "ctype": "webcam", "channel": 0}],
            "code": "void mainImage(out vec4 c, in vec2 f) { c = texture(iChannel0, f / iResolution.xy).bgra; }" }]));
        let c = from_api_json(&json).unwrap();
        assert!(c.is_effect);
        let def = EffectDef::parse(&c.source, "x").unwrap();
        assert!(def.inputs.iter().any(|i| i.name == "inputImage"));
        assert_eq!(def.passes.len(), 2);
        assert_eq!(def.passes[0].target.as_deref(), Some("ShadertoyInput"));
        assert!(def.glsl.contains("#define iChannel0 sampler2D(ShadertoyInput, isf_sampler)"));
    }

    #[test]
    fn api_errors_are_readable() {
        let e = from_api_json(r#"{"Error":"Shader not found"}"#).unwrap_err();
        assert!(e.contains("Public + API"), "{e}");
        assert!(from_api_json(r#"{"Error":"Invalid key"}"#).unwrap_err().contains("Settings"));
    }

    #[test]
    fn pasted_code() {
        let c = from_image_code(
            "void mainImage(out vec4 c, in vec2 f) { c = texture(iChannel0, f); }",
            "",
            "https://www.shadertoy.com/view/XsXXDn",
        )
        .unwrap();
        assert_eq!(c.id, "XsXXDn");
        assert_eq!(c.name, "Shadertoy XsXXDn");
        assert_eq!(c.file_name, "Shadertoy XsXXDn.fs");
        assert_eq!(c.notes.len(), 1);
        let c =
            from_image_code("// Rainbow rings\nvoid mainImage(out vec4 c, in vec2 f) { c = vec4(1.0); }", "", "XsXXDn")
                .unwrap();
        assert_eq!(c.file_name, "Rainbow rings (XsXXDn).fs");
        let def = EffectDef::parse(&c.source, "x").unwrap();
        assert_eq!(def.source, "https://www.shadertoy.com/view/XsXXDn");
        assert!(from_image_code("void main() {}", "", "").is_err());
        let c = from_image_code("void mainImage(out vec4 c, in vec2 f) { c = vec4(1.0); }", "My: shader", "").unwrap();
        assert_eq!(c.file_name, "My shader.fs");
        assert!(c.source.contains("\"ID\": \"shadertoy_my_shader\""));
    }
}
