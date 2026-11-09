/*{
    "DESCRIPTION": "Edge detection",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Stylize"],
    "OPENVJ": {"ID": "edges", "NAME": "Edges", "RESOLUME": ["EdgeDetection", "Edges", "EdgeDetect", "Sobel"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "strength", "LABEL": "Strength", "TYPE": "float", "MIN": 1, "MAX": 8, "DEFAULT": 4.5, "UNIT": "×", "RESOLUME": {"NAME": "Multiplier", "RANGE": [0, 0.5]}},
        {"NAME": "keep_color", "LABEL": "Keep colour", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 0, "UNIT": "%"},
        {"NAME": "use_color", "LABEL": "Use colour", "TYPE": "bool", "DEFAULT": false, "RESOLUME": "Use Color"},
        {"NAME": "color", "LABEL": "Colour", "TYPE": "color", "DEFAULT": [1, 1, 1, 1], "RESOLUME": "Color"}
    ]
}*/

float luma(vec2 uv) {
    return dot(IMG_NORM_PIXEL(inputImage, uv).rgb, vec3(0.299, 0.587, 0.114));
}

// Sobel.
void main() {
    vec2 uv = isf_FragNormCoord;
    vec2 d = 1.0 / RENDERSIZE;
    float tl = luma(uv + vec2(-d.x, -d.y));
    float tc = luma(uv + vec2(0.0, -d.y));
    float tr = luma(uv + vec2(d.x, -d.y));
    float ml = luma(uv + vec2(-d.x, 0.0));
    float mr = luma(uv + vec2(d.x, 0.0));
    float bl = luma(uv + vec2(-d.x, d.y));
    float bc = luma(uv + vec2(0.0, d.y));
    float br = luma(uv + vec2(d.x, d.y));
    float gx = tr + 2.0 * mr + br - tl - 2.0 * ml - bl;
    float gy = bl + 2.0 * bc + br - tl - 2.0 * tc - tr;
    float e = clamp(length(vec2(gx, gy)) * strength, 0.0, 1.0);
    vec4 c = IMG_THIS_PIXEL(inputImage);
    vec3 edge = use_color ? color.rgb * e : vec3(e);
    gl_FragColor = vec4(mix(edge, c.rgb * e, keep_color / 100.0), c.a);
}
