/*{
    "DESCRIPTION": "The fixed Transform of every clip and layer, laid out like Resolume's",
    "CREDIT": "openVJ",
    "ISFVSN": "2",
    "CATEGORIES": ["Geometry"],
    "OPENVJ": {"ID": "clip_transform", "NAME": "Transform", "HIDDEN": true},
    "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "position_x", "LABEL": "Position X", "TYPE": "float", "MIN": -4000, "MAX": 4000, "DEFAULT": 0, "UNIT": " px", "RESOLUME": "Position X"},
        {"NAME": "position_y", "LABEL": "Position Y", "TYPE": "float", "MIN": -4000, "MAX": 4000, "DEFAULT": 0, "UNIT": " px", "RESOLUME": "Position Y"},
        {"NAME": "scale", "LABEL": "Scale", "TYPE": "float", "MIN": 0, "MAX": 1000, "DEFAULT": 100, "UNIT": "%", "RESOLUME": "Scale"},
        {"NAME": "scale_w", "LABEL": "Scale W", "TYPE": "float", "MIN": 0, "MAX": 1000, "DEFAULT": 100, "UNIT": "%", "RESOLUME": "Scale W"},
        {"NAME": "scale_h", "LABEL": "Scale H", "TYPE": "float", "MIN": 0, "MAX": 1000, "DEFAULT": 100, "UNIT": "%", "RESOLUME": "Scale H"},
        {"NAME": "rotation_x", "LABEL": "Rotation X", "TYPE": "float", "MIN": -180, "MAX": 180, "DEFAULT": 0, "UNIT": "°", "WRAP": true, "RESOLUME": "Rotation X"},
        {"NAME": "rotation_y", "LABEL": "Rotation Y", "TYPE": "float", "MIN": -180, "MAX": 180, "DEFAULT": 0, "UNIT": "°", "WRAP": true, "RESOLUME": "Rotation Y"},
        {"NAME": "rotation_z", "LABEL": "Rotation Z", "TYPE": "float", "MIN": -180, "MAX": 180, "DEFAULT": 0, "UNIT": "°", "WRAP": true, "RESOLUME": "Rotation Z"},
        {"NAME": "anchor_x", "LABEL": "Anchor X", "TYPE": "float", "MIN": -4000, "MAX": 4000, "DEFAULT": 0, "UNIT": " px", "RESOLUME": "Anchor X"},
        {"NAME": "anchor_y", "LABEL": "Anchor Y", "TYPE": "float", "MIN": -4000, "MAX": 4000, "DEFAULT": 0, "UNIT": " px", "RESOLUME": "Anchor Y"}
    ]
}*/

// Pixels from the centre, Y up, like Resolume. The frame is a card: scaled and rotated
// around its anchor, moved, and seen through a perspective camera, so Rotation X and Y tilt
// it in depth. For each output pixel, find where the camera ray meets the card.

mat3 rot_x(float a) { float c = cos(a), s = sin(a); return mat3(1.0, 0.0, 0.0, 0.0, c, s, 0.0, -s, c); }
mat3 rot_y(float a) { float c = cos(a), s = sin(a); return mat3(c, 0.0, -s, 0.0, 1.0, 0.0, s, 0.0, c); }
mat3 rot_z(float a) { float c = cos(a), s = sin(a); return mat3(c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0); }

void main() {
    vec2 size = max(RENDERSIZE, vec2(1.0));
    vec2 y = (isf_FragNormCoord - 0.5) * size * vec2(1.0, -1.0);
    vec2 s = vec2(scale * scale_w, scale * scale_h) / 10000.0;
    vec2 anchor = vec2(anchor_x, anchor_y);
    mat3 r = rot_z(radians(rotation_z)) * rot_y(radians(rotation_y)) * rot_x(radians(rotation_x));

    // A 60° vertical field of view: the card at depth 0 is drawn 1:1.
    vec3 cam = vec3(0.0, 0.0, -0.5 * size.y / tan(radians(30.0)));
    vec3 origin = vec3(anchor + vec2(position_x, position_y), 0.0);
    vec3 n = r * vec3(0.0, 0.0, 1.0);
    vec3 dir = vec3(y, 0.0) - cam;
    float denom = dot(dir, n);
    float t = abs(denom) < 1e-6 ? -1.0 : dot(origin - cam, n) / denom;
    vec3 hit = cam + dir * t;
    vec2 local = vec2(dot(hit - origin, r * vec3(1.0, 0.0, 0.0)), dot(hit - origin, r * vec3(0.0, 1.0, 0.0)));
    bool flat_card = any(lessThan(abs(s), vec2(1e-6)));
    vec2 x = local / (flat_card ? vec2(1.0) : s) + anchor;

    vec2 uv = x / size * vec2(1.0, -1.0) + 0.5;
    bool inside = !flat_card && t > 0.0 && all(greaterThanEqual(uv, vec2(0.0))) && all(lessThanEqual(uv, vec2(1.0)));
    gl_FragColor = inside ? IMG_NORM_PIXEL(inputImage, uv) : vec4(0.0);
}
