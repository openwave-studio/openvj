/*{
    "DESCRIPTION": "Offset the colour channels",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Color"],
    "OPENVJ": {"ID": "rgb_split", "NAME": "RGB Split", "RESOLUME": ["RGBSplit", "ShiftRGB", "DelayRGB", "ChromaticAberration"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "amount", "LABEL": "Amount", "TYPE": "float", "MIN": 0, "MAX": 5, "DEFAULT": 1.5, "UNIT": "%"},
        {"NAME": "angle", "LABEL": "Angle", "TYPE": "float", "MIN": 0, "MAX": 360, "DEFAULT": 0, "UNIT": "°", "WRAP": true}
    ]
}*/

void main() {
    vec2 uv = isf_FragNormCoord;
    float a = radians(angle);
    vec2 d = vec2(cos(a), sin(a)) * amount / 100.0;
    vec4 c = IMG_NORM_PIXEL(inputImage, uv);
    gl_FragColor = vec4(IMG_NORM_PIXEL(inputImage, uv + d).r, c.g, IMG_NORM_PIXEL(inputImage, uv - d).b, c.a);
}
