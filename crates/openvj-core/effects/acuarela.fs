/*{
    "DESCRIPTION": "Watercolour smoke: the picture feeds back into itself, drifting on animated noise",
    "CREDIT": "openVJ, after the Acuarela Wire patch by Darien Brito",
    "ISFVSN": "2",
    "CATEGORIES": ["Feedback", "Stylize"],
    "OPENVJ": {"ID": "acuarela", "NAME": "Acuarela", "RESOLUME": ["Acuarela"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "speed", "LABEL": "Speed", "TYPE": "float", "MIN": 0, "MAX": 40, "DEFAULT": 4, "RESOLUME": "Speed"},
        {"NAME": "detail", "LABEL": "Detail", "TYPE": "float", "MIN": 0, "MAX": 8, "DEFAULT": 4, "RESOLUME": "Detail"},
        {"NAME": "density", "LABEL": "Density", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 4, "RESOLUME": "Density"},
        {"NAME": "strength", "LABEL": "Strength", "TYPE": "float", "MIN": 0, "MAX": 1, "DEFAULT": 0.322, "RESOLUME": "Strength"},
        {"NAME": "gain_x", "LABEL": "Gain X", "TYPE": "float", "MIN": 0, "MAX": 1, "DEFAULT": 0.3},
        {"NAME": "gain_y", "LABEL": "Gain Y", "TYPE": "float", "MIN": 0, "MAX": 1, "DEFAULT": 0.3},
        {"NAME": "gain", "LABEL": "Gain", "TYPE": "float", "MIN": 0, "MAX": 1, "DEFAULT": 0.01},
        {"NAME": "blend", "LABEL": "Blend", "TYPE": "long", "VALUES": [0, 1, 2], "LABELS": ["Lighten", "Screen", "Add"], "DEFAULT": 0},
        {"NAME": "reset", "LABEL": "Reset", "TYPE": "event"}
    ],
    "PASSES": [
        {"TARGET": "feedback", "PERSISTENT": true, "FLOAT": true}
    ]
}*/

// Each frame: lift the input by Gain, blend it with last frame's result faded to 98%,
// then push every pixel along a fractal noise field. Strength scales the push, Gain X/Y
// its direction, Density the noise scale, Detail its octaves and Speed how fast it evolves.

float hash(vec3 p) {
    p = fract(p * 0.3183099 + 0.1);
    p *= 17.0;
    return fract(p.x * p.y * p.z * (p.x + p.y + p.z));
}

float vnoise(vec3 p) {
    vec3 i = floor(p);
    vec3 f = fract(p);
    vec3 w = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(mix(hash(i), hash(i + vec3(1, 0, 0)), w.x), mix(hash(i + vec3(0, 1, 0)), hash(i + vec3(1, 1, 0)), w.x), w.y),
        mix(mix(hash(i + vec3(0, 0, 1)), hash(i + vec3(1, 0, 1)), w.x), mix(hash(i + vec3(0, 1, 1)), hash(i + vec3(1, 1, 1)), w.x), w.y),
        w.z) * 2.0 - 1.0;
}

float fbm(vec3 p) {
    int octaves = int(clamp(round(detail), 1.0, 8.0));
    float sum = 0.0;
    float amp = 0.55;
    for (int i = 0; i < 8; i++) {
        if (i >= octaves) {
            break;
        }
        sum += vnoise(p) * amp;
        p = p * 2.0 + vec3(17.3, 9.1, 0.0);
        amp *= 0.5;
    }
    return sum;
}

vec3 mixer(vec3 a, vec3 b) {
    if (blend == 1) {
        return 1.0 - (1.0 - a) * (1.0 - b);
    }
    if (blend == 2) {
        return a + b;
    }
    return max(a, b);
}

void main() {
    vec2 uv = isf_FragNormCoord;
    vec2 aspect = vec2(RENDERSIZE.x / max(RENDERSIZE.y, 1.0), 1.0);
    vec3 p = vec3(uv * aspect * density, TIME * speed * 0.25);
    vec2 n = clamp(vec2(fbm(p), fbm(p + vec3(31.7, 47.3, 11.1))) + 0.5, 0.0, 1.0);
    vec2 t = uv + n * vec2(gain_x, -gain_y) * strength * 0.005;

    vec4 src = IMG_NORM_PIXEL(inputImage, t);
    vec3 lifted = src.rgb + gain;
    vec3 prev = reset ? vec3(0.0) : IMG_NORM_PIXEL(feedback, t).rgb * 0.98;
    gl_FragColor = vec4(clamp(mixer(lifted, prev), 0.0, 1.0), 1.0);
}
