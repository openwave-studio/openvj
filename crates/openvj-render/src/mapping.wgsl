// Output mapping: draws a slice's warp mesh onto a screen, sampling the composition.

struct Mapping {
    // xy: screen size in pixels, zw: composition size in pixels.
    sizes: vec4f,
    // x: screen opacity, y: test card (1 on), z: brightness, w: contrast.
    look: vec4f,
    // rgb: red, green and blue offsets.
    rgb: vec4f,
    // Test card colour of this slice.
    card: vec4f,
};

@group(0) @binding(0) var<uniform> u: Mapping;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var comp: texture_2d<f32>;
@group(0) @binding(3) var screen_mask: texture_2d<f32>;
@group(0) @binding(4) var slice_mask: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4f,
    // Composition pixels.
    @location(0) src: vec2f,
};

@vertex
fn vs_map(@location(0) pos: vec2f, @location(1) src: vec2f) -> VsOut {
    var o: VsOut;
    let ndc = pos / u.sizes.xy * 2.0 - 1.0;
    o.pos = vec4f(ndc.x, -ndc.y, 0.0, 1.0);
    o.src = src;
    return o;
}

fn test_card(px: vec2f) -> vec3f {
    // A grid every 1/16 of the composition's width, with a brighter centre cross.
    let step = u.sizes.z / 16.0;
    let g = abs(fract(px / step + 0.5) - 0.5) * step;
    let line = min(g.x, g.y) < 1.5;
    let c = abs(px - u.sizes.zw * 0.5);
    let cross = min(c.x, c.y) < 2.5;
    var col = u.card.rgb * 0.35;
    if line { col = vec3f(0.85); }
    if cross { col = vec3f(1.0); }
    return col;
}

@fragment
fn fs_map(in: VsOut) -> @location(0) vec4f {
    let uv = in.src / u.sizes.zw;
    var c: vec3f;
    if u.look.y > 0.5 {
        c = test_card(in.src);
    } else {
        c = textureSampleLevel(comp, samp, uv, 0.0).rgb;
    }
    c = (c - 0.5) * (1.0 + u.look.w) + 0.5 + u.look.z;
    c = c * (1.0 + u.rgb.rgb);
    let m = textureSampleLevel(screen_mask, samp, in.pos.xy / u.sizes.xy, 0.0).r
        * textureSampleLevel(slice_mask, samp, uv, 0.0).r;
    return vec4f(clamp(c, vec3f(0.0), vec3f(1.0)) * u.look.x, m);
}
