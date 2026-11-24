# GX2 translucent particle fixture

Run `bash build.sh ASSEMBLED_SOURCE BUILD_DIRECTORY` with the pinned Cafe
toolchain. Pass the assembled source directory for the translucent particle
patch. The fixture links the production terrain and model shaders and draws
into both native outputs.

On each output, the green sample shows that opaque terrain occludes the
particles. The second sample shows a half-alpha particle blending over a
blue background and occluding a later probe behind it. Expect four
`pass=1` readbacks and the final
`GX2 particle fixture passed` message.
