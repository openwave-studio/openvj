/*{
    "DESCRIPTION": "Hue, saturation, brightness, contrast",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Color"],
    "OPENVJ": {"ID": "color", "NAME": "Color", "RESOLUME": ["HueRotate", "Colorize", "HueSaturation", "BrightContrast", "BrightnessContrast", "Saturation", "Levels", "Exposure", "Tint", "Vibrance", "Recolor"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "hue", "LABEL": "Hue", "TYPE": "float", "MIN": 0, "MAX": 360, "DEFAULT": 0, "UNIT": "°", "WRAP": true, "RESOLUME": {"NAME": "Hue Rotate", "RANGE": [0, 1]}},
        {"NAME": "saturation", "LABEL": "Saturation", "TYPE": "float", "MIN": 0, "MAX": 200, "DEFAULT": 100, "UNIT": "%", "RESOLUME": {"NAME": "Sat. Scale", "RANGE": [0, 1]}},
        {"NAME": "brightness", "LABEL": "Brightness", "TYPE": "float", "MIN": 0, "MAX": 200, "DEFAULT": 100, "UNIT": "%"},
        {"NAME": "contrast", "LABEL": "Contrast", "TYPE": "float", "MIN": -100, "MAX": 100, "DEFAULT": 0, "UNIT": "%"}
    ]
}*/

void main() {
    vec4 c = IMG_THIS_PIXEL(inputImage);
    vec3 rgb = c.rgb;
    float a = radians(hue);
    vec3 q = vec3(0.57735);
    rgb = rgb * cos(a) + cross(q, rgb) * sin(a) + q * dot(q, rgb) * (1.0 - cos(a));
    rgb = mix(vec3(dot(rgb, vec3(0.299, 0.587, 0.114))), rgb, saturation / 100.0);
    rgb *= brightness / 100.0;
    rgb = (rgb - 0.5) * pow(4.0, contrast / 100.0) + 0.5;
    gl_FragColor = vec4(clamp(rgb, 0.0, 1.0), c.a);
}
