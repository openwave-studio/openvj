/*{
    "DESCRIPTION": "Two colours: everything above a brightness, and everything below",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Color"],
    "OPENVJ": {"ID": "threshold", "NAME": "Threshold", "RESOLUME": ["Threshold", "Thresh", "BlackWhite"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "level", "LABEL": "Level", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 50, "UNIT": "%", "RESOLUME": "Threshold"},
        {"NAME": "softness", "LABEL": "Softness", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 2, "UNIT": "%"},
        {"NAME": "dark", "LABEL": "Dark", "TYPE": "color", "DEFAULT": [0, 0, 0, 1]},
        {"NAME": "light", "LABEL": "Light", "TYPE": "color", "DEFAULT": [1, 1, 1, 1]}
    ]
}*/

void main() {
    vec4 c = IMG_THIS_PIXEL(inputImage);
    float luma = dot(c.rgb, vec3(0.2126, 0.7152, 0.0722));
    float t = level / 100.0;
    float s = max(softness / 100.0, 0.001);
    vec4 o = mix(dark, light, smoothstep(t - s * 0.5, t + s * 0.5, luma));
    gl_FragColor = vec4(o.rgb, c.a * o.a);
}
