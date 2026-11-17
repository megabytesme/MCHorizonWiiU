#version 330
layout(location = 0) in vec4 aPosition;
layout(location = 1) in vec3 aColor;
layout(location = 2) in vec2 aUv;
layout(location = 3) in float aCutout;
uniform vec4 uClipX;
uniform vec4 uClipY;
uniform vec4 uClipZ;
uniform vec4 uClipW;
out vec3 vColor;
out vec2 vUv;
out float vCutout;
void main() {
    vec3 local = aPosition.xyz;
    gl_Position = vec4(dot(uClipX.xyz, local) + uClipX.w,
                       dot(uClipY.xyz, local) + uClipY.w,
                       dot(uClipZ.xyz, local) + uClipZ.w,
                       dot(uClipW.xyz, local) + uClipW.w);
    vColor = aColor;
    vUv = aUv;
    vCutout = aCutout;
}
