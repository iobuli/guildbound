# StatusFXFree adapter

Imported from the user-provided `assets/StatusFXFree.zip`, by Binbun3D (CC0).
Only the referenced shaders, the shatter preset and licence are retained; no Godot runtime is needed.

- `aura_sphere.gdshader`: scrolling, twisting noise and a vertical band mask adapted in
  `assets/shaders/wind_aura.wgsl` for charging. The Bevy version restricts it to the silhouette
  and clamps the band before fractional powers to avoid invalid values.
- `ice_particle.gdshader`: radial branches/spokes baked into the 64x64 frost particle sprite.
- `streaks.gdshader`: overlaid angular streaks and centre cutout baked into the arcane sprite.

The sprites share the existing Hanabi effect library and emitter cap. There are no extra
per-frame texture uploads. Regenerate reference files with `python tools/kev/import_statusfx.py`.
