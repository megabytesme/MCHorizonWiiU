#!/usr/bin/env python3
"""Convert nearby Horizon render sections into a GX2-ready mesh and compact atlas.

The desktop capture stores native-endian `TerrainVertex` records. This tool
decodes them on the host, combines the nearest sections within a vertex budget,
remaps their sprite layers into a small guarded 2D atlas, and emits C constants
for the Wii U big-endian ABI.
"""

import argparse
import json
import math
from pathlib import Path
import struct
import sys


VERTEX_SIZE = 24
LIGHTMAP_SIZE = 16
LIGHTMAP_BYTES = LIGHTMAP_SIZE * LIGHTMAP_SIZE * 4
DEFAULT_MAX_VERTICES = 65_536
MAX_VERTICES = 4_000_000
MAX_CAPTURE_SPRITES = 256
UV_STEPS = 32768.0
SPRITE_BORDER = 1


def c_float(value):
    if not math.isfinite(value):
        raise ValueError("capture contains a non-finite terrain position")
    return f"{value.hex()}f"


def make_compact_atlas(scene, capture_dir, layers):
    spec = scene["settings"]["atlas"]
    sprite_size = spec["sprite_size"]
    texture_layers = spec["texture_layers"]
    if (not isinstance(sprite_size, int) or sprite_size < 1
            or not isinstance(texture_layers, int) or texture_layers < 1):
        raise ValueError("scene atlas has invalid sprite dimensions")
    if len(layers) > MAX_CAPTURE_SPRITES:
        raise ValueError(
            f"selected section uses {len(layers)} sprites; limit is {MAX_CAPTURE_SPRITES}"
        )
    if any(layer >= texture_layers for layer in layers):
        raise ValueError("selected section refers to a sprite outside the captured atlas")

    pixels = (capture_dir / "sprites.bin").read_bytes()
    expected_bytes = sprite_size * sprite_size * 4 * texture_layers
    if len(pixels) != expected_bytes:
        raise ValueError(f"sprites.bin has {len(pixels)} bytes; atlas describes {expected_bytes}")

    cell_size = sprite_size + 2 * SPRITE_BORDER
    columns = math.ceil(math.sqrt(len(layers)))
    rows = math.ceil(len(layers) / columns)
    width = columns * cell_size
    height = rows * cell_size
    compact = bytearray(width * height * 4)
    layer_map = {layer: index for index, layer in enumerate(layers)}
    source_layer_bytes = sprite_size * sprite_size * 4
    for layer, compact_layer in layer_map.items():
        source_base = layer * source_layer_bytes
        cell_x = compact_layer % columns
        cell_y = compact_layer // columns
        for y in range(-SPRITE_BORDER, sprite_size + SPRITE_BORDER):
            source_y = min(max(y, 0), sprite_size - 1)
            for x in range(-SPRITE_BORDER, sprite_size + SPRITE_BORDER):
                source_x = min(max(x, 0), sprite_size - 1)
                source = source_base + (source_y * sprite_size + source_x) * 4
                target_x = cell_x * cell_size + x + SPRITE_BORDER
                target_y = cell_y * cell_size + y + SPRITE_BORDER
                target = (target_y * width + target_x) * 4
                compact[target:target + 4] = pixels[source:source + 4]
    return layer_map, bytes(compact), width, height, sprite_size


def atlas_uv(u_value, v_value, compact_layer, columns, rows, sprite_size):
    cell_size = sprite_size + 2 * SPRITE_BORDER
    cell_x = compact_layer % columns
    cell_y = compact_layer // columns
    wrapped_u = (u_value / UV_STEPS) % 1.0
    wrapped_v = (v_value / UV_STEPS) % 1.0
    u = (cell_x * cell_size + SPRITE_BORDER + 0.5 + wrapped_u * (sprite_size - 1)) / (columns * cell_size)
    v = (cell_y * cell_size + SPRITE_BORDER + 0.5 + wrapped_v * (sprite_size - 1)) / (rows * cell_size)
    return min(65535, math.floor(u * UV_STEPS + 0.5)), min(65535, math.floor(v * UV_STEPS + 0.5))


def convert(capture_dir: Path, output: Path, max_vertices=DEFAULT_MAX_VERTICES,
            source_endian=sys.byteorder):
    if source_endian not in ("little", "big"):
        raise ValueError("source endian must be little or big")
    if not 4 <= max_vertices <= MAX_VERTICES:
        raise ValueError(f"max vertices must be between 4 and {MAX_VERTICES}")

    records = json.loads((capture_dir / "sections.json").read_text(encoding="utf-8"))
    scene = json.loads((capture_dir / "scene.json").read_text(encoding="utf-8"))
    lightmap_pixels = (capture_dir / "lightmap.bin").read_bytes()
    if len(lightmap_pixels) != LIGHTMAP_BYTES:
        raise ValueError(
            f"lightmap.bin has {len(lightmap_pixels)} bytes; expected {LIGHTMAP_BYTES}"
        )
    camera = scene["frame"]["camera"]
    yaw = float(scene["frame"]["yaw"])
    pitch = float(scene["frame"]["pitch"])
    fov = float(scene["frame"]["fov_degrees"])
    min_y = scene["renderer"]["min_y"]
    if len(camera) != 3 or not all(math.isfinite(float(value)) for value in camera):
        raise ValueError("scene camera must contain three finite coordinates")
    if not all(math.isfinite(value) for value in (yaw, pitch, fov)) or not 0.0 < fov < 180.0:
        raise ValueError("scene camera rotation or field of view is invalid")
    width, height = scene["renderer"]["size"]
    if not isinstance(width, int) or not isinstance(height, int) or width < 1 or height < 1:
        raise ValueError("scene renderer size must contain positive pixel dimensions")
    projection_scale = 1.0 / math.tan(math.radians(fov) * 0.5)
    aspect = width / height

    blob_path = capture_dir / "sections.bin"
    blob_size = blob_path.stat().st_size
    cursor = 0
    candidates = []
    for record in records:
        key = record["key"]
        solid_count = record["solid"]
        translucent_count = record["translucent"]
        if (len(key) != 3 or any(not isinstance(value, int) for value in key)
                or not isinstance(solid_count, int) or solid_count < 0
                or not isinstance(translucent_count, int) or translucent_count < 0):
            raise ValueError("section record has an invalid key or vertex count")
        if solid_count % 4:
            raise ValueError(f"section {key} solid mesh is not quad-aligned")
        solid_offset = cursor
        solid_bytes = solid_count * VERTEX_SIZE
        cursor += solid_bytes + translucent_count * VERTEX_SIZE

        origin = [
            key[0] * 16.0 - float(camera[0]),
            min_y + key[1] * 16.0 - float(camera[1]),
            key[2] * 16.0 - float(camera[2]),
        ]
        distance_squared = sum((value + 8.0) ** 2 for value in origin)
        if solid_count:
            candidates.append((distance_squared, tuple(key), solid_count, solid_offset, origin))

    if cursor != blob_size:
        raise ValueError(f"sections.bin has {blob_size} bytes; section records describe {cursor}")
    if not candidates:
        raise ValueError("capture contains no solid section meshes")

    candidates.sort(key=lambda item: (item[0], item[1]))
    selected = []
    remaining_vertices = max_vertices
    for candidate in candidates:
        if candidate[2] <= remaining_vertices:
            selected.append(candidate)
            remaining_vertices -= candidate[2]
    if not selected:
        nearest = candidates[0]
        raise ValueError(
            f"nearest solid section {nearest[1]} has {nearest[2]} vertices; "
            f"increase --max-vertices (currently {max_vertices})"
        )

    key = selected[0][1]
    origin = selected[0][4]
    vertex_count = sum(candidate[2] for candidate in selected)
    endian = "<" if source_endian == "little" else ">"
    vertex_format = struct.Struct(f"{endian}fffHHHHI")
    decoded = []
    layers = set()
    with blob_path.open("rb") as stream:
        for _, section_key, section_vertices, offset, section_origin in selected:
            stream.seek(offset)
            data = stream.read(section_vertices * VERTEX_SIZE)
            if len(data) != section_vertices * VERTEX_SIZE:
                raise ValueError(
                    f"sections.bin ended while reading section {section_key}"
                )
            section_delta = [section_origin[axis] - origin[axis] for axis in range(3)]
            for index in range(section_vertices):
                position_x, position_y, position_z, u, v, layer, light, color = vertex_format.unpack_from(
                    data, index * VERTEX_SIZE
                )
                position = (
                    position_x + section_delta[0],
                    position_y + section_delta[1],
                    position_z + section_delta[2],
                )
                if not all(math.isfinite(value) for value in position):
                    raise ValueError(f"section {section_key} vertex {index} has a non-finite position")
                sprite_layer = layer & 0x7FFF
                layers.add(sprite_layer)
                decoded.append((position, u, v, layer, light, color, sprite_layer))

    layers = sorted(layers)
    layer_map, atlas_pixels, atlas_width, atlas_height, sprite_size = make_compact_atlas(
        scene, capture_dir, layers
    )
    columns = math.ceil(math.sqrt(len(layers)))
    rows = math.ceil(len(layers) / columns)
    vertex_rows = []
    for position, u, v, layer, light, color, sprite_layer in decoded:
        atlas_u, atlas_v = atlas_uv(u, v, layer_map[sprite_layer], columns, rows, sprite_size)
        # The sprite index is baked into the compact atlas UV. Keep only
        # Horizon's force-opaque bit for alpha testing in the GX2 shader.
        layer_flags = layer & 0x8000
        vertex_rows.append(
            "   {{%s, %s, %s}, {%uu, %uu}, %uu, %uu, 0x%08Xu}," % (
                *(c_float(value) for value in position), atlas_u, atlas_v,
                layer_flags, light, color
            )
        )

    atlas_rows = [
        "   " + ", ".join(f"0x{byte:02X}u" for byte in atlas_pixels[offset:offset + 16]) + ","
        for offset in range(0, len(atlas_pixels), 16)
    ]
    lightmap_rows = [
        "   " + ", ".join(f"0x{byte:02X}u" for byte in lightmap_pixels[offset:offset + 16]) + ","
        for offset in range(0, len(lightmap_pixels), 16)
    ]
    output.parent.mkdir(parents=True, exist_ok=True)
    contents = "\n".join(
        [
            "#include <stdint.h>",
            "typedef struct HorizonTerrainVertex {",
            "   float position[3];",
            "   uint16_t uv[2];",
            "   uint16_t layer;",
            "   uint16_t light;",
            "   uint32_t color;",
            "} HorizonTerrainVertex;",
            '_Static_assert(sizeof(HorizonTerrainVertex) == 24, "TerrainVertex ABI must stay 24 bytes");',
            "",
            "const HorizonTerrainVertex horizon_wiiu_capture_vertices[] = {",
            *vertex_rows,
            "};",
            f"const uint32_t horizon_wiiu_capture_vertex_count = {vertex_count}u;",
            "const float horizon_wiiu_capture_section_origin[3] = {",
            f"   {c_float(origin[0])}, {c_float(origin[1])}, {c_float(origin[2])}",
            "};",
            "const uint8_t horizon_wiiu_capture_atlas_pixels[] = {",
            *atlas_rows,
            "};",
            f"const uint32_t horizon_wiiu_capture_atlas_width = {atlas_width}u;",
            f"const uint32_t horizon_wiiu_capture_atlas_height = {atlas_height}u;",
            "const uint8_t horizon_wiiu_capture_lightmap_pixels[1024] = {",
            *lightmap_rows,
            "};",
            "const float horizon_wiiu_capture_view[4] = {",
            f"   {c_float(yaw)}, {c_float(pitch)}, {c_float(projection_scale)}, {c_float(aspect)}",
            "};",
            "",
        ]
    )
    output.write_text(contents, encoding="ascii", newline="\n")
    return {
        "key": key,
        "keys": [candidate[1] for candidate in selected],
        "sections": len(selected),
        "vertices": vertex_count,
        "origin": origin,
        "layers": layers,
        "atlas_size": (atlas_width, atlas_height),
        "atlas_pixels": atlas_pixels,
        "view": (yaw, pitch, projection_scale, aspect),
        "output": output,
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture_dir", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--max-vertices", type=int, default=DEFAULT_MAX_VERTICES)
    parser.add_argument("--source-endian", choices=("little", "big"), default=sys.byteorder)
    args = parser.parse_args(argv)
    try:
        result = convert(args.capture_dir, args.output, args.max_vertices, args.source_endian)
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError) as error:
        parser.error(str(error))
    print(
        f"selected {result['sections']} sections (nearest {result['key']}): "
        f"{result['vertices']} solid vertices from "
        f"{len(result['layers'])} sprite layers, compact atlas "
        f"{result['atlas_size'][0]}x{result['atlas_size'][1]}, camera-relative origin "
        f"{result['origin']} -> {result['output']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
