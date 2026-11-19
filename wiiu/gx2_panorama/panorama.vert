#version 330
layout(location = 0) in vec4 aPosition;
layout(location = 1) in vec3 aDirection;
out vec3 vDirection;
void main() {
    gl_Position = aPosition;
    vDirection = aDirection;
}
