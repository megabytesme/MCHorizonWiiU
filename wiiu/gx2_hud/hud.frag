#version 330
uniform sampler2D uAtlas;
uniform sampler2D uGlint;
uniform vec4 uPass;
uniform vec4 uGlintStyle;
uniform vec4 uCitTint;
uniform vec4 uEndSky;
uniform vec4 uEndPortal;
uniform vec4 uPortalTime;
in vec4 vColor;
in vec2 vUv;
in vec2 vGlintUv;
in vec2 vProjection;
out vec4 fragColor;
// Horizon hud.wgsl credits background. Each portal layer wraps in its atlas cell.
vec3 portalSample(vec4 rect, vec2 p) {
    return textureLod(uAtlas, rect.xy + fract(p) * rect.zw, 0.0).rgb;
}
vec3 portalLayer(vec2 p, float layer, vec3 color) {
    float angle = radians((layer * layer * 4321.0 + layer * 9.0) * 2.0);
    float c = cos(angle);
    float s = sin(angle);
    float k = (4.5 - layer / 4.0) * 2.0;
    vec2 v = k * vec2(c * p.x - s * p.y, s * p.x + c * p.y);
    v += vec2(17.0 / layer, (2.0 + layer / 1.5) * (uPortalTime.x * 1.5));
    return portalSample(uEndPortal, 0.5 * v + 0.25) * color;
}
vec4 endPortal() {
    vec2 p = vProjection;
    vec3 result = portalSample(uEndSky, p) * vec3(0.022087, 0.098399, 0.110818);
    result += portalLayer(p, 1.0, vec3(0.022087, 0.098399, 0.110818));
    result += portalLayer(p, 2.0, vec3(0.011892, 0.095924, 0.089485));
    result += portalLayer(p, 3.0, vec3(0.027636, 0.101689, 0.100326));
    result += portalLayer(p, 4.0, vec3(0.046564, 0.109883, 0.114838));
    result += portalLayer(p, 5.0, vec3(0.064901, 0.117696, 0.097189));
    result += portalLayer(p, 6.0, vec3(0.063761, 0.086895, 0.123646));
    result += portalLayer(p, 7.0, vec3(0.084817, 0.111994, 0.166380));
    result += portalLayer(p, 8.0, vec3(0.097489, 0.154120, 0.091064));
    result += portalLayer(p, 9.0, vec3(0.106152, 0.131144, 0.195191));
    result += portalLayer(p, 10.0, vec3(0.097721, 0.110188, 0.187229));
    result += portalLayer(p, 11.0, vec3(0.133516, 0.138278, 0.148582));
    result += portalLayer(p, 12.0, vec3(0.070006, 0.243332, 0.235792));
    result += portalLayer(p, 13.0, vec3(0.196766, 0.142899, 0.214696));
    result += portalLayer(p, 14.0, vec3(0.047281, 0.315338, 0.321970));
    result += portalLayer(p, 15.0, vec3(0.204675, 0.390010, 0.302066));
    return vec4(result, 1.0);
}
void main() {
    if (uPass.x == 6.0) {
        float mask = texture(uAtlas, vUv).a;
        vec4 tint = textureLod(uGlint, vGlintUv, 0.0) * uCitTint;
        tint.a *= vColor.a;
        if (mask < 0.1 || tint.a < 0.1) discard;
        fragColor = tint;
        return;
    }
    if (uPass.x == 5.0) {
        fragColor = endPortal();
        return;
    }
    if (vUv.x >= 2.0) {
        float mask = texture(uAtlas, vUv - vec2(2.0, 0.0)).a;
        if (mask < 0.1) discard;
        vec3 glint = uGlintStyle.x * texture(uGlint, vGlintUv).rgb;
        if (uGlintStyle.y >= 0.0) {
            glint = max(glint.r, max(glint.g, glint.b)) * uGlintStyle.yzw;
        }
        fragColor = vec4(glint * glint, mask);
        return;
    }
    vec4 c = texture(uAtlas, vUv) * vColor;
    if (uPass.x == 4.0) {
        fragColor = vec4(c.rgb, 1.0);
        return;
    }
    if (uPass.x == 1.0 ? c.a == 0.0 : c.a < 0.004) discard;
    fragColor = c;
}
