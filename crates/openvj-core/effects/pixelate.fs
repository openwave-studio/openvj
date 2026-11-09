/*{
    "DESCRIPTION": "Chunky pixels",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Stylize"],
    "OPENVJ": {"ID": "pixelate", "NAME": "Pixelate", "RESOLUME": ["Pixelate", "PixelBlur", "Mosaic", "LoRez"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "size", "LABEL": "Size", "TYPE": "float", "MIN": 0, "MAX": 100, "DEFAULT": 30, "UNIT": "%"}
    ]
}*/

// 0% is 160 cells down the frame, 100% is 6.
void main() {
    float cells = mix(160.0, 6.0, size / 100.0);
    vec2 grid = vec2(cells * RENDERSIZE.x / max(RENDERSIZE.y, 1.0), cells);
    gl_FragColor = IMG_NORM_PIXEL(inputImage, (floor(isf_FragNormCoord * grid) + 0.5) / grid);
}
