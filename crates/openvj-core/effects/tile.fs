/*{
    "DESCRIPTION": "Repeat the picture in a grid",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Geometry"],
    "OPENVJ": {"ID": "tile", "NAME": "Tile", "RESOLUME": ["Tile", "Tiles", "Repeat", "Grid"]},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "columns", "LABEL": "Columns", "TYPE": "float", "MIN": 1, "MAX": 16, "DEFAULT": 2, "RESOLUME": "Tiles X"},
        {"NAME": "rows", "LABEL": "Rows", "TYPE": "float", "MIN": 1, "MAX": 16, "DEFAULT": 2, "RESOLUME": "Tiles Y"},
        {"NAME": "mirror", "LABEL": "Mirror", "TYPE": "bool", "DEFAULT": false, "RESOLUME": "Mirror"}
    ]
}*/

// Mirror flips every other tile, so the tiles meet without seams.

void main() {
    vec2 g = isf_FragNormCoord * vec2(floor(columns), floor(rows));
    vec2 cell = floor(g);
    vec2 t = fract(g);
    if (mirror) {
        t = mix(t, 1.0 - t, mod(cell, 2.0));
    }
    gl_FragColor = IMG_NORM_PIXEL(inputImage, t);
}
