/*{
    "DESCRIPTION": "The fixed META look of every layer (Synthesis layout), like Synesthesia's meta controls",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Color"],
    "OPENVJ": {"ID": "layer_look", "NAME": "Meta", "HIDDEN": true},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "invert", "LABEL": "Invert", "TYPE": "bool", "DEFAULT": false},
        {"NAME": "brightness", "LABEL": "Brightness", "TYPE": "float", "MIN": 0, "MAX": 200, "DEFAULT": 100, "UNIT": "%"},
        {"NAME": "gamma", "LABEL": "Gamma", "TYPE": "float", "MIN": -100, "MAX": 100, "DEFAULT": 0, "UNIT": "%"},
        {"NAME": "contrast", "LABEL": "Contrast", "TYPE": "float", "MIN": -100, "MAX": 100, "DEFAULT": 0, "UNIT": "%"},
        {"NAME": "hue", "LABEL": "Hue", "TYPE": "float", "MIN": 0, "MAX": 360, "DEFAULT": 0, "UNIT": "°", "WRAP": true},
        {"NAME": "saturation", "LABEL": "Saturation", "TYPE": "float", "MIN": 0, "MAX": 200, "DEFAULT": 100, "UNIT": "%"},
        {"NAME": "mirror_v", "LABEL": "Vert Mirror", "TYPE": "bool", "DEFAULT": false},
        {"NAME": "mirror_h", "LABEL": "Hor Mirror", "TYPE": "bool", "DEFAULT": false},
        {"NAME": "limit", "LABEL": "Limit Colors", "TYPE": "bool", "DEFAULT": false},
        {"NAME": "low_color", "LABEL": "Low Color", "TYPE": "color", "DEFAULT": [0.25, 0.0, 0.05, 1.0]},
        {"NAME": "high_color", "LABEL": "High Color", "TYPE": "color", "DEFAULT": [1.0, 0.55, 0.0, 1.0]},
        {"NAME": "alpha", "LABEL": "Alpha Channel", "TYPE": "bool", "DEFAULT": false}
    ]
}*/
// One pass, skipped when every value is at its default. Order: mirror, invert, hue,
// saturation, brightness, contrast, gamma, limit colors, alpha.
void main() {
    vec2 t = isf_FragNormCoord;
    // Mirror the left (bottom) half onto the right (top), as Synesthesia does.
    if (mirror_h && t.x > 0.5) {
        t.x = 1.0 - t.x;
    }
    if (mirror_v && t.y > 0.5) {
        t.y = 1.0 - t.y;
    }
    vec4 c = IMG_NORM_PIXEL(inputImage, t);
    vec3 rgb = c.rgb;
    if (invert) {
        rgb = vec3(1.0) - rgb;
    }
    float a = radians(hue);
    vec3 q = vec3(0.57735);
    rgb = rgb * cos(a) + cross(q, rgb) * sin(a) + q * dot(q, rgb) * (1.0 - cos(a));
    const vec3 LUMA = vec3(0.299, 0.587, 0.114);
    rgb = mix(vec3(dot(rgb, LUMA)), rgb, saturation / 100.0);
    rgb *= brightness / 100.0;
    rgb = (rgb - 0.5) * pow(4.0, contrast / 100.0) + 0.5;
    // Gamma: +100% brightens the mid-tones 4×, -100% darkens them 4×.
    rgb = pow(clamp(rgb, 0.0, 1.0), vec3(pow(4.0, -gamma / 100.0)));
    if (limit) {
        rgb = mix(low_color.rgb, high_color.rgb, dot(rgb, LUMA));
    }
    float alpha_out = c.a;
    if (alpha) {
        alpha_out *= max(rgb.r, max(rgb.g, rgb.b));
    }
    gl_FragColor = vec4(clamp(rgb, 0.0, 1.0), alpha_out);
}
