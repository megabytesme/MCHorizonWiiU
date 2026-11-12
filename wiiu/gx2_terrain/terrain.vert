#version 330
layout(location = 0) in vec4 aPosition;
layout(location = 1) in vec3 aColor;
layout(location = 2) in vec2 aUv;
layout(location = 3) in float aCutout;
out vec3 vColor;
out vec2 vUv;
out float vCutout;
void main() {
    gl_Position = aPosition;
    vColor = aColor;
    vUv = aUv;
    vCutout = aCutout;
}
