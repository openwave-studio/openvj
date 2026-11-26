/*{
    "DESCRIPTION": "Flash to a colour on the beat",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Time"],
    "OPENVJ": {"ID": "strobe", "NAME": "Strobe", "RESOLUME": ["Strobe", "Flash"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "timing", "LABEL": "Timing", "TYPE": "long", "VALUES": [0, 1], "LABELS": ["Beats", "Hz"], "DEFAULT": 0},
        {"NAME": "rate", "LABEL": "Rate", "TYPE": "long", "VALUES": [0, 1, 2, 3, 4, 5], "LABELS": ["1/16", "1/8", "1/4", "1/2", "1 beat", "2 beats"], "DEFAULT": 2},
        {"NAME": "hz", "LABEL": "Hz", "TYPE": "float", "MIN": 0.5, "MAX": 30, "DEFAULT": 8, "UNIT": "Hz"},
        {"NAME": "duty", "LABEL": "On time", "TYPE": "float", "MIN": 1, "MAX": 99, "DEFAULT": 50, "UNIT": "%"},
        {"NAME": "color", "LABEL": "Colour", "TYPE": "color", "DEFAULT": [1, 1, 1, 1], "RESOLUME": "Color"}
    ]
}*/

// In Beats, the flashes follow the tempo (and Link or MIDI clock) and stay in phase with
// the beat; 1/4 flashes four times a beat.

void main() {
    vec4 c = IMG_THIS_PIXEL(inputImage);
    float beats = 0.0625 * pow(2.0, float(rate));
    float phase = timing == 1 ? fract(TIME * hz) : fract(BEAT / beats);
    bool on = phase < duty / 100.0;
    gl_FragColor = on ? vec4(color.rgb, max(c.a, color.a)) : c;
}
