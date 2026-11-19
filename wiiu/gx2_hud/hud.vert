#version 330
layout(location = 0) in vec4 aPosition;
layout(location = 1) in vec4 aColor;
layout(location = 2) in vec2 aUv;
layout(location = 3) in vec2 aGlintUv;
out vec4 vColor;
out vec2 vUv;
out vec2 vGlintUv;
out vec2 vProjection;
void main() {
    gl_Position = aPosition;
    vColor = aColor;
    vUv = aUv;
    vGlintUv = aGlintUv;
    vProjection = aPosition.xy * 0.5 + 0.5;
}
