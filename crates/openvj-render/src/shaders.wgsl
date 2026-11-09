// openVJ engine shaders. Every pass draws one fullscreen triangle.
//   fs_generator: built-in procedural clips (switch on u.kind, see GeneratorKind::shader_id)
//   fs_media:     image/video frame scaled into the target (cover fit); how to read the
//                 texture(s) in u.mode (see MEDIA_* below), the used part of them in u.crop
//   fs_blend:     tex_a = base, tex_b = layer; blend mode in u.mode (see BlendMode::shader_id)
//   Effects are ISF files compiled separately (see isf.rs).

struct Uniforms {
    res: vec2f,
    time: f32,
    beat: f32,
    params: vec4f,
    kind: u32,
    mode: u32,
    opacity: f32,
    _pad: f32,
    uv_scale: vec2f,
    uv_offset: vec2f,
    crop: vec4f,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var tex_a: texture_2d<f32>;
@group(0) @binding(3) var tex_b: texture_2d<f32>;

struct VOut {
    @builtin(position) pos: vec4f,
    // Top-left origin, matching texture coordinates.
    @location(0) uv: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VOut {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VOut;
    o.pos = vec4f(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2f(p.x, 1.0 - p.y);
    return o;
}

const TAU: f32 = 6.2831853;

fn hsv(h: f32, s: f32, v: f32) -> vec3f {
    let k = vec3f(1.0, 2.0 / 3.0, 1.0 / 3.0);
    let p = abs(fract(vec3f(h) + k) * 6.0 - 3.0);
    return v * mix(vec3f(1.0), clamp(p - 1.0, vec3f(0.0), vec3f(1.0)), s);
}

fn palette(t: f32, hue: f32) -> vec3f {
    return 0.5 + 0.5 * cos(TAU * (vec3f(t) + vec3f(hue, hue + 0.33, hue + 0.67)));
}

fn hash(p: vec2f) -> f32 {
    return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453);
}

fn vnoise(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    let a = mix(hash(i), hash(i + vec2f(1.0, 0.0)), w.x);
    let b = mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), w.x);
    return mix(a, b, w.y);
}

fn rot(a: f32) -> mat2x2f {
    let c = cos(a);
    let s = sin(a);
    return mat2x2f(c, -s, s, c);
}

fn wrap(x: f32, m: f32) -> f32 {
    return x - m * floor(x / m);
}

@fragment
fn fs_generator(in: VOut) -> @location(0) vec4f {
    let aspect = u.res.x / max(u.res.y, 1.0);
    let p = (vec2f(in.uv.x, 1.0 - in.uv.y) - 0.5) * vec2f(aspect, 1.0) * 2.0;
    let t = u.time;
    let b = u.beat;
    let k = u.params;
    var col = vec3f(0.0);

    switch u.kind {
        // Plasma
        case 0u: {
            let q = p * mix(1.5, 10.0, k.x);
            let w = vec2f(sin(t * 0.5), cos(t * 0.3)) * k.w * 4.0;
            var v = sin(q.x + t) + sin(q.y * 0.8 + t * 1.3) + sin((q.x + q.y) * 0.7 + t * 0.7)
                + sin(length(q + w) * 1.5);
            v = v * 0.25;
            col = palette(v * mix(0.5, 2.0, k.z) + t * 0.05, k.y);
        }
        // Tunnel
        case 1u: {
            let r = max(length(p), 0.001);
            let a = atan2(p.y, p.x);
            let z = 1.0 / r;
            let stripes = 0.5 + 0.5 * sin(a * 6.0 + z * (k.x - 0.5) * 4.0 + t);
            let band = 0.5 + 0.5 * sin(z * mix(2.0, 16.0, k.y) * 0.25 - t * 4.0);
            col = palette(z * 0.1 + t * 0.1, k.z) * band * stripes + vec3f(pow(band, 8.0) * k.w);
            col = col * clamp(r * 2.0, 0.0, 1.0);
        }
        // Bars
        case 2u: {
            let count = mix(2.0, 24.0, k.x) * 0.25;
            let d = (rot(k.y * 3.14159) * p).x * count + b * 0.25;
            let on = step(fract(d), mix(0.1, 0.9, k.w));
            let flash = 1.0 - fract(b) * 0.6;
            col = hsv(k.z + floor(d) * 0.07, 0.8, 1.0) * on * flash;
        }
        // Rings
        case 3u: {
            let r = length(p);
            let x = fract(r * mix(1.0, 10.0, k.x) - t * 0.5);
            let th = mix(0.05, 0.9, k.y) * 0.5;
            let ring = 1.0 - smoothstep(th - 0.03, th, abs(x - 0.5));
            let kick = pow(1.0 - fract(b), 3.0) * k.w;
            col = hsv(k.z + r * 0.2, 0.7, 1.0) * ring * (0.45 + kick);
        }
        // Noise
        case 4u: {
            var q = p * mix(1.0, 8.0, k.x);
            let octaves = i32(mix(1.0, 6.0, k.y));
            var f = 0.0;
            var amp = 0.5;
            for (var i = 0; i < 6; i++) {
                if i >= octaves {
                    break;
                }
                f += amp * vnoise(q + vec2f(t * 0.3, -t * 0.2));
                q = rot(0.6) * q * 2.0;
                amp *= 0.5;
            }
            f = clamp((f - 0.5) * mix(1.0, 4.0, k.w) + 0.5, 0.0, 1.0);
            col = palette(f + t * 0.02, k.z) * f;
        }
        // Kaleido
        case 5u: {
            let q0 = rot(t * (k.w - 0.5) * 2.0) * p;
            let r = length(q0);
            let seg = TAU / floor(mix(3.0, 16.0, k.x));
            let a = abs(wrap(atan2(q0.y, q0.x), seg) - seg * 0.5);
            let q = vec2f(cos(a), sin(a)) * r * mix(1.0, 6.0, k.y);
            let v = sin(q.x * 3.0 + t) * cos(q.y * 3.0 - t * 0.7) + sin(length(q) * 4.0 - t * 2.0);
            col = palette(v * 0.3 + r * 0.3, k.z) * (0.6 + 0.4 * v);
        }
        // Gradient
        case 6u: {
            let ang = k.x * TAU;
            let d = dot(p, vec2f(cos(ang), sin(ang)));
            col = hsv(k.z + d * mix(0.05, 1.0, k.y) + t * 0.05, k.w, 1.0);
        }
        // Strobe: rate picks 1/4, 1/2, 1, 2 or 4 flashes per beat.
        case 7u: {
            let steps = exp2(floor(k.x * 4.99) - 2.0);
            let on = step(fract(b * steps), mix(0.05, 0.9, k.y));
            col = hsv(k.z, k.w, 1.0) * on;
        }
        // Checker
        case 8u: {
            var q = rot(t * (k.y - 0.5)) * p;
            q += sin(q.yx * 3.0 + t) * k.w * 0.3;
            let size = mix(2.0, 20.0, k.x) * 0.5;
            let c = floor(q.x * size) + floor(q.y * size);
            let on = abs(wrap(c, 2.0));
            col = mix(vec3f(0.02), hsv(k.z + t * 0.03, 0.7, 1.0), on);
        }
        default: {
            col = vec3f(1.0, 0.0, 1.0);
        }
    }
    return vec4f(clamp(col, vec3f(0.0), vec3f(1.0)), 1.0);
}

// Media texture layouts (u.mode of fs_media); see MediaLayout in lib.rs.
const MEDIA_RGBA: u32 = 0u;
const MEDIA_OPAQUE: u32 = 1u;
const MEDIA_PREMULTIPLIED: u32 = 2u;
const MEDIA_YCOCG: u32 = 3u;
const MEDIA_YCOCG_ALPHA: u32 = 4u;
const MEDIA_SCALED_YCOCG: u32 = 5u;
const MEDIA_SCALED_YCOCG_ALPHA: u32 = 6u;

@fragment
fn fs_media(in: VOut) -> @location(0) vec4f {
    let uv = (in.uv - 0.5) * u.uv_scale + 0.5 + u.uv_offset;
    // Stay half a texel inside the cropped area so padding never bleeds in.
    let edge_a = u.crop.xy - 0.5 / vec2f(textureDimensions(tex_a));
    var c = textureSampleLevel(tex_a, samp, min(uv * u.crop.xy, edge_a), 0.0);
    switch u.mode {
        case MEDIA_OPAQUE: {
            c.a = 1.0;
        }
        case MEDIA_PREMULTIPLIED: {
            c = vec4f(select(vec3f(0.0), min(c.rgb / c.a, vec3f(1.0)), c.a > 0.0), c.a);
        }
        case MEDIA_YCOCG, MEDIA_YCOCG_ALPHA: {
            // Luma (and alpha) at full size in tex_a, Co/Cg at half size in tex_b.
            let edge_b = u.crop.zw - 0.5 / vec2f(textureDimensions(tex_b));
            let cocg = textureSampleLevel(tex_b, samp, min(uv * u.crop.zw, edge_b), 0.0).rg - vec2f(128.0 / 255.0);
            let y = c.r;
            let rgb = vec3f(y + cocg.x - cocg.y, y + cocg.y, y - cocg.x - cocg.y);
            c = vec4f(clamp(rgb, vec3f(0.0), vec3f(1.0)), select(1.0, c.g, u.mode == MEDIA_YCOCG_ALPHA));
        }
        case MEDIA_SCALED_YCOCG, MEDIA_SCALED_YCOCG_ALPHA: {
            // HAP Q: R = Co, G = Cg, B = scale, A = luma (van Waveren & Castaño's YCoCg-DXT).
            let scale = c.b * (255.0 / 8.0) + 1.0;
            let cocg = (c.rg - vec2f(128.0 / 255.0)) / scale;
            let y = c.a;
            let rgb = vec3f(y + cocg.x - cocg.y, y + cocg.y, y - cocg.x - cocg.y);
            var a = 1.0;
            if u.mode == MEDIA_SCALED_YCOCG_ALPHA {
                // HAP Q Alpha: BC4 alpha at full size in tex_b.
                let edge_b = u.crop.zw - 0.5 / vec2f(textureDimensions(tex_b));
                a = textureSampleLevel(tex_b, samp, min(uv * u.crop.zw, edge_b), 0.0).r;
            }
            c = vec4f(clamp(rgb, vec3f(0.0), vec3f(1.0)), a);
        }
        default: {}
    }
    let inside = all(uv >= vec2f(0.0)) && all(uv <= vec2f(1.0));
    return select(vec4f(0.0), c, inside);
}

const MODE_MASTER: u32 = 100u;
const MODE_MIX: u32 = 101u;

@fragment
fn fs_blend(in: VOut) -> @location(0) vec4f {
    let base = textureSampleLevel(tex_a, samp, in.uv, 0.0);
    let layer = textureSampleLevel(tex_b, samp, in.uv, 0.0);
    let b = base.rgb;
    let l = layer.rgb;

    if u.mode == MODE_MASTER {
        return vec4f(b * u.opacity, 1.0);
    }
    if u.mode == MODE_MIX {
        return mix(base, layer, u.opacity);
    }

    var c = l;
    switch u.mode {
        case 1u: { c = b + l; }
        case 2u: { c = 1.0 - (1.0 - b) * (1.0 - l); }
        case 3u: { c = b * l; }
        case 4u: { c = select(1.0 - 2.0 * (1.0 - b) * (1.0 - l), 2.0 * b * l, b < vec3f(0.5)); }
        case 5u: { c = abs(b - l); }
        case 6u: { c = max(b, l); }
        case 7u: { c = min(b, l); }
        case 8u: { c = b - l; }
        default: {}
    }
    // Straight-alpha "source over" with the blend mode applied where the base has coverage
    // (W3C compositing). Over an opaque base this is mix(b, c, a); over a transparent one
    // (the first layer of a group) it is just the layer.
    let a = layer.a * u.opacity;
    let out_a = a + base.a * (1.0 - a);
    if out_a <= 0.0 {
        return vec4f(0.0);
    }
    let blended = mix(l, c, base.a);
    let rgb = (blended * a + b * base.a * (1.0 - a)) / out_a;
    return vec4f(clamp(rgb, vec3f(0.0), vec3f(1.0)), out_a);
}
