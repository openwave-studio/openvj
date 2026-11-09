/*{
    "DESCRIPTION": "Soft blur",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Blur"],
    "OPENVJ": {"ID": "blur", "NAME": "Blur", "RESOLUME": ["Blur", "FastBlur", "GaussianBlur", "RadialBlur", "TiltShift"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "radius", "LABEL": "Radius", "TYPE": "float", "MIN": 0, "MAX": 12, "DEFAULT": 3.6, "UNIT": " px"}
    ]
}*/

// 5×5 taps spread over the radius.
void main() {
    vec2 step = radius * 0.5 / RENDERSIZE;
    vec4 acc = vec4(0.0);
    for (int y = -2; y <= 2; y++) {
        for (int x = -2; x <= 2; x++) {
            acc += IMG_NORM_PIXEL(inputImage, isf_FragNormCoord + vec2(float(x), float(y)) * step);
        }
    }
    gl_FragColor = acc / 25.0;
}
