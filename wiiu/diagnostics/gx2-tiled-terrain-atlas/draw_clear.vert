#version 330
layout(location = 0) in vec3 aPosition;
uniform vec4 uClipX;
uniform vec4 uClipY;
uniform vec4 uClipZ;
uniform vec4 uClipW;

void main() {
    vec4 pos = vec4(aPosition, 1.0);
    gl_Position = vec4(dot(uClipX, pos), dot(uClipY, pos),
                       dot(uClipZ, pos), dot(uClipW, pos));
}
