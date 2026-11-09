/*{
    "DESCRIPTION": "Sine-wave displacement",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Distortion"],
    "OPENVJ": {"ID": "wave", "NAME": "Wave", "RESOLUME": ["Wave", "WaveWarp", "Displace", "Ripples"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "amount", "LABEL": "Amount", "TYPE": "float", "MIN": 0, "MAX": 10, "DEFAULT": 3, "UNIT": "%"},
        {"NAME": "frequency", "LABEL": "Frequency", "TYPE": "float", "MIN": 1, "MAX": 40, "DEFAULT": 16.6},
        {"NAME": "speed", "LABEL": "Speed", "TYPE": "float", "MIN": 0, "MAX": 12, "DEFAULT": 3.6},
        {"NAME": "direction", "LABEL": "Direction", "TYPE": "long", "VALUES": [0, 1], "LABELS": ["Horizontal", "Vertical"], "DEFAULT": 0}
    ]
}*/

void main() {
    vec2 uv = isf_FragNormCoord;
    vec2 t = uv;
    float phase = TIME * speed;
    if (direction == 0) {
        t.x += sin(uv.y * frequency + phase) * amount / 100.0;
    } else {
        t.y += sin(uv.x * frequency + phase) * amount / 100.0;
    }
    gl_FragColor = IMG_NORM_PIXEL(inputImage, clamp(t, 0.0, 1.0));
}
