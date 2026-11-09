/*{
    "DESCRIPTION": "Zoom, rotate and move",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Geometry"],
    "OPENVJ": {"ID": "transform", "NAME": "Transform"},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "zoom", "LABEL": "Zoom", "TYPE": "float", "MIN": -100, "MAX": 100, "DEFAULT": 0, "UNIT": "%"},
        {"NAME": "rotate", "LABEL": "Rotate", "TYPE": "float", "MIN": -180, "MAX": 180, "DEFAULT": 0, "UNIT": "°", "WRAP": true},
        {"NAME": "x", "LABEL": "X", "TYPE": "float", "MIN": -50, "MAX": 50, "DEFAULT": 0, "UNIT": "%"},
        {"NAME": "y", "LABEL": "Y", "TYPE": "float", "MIN": -50, "MAX": 50, "DEFAULT": 0, "UNIT": "%"}
    ]
}*/

// Zoom: ±100% is 4× in or out. X and Y move by a share of the frame.
void main() {
    vec2 aspect = vec2(RENDERSIZE.x / max(RENDERSIZE.y, 1.0), 1.0);
    vec2 p = (isf_FragNormCoord - 0.5) * aspect;
    float a = radians(rotate);
    p = mat2(cos(a), -sin(a), sin(a), cos(a)) * p;
    p /= exp2(zoom / 50.0);
    vec2 t = p / aspect + 0.5 - vec2(x, y) / 100.0;
    bool inside = all(greaterThanEqual(t, vec2(0.0))) && all(lessThanEqual(t, vec2(1.0)));
    gl_FragColor = inside ? IMG_NORM_PIXEL(inputImage, t) : vec4(0.0);
}
