/*{
    "DESCRIPTION": "Darken the edges",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Color"],
    "OPENVJ": {"ID": "vignette", "NAME": "Vignette", "RESOLUME": ["Vignette", "Vignetting"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "amount", "LABEL": "Amount", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 70, "UNIT": "%", "RESOLUME": "Amount"},
        {"NAME": "size", "LABEL": "Size", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 55, "UNIT": "%", "RESOLUME": "Size"},
        {"NAME": "softness", "LABEL": "Softness", "TYPE": "float", "MIN": 1, "MAX": 100, "DEFAULT": 45, "UNIT": "%", "RESOLUME": "Softness"},
        {"NAME": "circular", "LABEL": "Round", "TYPE": "bool", "DEFAULT": true}
    ]
}*/

// Round keeps the vignette circular on wide screens; off, it follows the frame's shape.

void main() {
    vec4 c = IMG_THIS_PIXEL(inputImage);
    vec2 d = isf_FragNormCoord - 0.5;
    if (circular) {
        d.x *= RENDERSIZE.x / max(RENDERSIZE.y, 1.0);
    }
    float r = length(d) * 2.0;
    float inner = size / 100.0;
    float v = 1.0 - smoothstep(inner, inner + softness / 100.0, r);
    gl_FragColor = vec4(c.rgb * mix(1.0, v, amount / 100.0), c.a);
}
