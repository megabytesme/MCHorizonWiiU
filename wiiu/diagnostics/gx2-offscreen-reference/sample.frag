#version 330
uniform sampler2D source_texture;
in vec2 image_uv;
layout(location = 0) out vec4 pixel;

void main() {
    pixel = texture(source_texture, image_uv);
}
