/*{
    "DESCRIPTION": "Each frame leaves a fading trail",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Feedback"],
    "OPENVJ": {"ID": "trails", "NAME": "Trails", "RESOLUME": ["Trails", "Trail", "Feedback"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "feedback", "LABEL": "Feedback", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 85, "UNIT": "%", "RESOLUME": "Feedback"},
        {"NAME": "mode", "LABEL": "Mode", "TYPE": "long", "VALUES": [0, 1], "LABELS": ["Lighten", "Mix"], "DEFAULT": 0},
        {"NAME": "reset", "LABEL": "Reset", "TYPE": "event"}
    ],
    "PASSES": [
        {"TARGET": "trail", "PERSISTENT": true, "FLOAT": true}
    ]
}*/

// Lighten keeps whichever is brighter, the new frame or the faded trail (light painting);
// Mix blends the new frame over the trail (motion blur).

void main() {
    vec4 src = IMG_THIS_PIXEL(inputImage);
    vec4 prev = reset ? vec4(0.0) : IMG_THIS_PIXEL(trail);
    float f = feedback / 100.0;
    vec4 out_c = mode == 1 ? mix(src, prev, f) : max(src, prev * f);
    gl_FragColor = clamp(out_c, 0.0, 1.0);
}
