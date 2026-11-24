#version 330
layout(location = 0) in vec2 position;
layout(location = 1) in vec2 uv;
out vec2 image_uv;

void main() {
    gl_Position = vec4(position, 0.5, 1.0);
    image_uv = uv;
}
