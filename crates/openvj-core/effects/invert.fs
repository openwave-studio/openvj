/*{
    "DESCRIPTION": "Invert colours",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Color"],
    "OPENVJ": {"ID": "invert", "NAME": "Invert", "RESOLUME": ["Invert", "InvertRGB", "InvertColors"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"}
    ]
}*/

void main() {
    vec4 c = IMG_THIS_PIXEL(inputImage);
    gl_FragColor = vec4(1.0 - c.rgb, c.a);
}
