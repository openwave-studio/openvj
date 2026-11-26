/*{
    "DESCRIPTION": "Make one colour transparent (green or blue screen)",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Key"],
    "OPENVJ": {"ID": "chroma_key", "NAME": "Chroma Key", "RESOLUME": ["ChromaKey", "Chroma Key", "ChromaKeyer", "GreenScreen"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "key", "LABEL": "Key colour", "TYPE": "color", "DEFAULT": [0, 1, 0, 1], "RESOLUME": "Color"},
        {"NAME": "tolerance", "LABEL": "Tolerance", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 25, "UNIT": "%", "RESOLUME": "Tolerance"},
        {"NAME": "softness", "LABEL": "Softness", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 10, "UNIT": "%", "RESOLUME": "Smoothness"},
        {"NAME": "spill", "LABEL": "Spill", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 50, "UNIT": "%", "RESOLUME": "Spill"}
    ]
}*/

// Compares colours by their chroma (CbCr), so shadows and highlights on the screen key
// the same. Spill takes the key colour's tint out of the edges that stay.

vec2 chroma(vec3 c) {
    float y = dot(c, vec3(0.2126, 0.7152, 0.0722));
    return vec2((c.b - y) / 1.8556, (c.r - y) / 1.5748);
}

void main() {
    vec4 c = IMG_THIS_PIXEL(inputImage);
    float d = distance(chroma(c.rgb), chroma(key.rgb));
    float t = tolerance / 100.0 * 0.4;
    float s = max(softness / 100.0 * 0.4, 0.001);
    float keep = smoothstep(t, t + s, d);
    // Spill: near the key colour, pull the pixel toward grey.
    float near = 1.0 - smoothstep(t, t + s + 0.15, d);
    float luma = dot(c.rgb, vec3(0.2126, 0.7152, 0.0722));
    vec3 rgb = mix(c.rgb, vec3(luma), near * spill / 100.0);
    gl_FragColor = vec4(rgb, c.a * keep);
}
