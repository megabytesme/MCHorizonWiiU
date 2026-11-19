#version 330

uniform sampler2D uTexture;
uniform sampler2D uLightmap;
in vec3 vColor;
in vec2 vUv;
flat in float vAlphaCutout;
in vec2 vLightUv;
out vec4 fragColor;

void main() {
    vec4 texel = texture(uTexture, vUv);
    if (vAlphaCutout > 0.5 && texel.a < 0.5) {
        discard;
    }
    vec4 light = texture(uLightmap, vLightUv);
    fragColor = texel * vec4(vColor, 1.0) * light;
}
