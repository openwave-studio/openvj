/*{
    "DESCRIPTION": "Zoom into the picture forever, tiled inside itself",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Geometry"],
    "OPENVJ": {"ID": "infinite_zoom", "NAME": "Infinite Zoom", "RESOLUME": ["InfiniteZoom", "Infinite Zoom", "Droste"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "speed", "LABEL": "Speed", "TYPE": "float", "MIN": -2, "MAX": 2, "DEFAULT": 0.25, "UNIT": "/beat", "RESOLUME": "Speed"},
        {"NAME": "scale", "LABEL": "Step", "TYPE": "float", "MIN": 1.5, "MAX": 8, "DEFAULT": 2, "UNIT": "×"}
    ]
}*/

// Three copies of the picture, each Step times smaller than the last, scaled up together
// at Speed steps per beat. Each fades in as it grows from the centre and out as it leaves,
// so the loop is seamless.

void main() {
    vec2 uv = isf_FragNormCoord - 0.5;
    float f = fract(BEAT * speed);
    vec4 sum = vec4(0.0);
    float total = 0.0;
    for (int i = 0; i < 3; i++) {
        float level = float(i) + f;
        float zoom = pow(scale, level - 1.0);
        float w = sin(3.14159265 * level / 3.0);
        sum += IMG_NORM_PIXEL(inputImage, fract(uv / zoom + 0.5)) * w;
        total += w;
    }
    gl_FragColor = sum / max(total, 0.001);
}
