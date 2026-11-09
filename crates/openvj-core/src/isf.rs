//! Effect definitions in the [ISF](https://isf.video) format: a JSON header in a leading
//! `/* … */` comment, followed by GLSL.
//!
//! openVJ reads the standard keys (`DESCRIPTION`, `CATEGORIES`, `INPUTS`, `PASSES`) and a
//! few of its own that other ISF hosts ignore:
//!
//! * top level `"OPENVJ": {"ID": "blur", "NAME": "Blur", "RESOLUME": ["Blur", "FastBlur"]}`:
//!   a stable id saved in compositions, the display name, and the Resolume effect types
//!   this effect stands in for when importing `.avc` files.
//!   `"HIDDEN": true` keeps an effect out of the Effects tab (the fixed clip and layer
//!   Transform is one).
//! * per input `"UNIT": "°"` (shown after the value) and `"WRAP": true` (angles and hues:
//!   modulation wraps around instead of stopping at the ends).
//! * per input `"RESOLUME": "Strength"`: the Resolume parameter it imports from, in this
//!   input's own units, or `{"NAME": "In / Out", "RANGE": [0, 1]}` when Resolume's value
//!   runs over another range (mapped linearly onto `MIN`..`MAX`).
//!
//! Every input except images is exposed as one or more *channels*, each a normalised
//! 0..1 [`Param`](crate::Param) that modulation drives. A float or a choice is one channel,
//! a 2D point two (`name.x`, `name.y`), a colour four (`name.r` … `name.a`).

use serde_json::Value;

/// One parsed `.fs` file.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectDef {
    /// Stable id saved in compositions.
    pub id: String,
    pub name: String,
    pub description: String,
    pub credit: String,
    pub categories: Vec<String>,
    pub inputs: Vec<InputDef>,
    pub passes: Vec<PassDef>,
    /// Resolume effect types this one replaces on import.
    pub resolume: Vec<String>,
    /// Not listed in the Effects tab.
    pub hidden: bool,
    /// The GLSL after the header.
    pub glsl: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputDef {
    /// GLSL identifier.
    pub name: String,
    /// Shown in the UI.
    pub label: String,
    pub ty: InputType,
    pub unit: String,
    /// Angles and hues: modulation wraps around.
    pub wrap: bool,
    /// The Resolume parameter this input imports from.
    pub resolume: Option<ResolumeParam>,
}

/// Where an input's value comes from in a Resolume `.avc`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolumeParam {
    pub name: String,
    /// Resolume's own range, mapped onto the input's `MIN`..`MAX`. `None`: same units.
    pub range: Option<[f32; 2]>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputType {
    Float {
        min: f32,
        max: f32,
        default: f32,
    },
    /// A choice from a list (`long` in ISF).
    Long {
        values: Vec<i32>,
        labels: Vec<String>,
        default: i32,
    },
    Bool {
        default: bool,
    },
    /// A momentary trigger; true for the frame it fires.
    Event,
    Color {
        default: [f32; 4],
    },
    Point2D {
        min: [f32; 2],
        max: [f32; 2],
        default: [f32; 2],
    },
    Image,
}

/// One render pass. An effect without `PASSES` has one implicit pass writing the output.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PassDef {
    /// Buffer the pass renders into; `None` renders the effect's output.
    pub target: Option<String>,
    /// Keeps its contents between frames (feedback).
    pub persistent: bool,
    /// Size expressions such as `"$WIDTH / 2"`; empty means full size.
    pub width: String,
    pub height: String,
}

/// One modulatable 0..1 value of an input.
#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    /// Key in [`Effect::params`](crate::Effect::params): the input name, plus `.x`/`.r`… for vectors.
    pub key: String,
    pub label: String,
    /// Index into [`EffectDef::inputs`].
    pub input: usize,
    /// Component within the input (0 for scalars).
    pub component: usize,
    /// Normalised default.
    pub default: f32,
}

impl InputDef {
    /// Number of 0..1 channels this input exposes.
    pub fn channel_count(&self) -> usize {
        match self.ty {
            InputType::Image => 0,
            InputType::Point2D { .. } => 2,
            InputType::Color { .. } => 4,
            _ => 1,
        }
    }

    /// Normalised defaults of each channel.
    fn channel_defaults(&self) -> Vec<f32> {
        match &self.ty {
            InputType::Float { min, max, default } => vec![unlerp(*min, *max, *default)],
            InputType::Long { values, default, .. } => {
                let i = values.iter().position(|v| v == default).unwrap_or(0);
                vec![choice_norm(i, values.len())]
            }
            InputType::Bool { default } => vec![if *default { 1.0 } else { 0.0 }],
            InputType::Event => vec![0.0],
            InputType::Color { default } => default.to_vec(),
            InputType::Point2D { min, max, default } => (0..2).map(|i| unlerp(min[i], max[i], default[i])).collect(),
            InputType::Image => vec![],
        }
    }

    /// The value a shader sees for normalised channel value `v`, as a float. Choices give the
    /// selected entry of `VALUES`, bools 0 or 1.
    /// Normalised channel value for a value in real units (the inverse of [`InputDef::real`]).
    pub fn norm(&self, component: usize, r: f32) -> f32 {
        match &self.ty {
            InputType::Float { min, max, .. } => unlerp(*min, *max, r),
            InputType::Long { values, .. } => {
                let i = values
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| (**a as f32 - r).abs().total_cmp(&(**b as f32 - r).abs()))
                    .map_or(0, |(i, _)| i);
                choice_norm(i, values.len())
            }
            InputType::Bool { .. } | InputType::Event => (r >= 0.5) as u8 as f32,
            InputType::Color { .. } => r.clamp(0.0, 1.0),
            InputType::Point2D { min, max, .. } => unlerp(min[component], max[component], r),
            InputType::Image => 0.0,
        }
    }

    /// Normalised value for a value saved by Resolume for this input, if it maps one.
    pub fn from_resolume(&self, component: usize, v: f32) -> Option<f32> {
        let rp = self.resolume.as_ref()?;
        let real = match (rp.range, &self.ty) {
            (Some([a, b]), InputType::Float { min, max, .. }) => min + (max - min) * unlerp(a, b, v),
            (Some([a, b]), InputType::Point2D { min, max, .. }) => {
                min[component] + (max[component] - min[component]) * unlerp(a, b, v)
            }
            (Some([a, b]), _) => unlerp(a, b, v),
            (None, _) => v,
        };
        Some(self.norm(component, real))
    }

    pub fn real(&self, component: usize, v: f32) -> f32 {
        match &self.ty {
            InputType::Float { min, max, .. } => min + (max - min) * v,
            InputType::Long { values, .. } => values.get(choice_index(v, values.len())).copied().unwrap_or(0) as f32,
            InputType::Bool { .. } | InputType::Event => (v >= 0.5) as u8 as f32,
            InputType::Color { .. } => v,
            InputType::Point2D { min, max, .. } => min[component] + (max[component] - min[component]) * v,
            InputType::Image => 0.0,
        }
    }

    /// Human-readable value for normalised channel value `v`.
    pub fn display(&self, component: usize, v: f32) -> String {
        match &self.ty {
            InputType::Long { values, labels, .. } => {
                let i = choice_index(v, values.len());
                labels.get(i).cloned().unwrap_or_else(|| values.get(i).copied().unwrap_or(0).to_string())
            }
            InputType::Bool { .. } | InputType::Event => if v >= 0.5 { "On" } else { "Off" }.into(),
            InputType::Color { .. } => format!("{:.0}%", v * 100.0),
            _ => {
                let r = self.real(component, v);
                let digits = match self.span().abs() {
                    s if s >= 100.0 => 0,
                    s if s >= 10.0 => 1,
                    s if s >= 1.0 => 2,
                    _ => 3,
                };
                format!("{r:.digits$}{}", self.unit)
            }
        }
    }

    fn span(&self) -> f32 {
        match &self.ty {
            InputType::Float { min, max, .. } => max - min,
            InputType::Point2D { min, max, .. } => (max[0] - min[0]).max(max[1] - min[1]),
            _ => 1.0,
        }
    }
}

/// Index of the option a normalised value selects.
pub fn choice_index(v: f32, count: usize) -> usize {
    if count <= 1 {
        return 0;
    }
    ((v.clamp(0.0, 1.0) * (count - 1) as f32).round() as usize).min(count - 1)
}

/// Normalised value that selects option `i` of `count`.
pub fn choice_norm(i: usize, count: usize) -> f32 {
    if count <= 1 { 0.0 } else { i as f32 / (count - 1) as f32 }
}

fn unlerp(min: f32, max: f32, v: f32) -> f32 {
    if max == min { 0.0 } else { ((v - min) / (max - min)).clamp(0.0, 1.0) }
}

impl EffectDef {
    /// Parses an ISF file. `fallback_id` names the effect when the header has no `OPENVJ.ID`
    /// (usually the file stem).
    pub fn parse(source: &str, fallback_id: &str) -> Result<Self, String> {
        let trimmed = source.trim_start();
        let (header, glsl) = match trimmed.strip_prefix("/*") {
            Some(rest) => {
                let end = rest.find("*/").ok_or("the ISF header comment is not closed")?;
                (&rest[..end], &rest[end + 2..])
            }
            None => return Err("missing the ISF header (a /* { … } */ comment at the top)".into()),
        };
        let json: Value = serde_json::from_str(header).map_err(|e| format!("ISF header: {e}"))?;
        let obj = json.as_object().ok_or("the ISF header is not a JSON object")?;
        let ovj = obj.get("OPENVJ").and_then(Value::as_object);
        let id = ovj.and_then(|o| o.get("ID")).and_then(Value::as_str).unwrap_or(fallback_id).to_owned();
        let name = ovj
            .and_then(|o| o.get("NAME"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| title_case(&id));
        let resolume = ovj.and_then(|o| o.get("RESOLUME")).map(strings).unwrap_or_default();
        let text = |k: &str| obj.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();

        let mut inputs = Vec::new();
        for (i, input) in obj.get("INPUTS").and_then(Value::as_array).into_iter().flatten().enumerate() {
            inputs.push(parse_input(input).map_err(|e| format!("input {}: {e}", i + 1))?);
        }
        let passes = match obj.get("PASSES").and_then(Value::as_array) {
            Some(list) if !list.is_empty() => list.iter().map(parse_pass).collect(),
            _ => vec![PassDef::default()],
        };
        Ok(Self {
            id,
            name,
            description: text("DESCRIPTION"),
            credit: text("CREDIT"),
            categories: obj.get("CATEGORIES").map(strings).unwrap_or_default(),
            inputs,
            passes,
            resolume,
            hidden: ovj.and_then(|o| o.get("HIDDEN")).and_then(Value::as_bool).unwrap_or(false),
            glsl: glsl.to_owned(),
        })
    }

    /// Every modulatable channel, in input order.
    pub fn channels(&self) -> Vec<Channel> {
        let mut out = Vec::new();
        for (i, input) in self.inputs.iter().enumerate() {
            let defaults = input.channel_defaults();
            let suffixes: &[&str] = match input.ty {
                InputType::Point2D { .. } => &["x", "y"],
                InputType::Color { .. } => &["r", "g", "b", "a"],
                _ => &[""],
            };
            for (c, default) in defaults.into_iter().enumerate() {
                let (key, label) = if suffixes[c].is_empty() {
                    (input.name.clone(), input.label.clone())
                } else {
                    (
                        format!("{}.{}", input.name, suffixes[c]),
                        format!("{} {}", input.label, suffixes[c].to_uppercase()),
                    )
                };
                out.push(Channel { key, label, input: i, component: c, default });
            }
        }
        out
    }

    /// Image inputs other than `inputImage` (not supported yet).
    pub fn extra_images(&self) -> impl Iterator<Item = &InputDef> {
        self.inputs.iter().filter(|i| i.ty == InputType::Image && i.name != "inputImage")
    }
}

fn strings(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => a.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
        _ => vec![],
    }
}

fn num(v: Option<&Value>) -> Option<f32> {
    match v? {
        Value::Number(n) => n.as_f64().map(|f| f as f32),
        Value::Bool(b) => Some(*b as u8 as f32),
        _ => None,
    }
}

fn vec_n<const N: usize>(v: Option<&Value>) -> Option<[f32; N]> {
    let a = v?.as_array()?;
    let mut out = [0.0; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = num(a.get(i))?;
    }
    Some(out)
}

fn parse_input(v: &Value) -> Result<InputDef, String> {
    let name = v.get("NAME").and_then(Value::as_str).ok_or("missing NAME")?.to_owned();
    let ty_name = v.get("TYPE").and_then(Value::as_str).ok_or("missing TYPE")?;
    let label = v.get("LABEL").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| title_case(&name));
    let ty = match ty_name {
        "float" => {
            let min = num(v.get("MIN")).unwrap_or(0.0);
            let max = num(v.get("MAX")).unwrap_or(1.0);
            InputType::Float {
                min,
                max,
                default: num(v.get("DEFAULT")).unwrap_or(min).clamp(min.min(max), max.max(min)),
            }
        }
        "long" => {
            let values: Vec<i32> = match v.get("VALUES").and_then(Value::as_array) {
                Some(a) => a.iter().filter_map(|x| num(Some(x))).map(|f| f as i32).collect(),
                None => {
                    let min = num(v.get("MIN")).unwrap_or(0.0) as i32;
                    let max = num(v.get("MAX")).unwrap_or(min as f32) as i32;
                    (min..=max.max(min)).collect()
                }
            };
            let labels = v.get("LABELS").map(strings).unwrap_or_else(|| values.iter().map(|x| x.to_string()).collect());
            let default = num(v.get("DEFAULT")).map(|f| f as i32).unwrap_or(values.first().copied().unwrap_or(0));
            InputType::Long { values, labels, default }
        }
        "bool" => InputType::Bool { default: num(v.get("DEFAULT")).unwrap_or(0.0) >= 0.5 },
        "event" => InputType::Event,
        "color" => InputType::Color { default: vec_n::<4>(v.get("DEFAULT")).unwrap_or([0.0, 0.0, 0.0, 1.0]) },
        "point2D" => {
            let min = vec_n::<2>(v.get("MIN")).unwrap_or([0.0, 0.0]);
            let max = vec_n::<2>(v.get("MAX")).unwrap_or([1.0, 1.0]);
            InputType::Point2D { min, max, default: vec_n::<2>(v.get("DEFAULT")).unwrap_or([0.5, 0.5]) }
        }
        "image" => InputType::Image,
        other => return Err(format!("{name}: input type \"{other}\" isn't supported")),
    };
    Ok(InputDef {
        name,
        label,
        ty,
        unit: v.get("UNIT").and_then(Value::as_str).unwrap_or_default().to_owned(),
        wrap: v.get("WRAP").and_then(Value::as_bool).unwrap_or(false),
        resolume: match v.get("RESOLUME") {
            Some(Value::String(s)) => Some(ResolumeParam { name: s.clone(), range: None }),
            Some(Value::Object(o)) => o
                .get("NAME")
                .and_then(Value::as_str)
                .map(|n| ResolumeParam { name: n.to_owned(), range: vec_n::<2>(o.get("RANGE")) }),
            _ => None,
        },
    })
}

fn parse_pass(v: &Value) -> PassDef {
    let expr = |k: &str| match v.get(k) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    PassDef {
        target: v.get("TARGET").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned),
        persistent: v.get("PERSISTENT").is_some_and(|p| p.as_bool().unwrap_or(num(Some(p)).unwrap_or(0.0) != 0.0)),
        width: expr("WIDTH"),
        height: expr("HEIGHT"),
    }
}

fn title_case(id: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for ch in id.chars() {
        if ch == '_' || ch == '-' || ch == ' ' {
            out.push(' ');
            up = true;
        } else if up {
            out.extend(ch.to_uppercase());
            up = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Evaluates an ISF size expression such as `"floor($WIDTH / 4.0)"`. Supports numbers,
/// `$WIDTH`, `$HEIGHT`, `+ - * /`, parentheses and `floor`, `ceil`, `round`, `min`, `max`.
/// Returns `None` for an empty or invalid expression.
pub fn eval_size(expr: &str, width: f32, height: f32) -> Option<f32> {
    struct P<'a> {
        s: &'a [u8],
        i: usize,
        w: f32,
        h: f32,
    }
    impl P<'_> {
        fn ws(&mut self) {
            while self.s.get(self.i).is_some_and(|c| c.is_ascii_whitespace()) {
                self.i += 1;
            }
        }
        fn eat(&mut self, c: u8) -> bool {
            self.ws();
            if self.s.get(self.i) == Some(&c) {
                self.i += 1;
                true
            } else {
                false
            }
        }
        fn expr(&mut self) -> Option<f32> {
            let mut v = self.term()?;
            loop {
                if self.eat(b'+') {
                    v += self.term()?;
                } else if self.eat(b'-') {
                    v -= self.term()?;
                } else {
                    return Some(v);
                }
            }
        }
        fn term(&mut self) -> Option<f32> {
            let mut v = self.atom()?;
            loop {
                if self.eat(b'*') {
                    v *= self.atom()?;
                } else if self.eat(b'/') {
                    v /= self.atom()?;
                } else {
                    return Some(v);
                }
            }
        }
        fn atom(&mut self) -> Option<f32> {
            self.ws();
            if self.eat(b'-') {
                return Some(-self.atom()?);
            }
            if self.eat(b'(') {
                let v = self.expr()?;
                return self.eat(b')').then_some(v);
            }
            let start = self.i;
            if self.s.get(self.i) == Some(&b'$') {
                self.i += 1;
            }
            while self.s.get(self.i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'.') {
                self.i += 1;
            }
            let word = std::str::from_utf8(&self.s[start..self.i]).ok()?;
            match word {
                "$WIDTH" => Some(self.w),
                "$HEIGHT" => Some(self.h),
                "floor" | "ceil" | "round" | "min" | "max" | "abs" => {
                    if !self.eat(b'(') {
                        return None;
                    }
                    let a = self.expr()?;
                    let v = match word {
                        "floor" => a.floor(),
                        "ceil" => a.ceil(),
                        "round" => a.round(),
                        "abs" => a.abs(),
                        _ => {
                            if !self.eat(b',') {
                                return None;
                            }
                            let b = self.expr()?;
                            if word == "min" { a.min(b) } else { a.max(b) }
                        }
                    };
                    self.eat(b')').then_some(v)
                }
                _ => word.parse().ok(),
            }
        }
    }
    if expr.trim().is_empty() {
        return None;
    }
    let mut p = P { s: expr.as_bytes(), i: 0, w: width, h: height };
    let v = p.expr()?;
    p.ws();
    (p.i == p.s.len() && v.is_finite()).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"/*{
        "DESCRIPTION": "Test",
        "CATEGORIES": ["Blur"],
        "OPENVJ": {"ID": "test_fx", "RESOLUME": ["FastBlur"]},
        "INPUTS": [
            {"NAME": "inputImage", "TYPE": "image"},
            {"NAME": "angle", "TYPE": "float", "MIN": 0, "MAX": 360, "DEFAULT": 90, "UNIT": "°", "WRAP": true},
            {"NAME": "mode", "TYPE": "long", "VALUES": [0, 1, 2], "LABELS": ["A", "B", "C"], "DEFAULT": 2},
            {"NAME": "on", "TYPE": "bool", "DEFAULT": true},
            {"NAME": "tint", "TYPE": "color", "DEFAULT": [1, 0.5, 0, 1]},
            {"NAME": "center", "TYPE": "point2D", "MIN": [-1, -1], "MAX": [1, 1], "DEFAULT": [0, 0.5]}
        ],
        "PASSES": [{"TARGET": "buf", "PERSISTENT": true, "WIDTH": "$WIDTH/2"}, {}]
    }*/
    void main() { gl_FragColor = vec4(1.0); }"#;

    #[test]
    fn parses_header_and_channels() {
        let d = EffectDef::parse(SAMPLE, "fallback").unwrap();
        assert_eq!(d.id, "test_fx");
        assert_eq!(d.name, "Test Fx");
        assert_eq!(d.resolume, ["FastBlur"]);
        assert!(d.glsl.contains("void main"));
        assert_eq!(d.passes.len(), 2);
        assert!(d.passes[0].persistent && d.passes[0].target.as_deref() == Some("buf"));
        assert_eq!(d.passes[1], PassDef::default());

        let ch = d.channels();
        let keys: Vec<_> = ch.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["angle", "mode", "on", "tint.r", "tint.g", "tint.b", "tint.a", "center.x", "center.y"]);
        assert_eq!(ch[0].default, 0.25);
        assert_eq!(ch[1].default, 1.0);
        assert_eq!(ch[2].default, 1.0);
        assert_eq!(ch[8].default, 0.75);

        let angle = &d.inputs[1];
        assert!(angle.wrap);
        assert_eq!(angle.real(0, 0.5), 180.0);
        assert_eq!(angle.display(0, 0.5), "180°");
        let mode = &d.inputs[2];
        assert_eq!(mode.display(0, 0.4), "B");
        assert_eq!(mode.real(0, 1.0), 2.0);
        assert_eq!(d.inputs[3].display(0, 0.2), "Off");
    }

    #[test]
    fn missing_header_is_an_error() {
        assert!(EffectDef::parse("void main() {}", "x").is_err());
        assert!(EffectDef::parse("/* not json */", "x").is_err());
    }

    #[test]
    fn size_expressions() {
        assert_eq!(eval_size("$WIDTH/2", 1920.0, 1080.0), Some(960.0));
        assert_eq!(eval_size("floor($HEIGHT / 3.0) + 1", 1920.0, 1080.0), Some(361.0));
        assert_eq!(eval_size("max(1, $WIDTH * 0.25)", 2.0, 2.0), Some(1.0));
        assert_eq!(eval_size("", 1.0, 1.0), None);
        assert_eq!(eval_size("$DEPTH", 1.0, 1.0), None);
    }
}
