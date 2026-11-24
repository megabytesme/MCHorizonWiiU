#version 330
layout(location = 0) in vec4 aPosition;

uniform vec4 uClipX;
uniform vec4 uClipY;
uniform vec4 uClipZ;
uniform vec4 uClipW;
uniform vec4 uTileColor;

out vec3 vColor;

void main()
{
    gl_Position = vec4(dot(uClipX, aPosition), dot(uClipY, aPosition),
                       dot(uClipZ, aPosition), dot(uClipW, aPosition));
    vColor = uTileColor.rgb;
}
