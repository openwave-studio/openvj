/*{
    "DESCRIPTION": "Glow around the bright parts",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Stylize"],
    "OPENVJ": {"ID": "bloom", "NAME": "Bloom", "RESOLUME": ["Bloom", "Glow"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "threshold", "LABEL": "Threshold", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 60, "UNIT": "%", "RESOLUME": "Threshold"},
        {"NAME": "amount", "LABEL": "Amount", "TYPE": "float", "MIN": 0, "MAX": 300, "DEFAULT": 100, "UNIT": "%", "RESOLUME": "Amount"},
        {"NAME": "radius", "LABEL": "Radius", "TYPE": "float", "MIN": 1, "MAX": 64, "DEFAULT": 16, "UNIT": "px", "RESOLUME": "Radius"}
    ],
    "PASSES": [
        {"TARGET": "bright", "WIDTH": "$WIDTH / 2.0", "HEIGHT": "$HEIGHT / 2.0"},
        {"TARGET": "blurred", "WIDTH": "$WIDTH / 2.0", "HEIGHT": "$HEIGHT / 2.0"},
        {}
    ]
}*/

// Pass 1 keeps what is above Threshold at half size, pass 2 blurs it in a disc, and the
// last pass adds it back over the picture.

float weight(vec3 c) {
    float luma = dot(c, vec3(0.2126, 0.7152, 0.0722));
    return smoothstep(threshold / 100.0, threshold / 100.0 + 0.1, luma);
}

void main() {
    vec2 uv = isf_FragNormCoord;
    if (PASSINDEX == 0) {
        vec4 c = IMG_NORM_PIXEL(inputImage, uv);
        gl_FragColor = vec4(c.rgb * weight(c.rgb) * c.a, 1.0);
    } else if (PASSINDEX == 1) {
        vec2 px = radius / RENDERSIZE;
        vec3 sum = IMG_NORM_PIXEL(bright, uv).rgb;
        float total = 1.0;
        for (int ring = 1; ring <= 3; ring++) {
            float r = float(ring) / 3.0;
            float w = 1.0 - r * 0.6;
            for (int i = 0; i < 12; i++) {
                float a = 6.2831853 * (float(i) + 0.5 * float(ring)) / 12.0;
                sum += IMG_NORM_PIXEL(bright, uv + vec2(cos(a), sin(a)) * px * r).rgb * w;
                total += w;
            }
        }
        gl_FragColor = vec4(sum / total, 1.0);
    } else {
        vec4 c = IMG_NORM_PIXEL(inputImage, uv);
        vec3 glow = IMG_NORM_PIXEL(blurred, uv).rgb * amount / 100.0 * 2.0;
        gl_FragColor = vec4(clamp(c.rgb + glow, 0.0, 1.0), max(c.a, dot(glow, vec3(0.333))));
    }
}
