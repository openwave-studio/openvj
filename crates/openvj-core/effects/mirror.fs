/*{
    "DESCRIPTION": "Mirror horizontally, vertically or both",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Geometry"],
    "OPENVJ": {"ID": "mirror", "NAME": "Mirror", "RESOLUME": ["Mirror", "MirrorQuad"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "mode", "LABEL": "Mode", "TYPE": "long", "VALUES": [0, 1, 2], "LABELS": ["Horizontal", "Vertical", "Both"], "DEFAULT": 0},
        {"NAME": "offset", "LABEL": "Offset", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 50, "UNIT": "%"}
    ]
}*/

void main() {
    vec2 t = isf_FragNormCoord;
    float o = offset / 100.0;
    if (mode != 1 && t.x > o) {
        t.x = 2.0 * o - t.x;
    }
    if (mode != 0 && t.y > o) {
        t.y = 2.0 * o - t.y;
    }
    gl_FragColor = IMG_NORM_PIXEL(inputImage, clamp(t, 0.0, 1.0));
}
