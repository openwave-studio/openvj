/*{
    "DESCRIPTION": "Reduce colour levels",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Color"],
    "OPENVJ": {"ID": "posterize", "NAME": "Posterize", "RESOLUME": ["Posterize"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "levels", "LABEL": "Levels", "TYPE": "float", "MIN": 2, "MAX": 16, "DEFAULT": 11.8}
    ]
}*/

void main() {
    vec4 c = IMG_THIS_PIXEL(inputImage);
    float n = floor(levels);
    gl_FragColor = vec4(floor(c.rgb * n + 0.5) / n, c.a);
}
