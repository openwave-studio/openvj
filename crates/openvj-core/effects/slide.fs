/*{
    "DESCRIPTION": "Scroll the picture, wrapping at the edges",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Geometry"],
    "OPENVJ": {"ID": "slide", "NAME": "Slide", "RESOLUME": ["Slide", "Scroll", "Shift"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "x", "LABEL": "Offset X", "TYPE": "float", "MIN": -100, "MAX": 100, "DEFAULT": 0, "UNIT": "%", "RESOLUME": "X"},
        {"NAME": "y", "LABEL": "Offset Y", "TYPE": "float", "MIN": -100, "MAX": 100, "DEFAULT": 0, "UNIT": "%", "RESOLUME": "Y"},
        {"NAME": "speed_x", "LABEL": "Speed X", "TYPE": "float", "MIN": -4, "MAX": 4, "DEFAULT": 0.25, "UNIT": "/beat"},
        {"NAME": "speed_y", "LABEL": "Speed Y", "TYPE": "float", "MIN": -4, "MAX": 4, "DEFAULT": 0, "UNIT": "/beat"}
    ]
}*/

// Speed is in screens per beat, so a scroll stays locked to the tempo.

void main() {
    vec2 off = vec2(x, y) / 100.0 + vec2(speed_x, speed_y) * BEAT;
    gl_FragColor = IMG_NORM_PIXEL(inputImage, fract(isf_FragNormCoord - off));
}
