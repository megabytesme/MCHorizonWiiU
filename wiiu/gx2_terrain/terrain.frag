#version 330
uniform sampler2D uTerrain;
in vec3 vColor;
in vec2 vUv;
in float vCutout;
out vec4 fragColor;
void main() {
    vec4 texel = texture(uTerrain, vUv);
    if (vCutout > 0.5 && texel.a < 0.5) discard;
    fragColor = vec4(texel.rgb * vColor, texel.a);
}
