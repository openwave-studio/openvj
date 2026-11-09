/*{
    "DESCRIPTION": "Kaleidoscope of the input",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Geometry"],
    "OPENVJ": {"ID": "kaleido", "NAME": "Kaleido", "RESOLUME": ["Kaleidoscope", "Kaleido", "PolarKaleido"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "segments", "LABEL": "Segments", "TYPE": "float", "MIN": 2, "MAX": 16, "DEFAULT": 6.2},
        {"NAME": "rotate", "LABEL": "Rotate", "TYPE": "float", "MIN": 0, "MAX": 360, "DEFAULT": 0, "UNIT": "°", "WRAP": true, "RESOLUME": {"NAME": "Rotation", "RANGE": [0, 1]}},
        {"NAME": "zoom", "LABEL": "Zoom", "TYPE": "float", "MIN": 50, "MAX": 200, "DEFAULT": 125, "UNIT": "%", "RESOLUME": {"NAME": "Zoom", "RANGE": [0, 1]}}
    ]
}*/

void main() {
    vec2 aspect = vec2(RENDERSIZE.x / max(RENDERSIZE.y, 1.0), 1.0);
    vec2 p = (isf_FragNormCoord - 0.5) * aspect;
    float r = length(p) / (zoom / 100.0);
    float seg = 6.2831853 / floor(segments);
    float a = atan(p.y, p.x) + radians(rotate);
    a = abs(a - seg * floor(a / seg) - seg * 0.5);
    p = vec2(cos(a), sin(a)) * r;
    gl_FragColor = IMG_NORM_PIXEL(inputImage, clamp(p / aspect + 0.5, 0.0, 1.0));
}
