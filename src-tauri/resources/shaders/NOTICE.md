# Bundled GLSL upscaling shaders

These third-party mpv user shaders are bundled to power Lumo's "AI Upscaling"
feature. Both are MIT-licensed.

## Anime (`anime/`)
Anime4K — https://github.com/bloc97/Anime4K — Copyright (c) bloc97, MIT License.
Pipeline: Clamp Highlights → Restore CNN (M) → Upscale CNN x2 (M).

## Live-action (`live/`)
ravu (mpv-prescalers) — https://github.com/bjin/mpv-prescalers — Copyright (c)
bjin, MIT License. File: ravu-lite-ar-r3 (bundled with a `.glsl` extension;
mpv reads user shaders by their `//!HOOK` directives, not the file extension).

The full MIT license text is retained at the top of each shader file.
