#version 330
uniform sampler2D uPanorama;
in vec3 vDirection;
out vec4 fragColor;
void main() {
 vec3 d=vDirection; vec3 a=abs(d); vec2 uv; float face;
 if (a.x >= a.y && a.x >= a.z) {
   if (d.x >= 0.0) { face=0.0; uv=vec2(-d.z,-d.y)/a.x; }
   else { face=1.0; uv=vec2(d.z,-d.y)/a.x; }
 } else if (a.y >= a.z) {
   if (d.y >= 0.0) { face=2.0; uv=vec2(d.x,d.z)/a.y; }
   else { face=3.0; uv=vec2(d.x,-d.z)/a.y; }
 } else {
   if (d.z >= 0.0) { face=4.0; uv=vec2(d.x,-d.y)/a.z; }
   else { face=5.0; uv=vec2(-d.x,-d.y)/a.z; }
 }
 uv=uv*0.5+0.5;
 float border=0.5 / float(textureSize(uPanorama,0).x);
 uv=clamp(uv,vec2(border),vec2(1.0-border));
 fragColor=texture(uPanorama,vec2(uv.x,(face+uv.y)/6.0));
}
