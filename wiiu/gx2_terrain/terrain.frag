#version 330
uniform sampler2D uTerrain;
uniform vec4 uFogColor;
uniform vec4 uFogRanges;
in vec3 vColor;
in vec2 vUv;
in float vCutout;
in vec2 vFogDistance;
out vec4 fragColor;

float linearFog(float distance, float start, float end) {
    if (distance <= start) return 0.0;
    if (distance >= end) return 1.0;
    return (distance - start) / (end - start);
}

void main() {
    vec4 texel = texture(uTerrain, vUv);
    if (vCutout > 0.5 && texel.a < 0.5) discard;
    float fog = max(linearFog(vFogDistance.x, uFogRanges.x, uFogRanges.y),
                    linearFog(vFogDistance.y, uFogRanges.z, uFogRanges.w));
    fragColor = vec4(mix(texel.rgb * vColor, uFogColor.rgb, fog * uFogColor.a), texel.a);
}
