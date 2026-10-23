# Magic effects and rendering limits

The sword exports now preserve skin joint metadata even when they omit rendered meshes. The
game donor loader requires that metadata. The previous animation-only exports compiled into a
Rust build successfully but caused a `no skin 0 joints` panic at startup. Regenerate them with
`python tools/kev/autoanim/unreal/build_sword_styles.py`.

The new spell accents use the existing animated sheets and shared geometry:

| Spell | Look |
| --- | --- |
| Ember Burst | Compact ember, bright flame impact |
| Glacier Lance | Straight frost ray, crystallizing impact |
| Thunder Ring | Expanding ground ripple and electric circle |
| Tide Orbs | Cold motes with small bubble halos |
| Arcane Step | Sparse purple departure sparks and phantom accent |
| Sunflare | Solar core with a narrow corona and sunburst impact |
| Shadow Veil | Subdued midnight ripple at the feet |
| Stone Ward | Thin protection bands and six faceted stones |
| Cometfall | Elongated fire core, curling fire impact |
| Gale Crescent | Shared crescent blade and slash impact |

Each spell uses its configured cast clip. Glacier Lance and Thunder Ring now use their own
elements throughout. Arcane Step applies its declared damage along actual swept movement once
per enemy, and emits sparks at a fixed interval instead of spawning a burst every frame.
Wards use thin bands without a filled sphere covering the character.

Held spells use moving upper-body preparations from the Sekiro big projectile clip.
Ki bullets and one-handed beams use the ki-blast clip; sword crescents prepare their sword
swing. The old full-body static ki-charge pose is no longer used for holding Q/E.

Charge rises over 1.5 seconds and is limited to what the current energy pool can afford.
Damage reaches 1.75 times the base value for 1.5 times the cost. Radius grows where supported;
step distance and lance range also increase. Wards and veils gain duration, and wards gain
absorption. Held colour shifts toward a hotter pale tint; a thin wind rim accelerates and one
64-slot emitter raises small motes around the body. Release/cancel removes both effects.
Fireballs use one smooth shared sphere with small ember trails. Three shared colour levels
show the released projectile's charge without allocating a new material for every shot.
Two-handed magic gathers and launches from the midpoint of the palms. Hand sprites use the
bones' world transforms, rather than their local offsets. A small shared sphere grows and
changes colour at that midpoint during charging. Previews use the same origin.
Beams use the right palm and retain a one-handed release while firing; their muzzle follows
that bone while the impact point remains fixed. Chained segments start at the previous impact.

Lances request the existing glacier/plasma energy-beam presets. All presets share four tube
meshes and one flare quad. Up to four requested beams may be active in the default/high
quality modes (two in low); requests beyond the cap omit decoration while damage still resolves.
Casting keeps the shoulder view through release, and short beams reach their brightness
quickly instead of spending most of their lifetime fading in. The camera aim ray starts at
the caster's depth so a villager behind the caster cannot redirect a shot backward.

The supplied StatusFXFree Godot pack contributes the scrolling charge-rim pattern and baked
frost/arcane particle masks. Its CC0 source references and hashes are preserved under
`assets/vfx/statusfx`; the game loads no Godot scenes or runtime.

## Rendering budget

Normal play defaults to 60 Hz frame pacing with VSync, and 15 Hz while unfocused.
`GB_MAX_FPS=30..240` changes the focused target. This is a frame scheduling target; it does not
promise that a slow scene can sustain the requested rate.

| GB_VFX_QUALITY | Particle emitters | Animated quads | Fading mesh accents |
| --- | ---: | ---: | ---: |
| low | 16 | 24 | 24 |
| medium (default) | 32 | 48 | 40 |
| high | 48 | 72 | 64 |

Emitters are admitted before Hanabi compiles/extracts them for the GPU. New effects nearest
the player take priority, and new particle emitters beyond 80 metres are discarded. Rejected
decoration does not remove the underlying projectile, damage, aim, or collision entity.

Particle capacities are 128 per trail, 256 per impact, 64 per casting hand, and 96 per shockwave.
The default cap permits at most 8,192 requested particle slots across admitted emitters; GPU
allocator padding and other rendering resources are additional. The old per-emitter capacities
were 4,096 / 2,048 / 1,024 / 1,024 respectively.

Projectile cores, crescents, ward bands and ripple geometry share mesh handles and static
materials where possible. Lightning changes brightness without regenerating its ribbon mesh.
Projectiles and impacts no longer create transient point lights. Small sprite accents replace
large overlapping glow shells. The `[vfx-budget]` log reports active/peak emitter and quad
counts, rejected decoration, and total mesh/material asset counts every five seconds.

GPU utilization still depends on the map, resolution, other applications and hardware. The
budget bounds these decorative effects; it does not cap all GPU memory or GPU utilization.

## Focused validation, 23 October 2026

The optimized development build passed. Rendered runs for each sword school completed their
three-cut chain, charged finisher and guard, then exited normally. The held-spell run rendered
the moving preparation, charge rim and rising particles without shader validation errors.
After the palm-placement change, separate rendered runs captured the sphere between both
palms and the beam at the extended right hand; both exited with code 0. Captures are
`two-palm-charge.png` and `one-palm-beam.png` in the artifact directory below.
The ten-spell showcase exercised every newly added spell and exited normally. No panic,
unknown animation track, or mesh allocator use-after-free was observed in these runs.
Empty imported/generated meshes are kept out of zero-byte GPU allocations.

The 1280x720 controlled village spell showcase recorded 25 whole-GPU samples: utilization
averaged 30.6%, peaked at 50%, and total GPU memory peaked at 2,753 MiB. A subsequent 22-sample
run with shoulder-framed casting averaged 30.3%, peaked at 41%, and used up to 2,914 MiB total.
These include startup and other applications; combat NPCs were removed by the showcase
driver. This is not a full-combat performance benchmark. Effects peaked at six admitted emitters and six animated
quads, with zero rejected effects; mesh count stayed at 1,808 throughout the showcase.

Logs, CSV and captures are in ignored `staging/kev/magic-polish/`. The host still reports
missing Epic Vulkan overlay JSON files. Kev Studio changes passed JavaScript/Python syntax
checks; its browser runtime review was blocked by automatic approval review. Combat balance,
all environment/pose clipping and sustained full-combat performance remain for the requested
later playtest.
