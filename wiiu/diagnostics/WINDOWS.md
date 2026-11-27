# Standalone Cafe diagnostics on Windows

The Windows builder supports native Wii U C sources, vertex and fragment
GLSL, and prebuilt GFDs. Use Python 3.13 or later, the portable devkitPPC
16.2.0 and WUT 1.9.2 SDK, its verified staging receipt, and the official
CafeGLSL 1.0.0-rc1 Windows executable. The hosted Rust game build continues
to use the pinned Linux Cafe toolchain. [CafeGLSL release](https://github.com/Exzap/CafeGLSL/releases/tag/v1.0.0-rc1) ·
[devkitPro package definitions](https://github.com/devkitPro/pacman-packages)

The SDK receipt records `packages` with the seven devkitPro package pins,
`files` with each staged relative path, size and hash by package, and
`aliases` with each staged alias, target and hash. Use the receipt from
the verified SDK staging. The builder checks the common WUT and newlib
files against the canonical toolchain specification.

Provide each path explicitly. Each recipe input starts with a root name
and uses a slash-separated path within that root. The builder stages the
declared source bytes and records each command and output hash.

```powershell
python -B wiiu/diagnostics/build_windows_diagnostic.py `
  --sdk-root $sdkRoot --sdk-receipt $sdkReceipt `
  --toolchain-spec catalog/toolchain-cafe-v1.1.0.json `
  --cafeglsl $windowsCafeGlsl --recipe $recipe --output $newOutput `
  --root fixture $fixtureSource --root engine $assembledEngine `
  --root port $portDirectory
```

The output directory must be new. `--reference` takes the optional path to
a previous `build-receipt.json` with the retained artifacts in the same
directory. The fresh ELF and RPX and other named artifacts must match the
retained files' hashes. Failed tools leave logs in the output directory.

| Recipe field | Contents |
| --- | --- |
| `schema` | `1` |
| `name` | Basename for the ELF, RPX and optional WUHB |
| `inputs` | Map every local C, transitive header, GLSL and prebuilt input to its raw SHA-256 |
| `sources` | Ordered entries with `path` and `object`. Use an object stem without `.o` |
| `includes` | Optional source directories with their files declared in `inputs` |
| `shaders` | Optional entries with `name`, expected `sha256`, and `vertex` and `pixel` paths or one prebuilt `gfd` path |
| `defines`, `libraries` | Optional additional preprocessor definitions and Wii U libraries |
| `wuhb` | Optional packaging fields `name`, `short_name` and `author` |

Shader entries produce `<name>.gsh` and `<name>.o`, with the corresponding
`_binary_<name>_gsh_start` symbol available to C. Supply `uniform_mode`
on a GLSL entry when it requires an explicit `auto`, `register` or
`block` mode. Generate derived sources before invoking the builder and
list them with their exact hashes.

The atlas point-address fixture rebuilt byte for byte from GLSL and from
its pinned GFD, including the PowerPC ELF, RPX and WUHB. The [paired atlas
fixture](gx2-atlas-parity/README.md) was also built with this route and
checked on both Cemu backends.
