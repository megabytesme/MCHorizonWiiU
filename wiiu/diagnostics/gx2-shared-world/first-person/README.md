# GX2 first-person crop projection fixture

Run the fixture against an explicit assembled engine source path. The runner verifies the SHA-256 of every imported file, accepting either Git checkout newline convention. It generates a disposable Rust crate, pins the host math dependencies and removes the generated source and build output after testing.

```sh
python3 -B wiiu/diagnostics/gx2-shared-world/first-person/run.py /path/to/assembled-rc46 --offline
```

Use Rust 2024 edition support and Python 3.9 or newer. Omit `--offline` if Cargo still needs the locked dependencies. The fixture needs no prepared game assets. It accepts a checkout or export with the seven pinned input files.

All seven imported Git blobs are byte-for-byte identical in stock RC46 tree `037bcf9dc59e000d0de76509f3f8304f63a9c5f6` and the private shared-world scene tree `34092c4b1bbe5c53c1533e7384d965706489c18a`. The fixture makes no assumption about other files in the assembled source. If one of the inputs changes, review the extraction and the expected results before updating its digest.

The tests use the native hand perspective, terrain clip rows and selection stroke expansion. They compare the fitted 720p and 1080p TV viewports and a narrower GamePad crop. Hand coordinates agree after the centered crop with the same vertical projection and near clipping. Continuous perspective-correct held-item UV also agrees. The negative controls confirm that a 2.5-pixel native GamePad outline shrinks to about 1.11 pixels when copied from the fitted 1080p TV view, and that a centered crop changes the texture coordinates of a full-screen overlay.

These are host projection and geometry checks. A native first-person comparison still needs pixel evidence for its scaled hand, world color, atlas updates and GPU ordering.
