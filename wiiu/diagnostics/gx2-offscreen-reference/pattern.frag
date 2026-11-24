#version 330
in vec2 image_uv;
layout(location = 0) out vec4 pixel;

void main() {
    vec3 color = image_uv.y < 0.42 ? vec3(1, 0, 0) : vec3(0, 0, 1);
    if (image_uv.x < 0.18 && image_uv.y < 0.20)
        color = vec3(0.9, 0, 1);
    if (image_uv.x > 0.72 && image_uv.y > 0.64)
        color = vec3(1, 0.75, 0);
    pixel = vec4(color, 1);
}
