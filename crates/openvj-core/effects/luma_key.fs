/*{
    "DESCRIPTION": "Make dark (or bright) parts transparent",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Key"],
    "OPENVJ": {"ID": "luma_key", "NAME": "Luma Key", "RESOLUME": ["LumaKey", "Luma Key", "LumaKeyer"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "threshold", "LABEL": "Threshold", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 20, "UNIT": "%", "RESOLUME": "Threshold"},
        {"NAME": "softness", "LABEL": "Softness", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 10, "UNIT": "%", "RESOLUME": "Smoothness"},
        {"NAME": "invert", "LABEL": "Invert", "TYPE": "bool", "DEFAULT": false, "RESOLUME": "Invert"}
    ]
}*/

// Pixels darker than Threshold turn transparent (brighter ones with Invert), fading over
// Softness. The layer's blend mode then shows what is below.

void main() {
    vec4 c = IMG_THIS_PIXEL(inputImage);
    float luma = dot(c.rgb, vec3(0.2126, 0.7152, 0.0722));
    float t = threshold / 100.0;
    float s = max(softness / 100.0, 0.001);
    float keep = smoothstep(t - s * 0.5, t + s * 0.5, luma);
    if (invert) {
        keep = 1.0 - keep;
    }
    gl_FragColor = vec4(c.rgb, c.a * keep);
}
