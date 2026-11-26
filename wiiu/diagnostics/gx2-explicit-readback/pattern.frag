#version 330
in vec2 image_uv;
layout(location = 0) out vec4 pixel;

void main() {
    vec3 rgb = image_uv.y < 0.5
        ? (image_uv.x < 0.5 ? vec3(1, 0, 0) : vec3(0, 1, 0))
        : (image_uv.x < 0.5 ? vec3(0, 0, 1) : vec3(1, 1, 0));
    pixel = vec4(rgb, 1);
}
