#version 330
in vec3 vColor;
layout(location = 0) out vec4 pixel;

void main()
{
    pixel = vec4(vColor, 1.0);
}
