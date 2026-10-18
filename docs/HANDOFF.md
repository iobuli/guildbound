# Guildbound overhaul: handoff

State as of 2026-10-17, branch `overhaul-2026-10-16`. The machine-readable copy is `docs/handoff.json`; keep both in step.
Read this file first, then the module header (`//!`) of any file before you edit it.

## Build and run

- Toolchain: `CARGO_HOME=D:\dev\cargo`, `RUSTUP_HOME=D:\dev\rustup` (moved off C:). If a shell predates that, set both and put `D:\dev\cargo\bin` on PATH.
- `cargo build -p guildbound` is the verification bar. The user runs `play.cmd` and checks visuals at milestones; don't run the game or take screenshots yourself.
- Pause for a user playtest at every playable milestone.
- Workspace: `crates/game` (the game, bin `guildbound`), `crates/sim` (game uses only `sim::terrain::ground`), `tools/gb_lab` (old physics lab), `crates/posenet` (outside the workspace). Old docs are in `docs/archive/`, and unused asset dumps are in `D:\gb_assets_archive\`.

## Module map (`crates/game/src`)

| File | Responsibility | Status |
|---|---|---|
| `main.rs` | App, 120 Hz fixed tick, Avian (8 substeps), plugins | done |
| `motion/donor.rs` | GLB parse (gltf crate): hierarchy, skin joints, curves | done |
| `motion/retarget.rs` | Donor clips → canonical UAL2 skeleton (name copy / Mixamo bind-delta) | done |
| `motion/extract.rs` | Dense clip → tagged extreme keys | done |
| `motion/timing.rs` | Stylized timing: holds, snaps, stepping on twos, exaggeration | done |
| `motion/bank.rs` | `PoseBank`: tracks, masks, `assets/anim/poses.json` overrides (hot reload) | done |
| `motion/rig.rs` | Spawn model on `ModelRequest`, bind bones → `Rig` | done |
| `motion/secondary.rs` | Per-bone angular springs, arm world inertia + acceleration trail (`Arm swing` knob), chest/neck/head nod against vertical acceleration, impulses, squash | done |
| `motion/body.rs` | Body capsules (+ the root's `BodyPadding` for worn armour); swings arms out of torso/head/opposite thigh and thighs apart (after springs, before contact) | done |
| `motion/contact.rs` | Foot IK (heel + ball), pelvis drop only as far as a planted foot can't reach by straightening its leg (swing feet ignored), step-bounce dip (`ContactRequest.bob`, fades where the feet stand at different heights), COP lean, look-at, hand IK | done |
| `motion/compose.rs` | **Only bone writer**: layers → springs → body → contact → bone `Transform`s; also publishes the pose in model space as `RigPose` | done |
| `motion/tweak.rs` | F2 pose tweak: freeze the player on a track key (PgUp/PgDn track, [ ] key), click or , . a bone, arrows swing (camera axes), wheel twists, Shift fine, Backspace reset, Ctrl+S merges into `poses.json` `edits` (hot-reloaded); player input is ignored while it is on | new |
| `play/mod.rs` | Shared types: `Intent Mover Action Fighter Loadout Hit StaminaSpend Interactable Climbable Water Feel`, capsule consts, `spawn_character` | done |
| `play/world.rs` | Terrain, pond, stairs, mantle boxes, climb walls, props, interactables, characters | done |
| `play/input.rs` | Keyboard/mouse/gamepad → `Intent` | done |
| `play/camera.rs` | Orbit camera, lock-on framing, `CameraShake` | done |
| `play/controller.rs` | Kinematic capsule (MoveAndSlide), floating support probe, steps, jump, knockback | done |
| `play/select.rs` | **Only writer of `MotionLayers.layers`** + `ContactRequest` (incl. the `Bounce` character: step dip, its acceleration in `accel.y`, squash rhythm, idle breath) | done |
| `play/interact.rs` | Focus, `offer` (per held tool; `requires` → head-shake refusal), radial stands, walk-in, use clips, `beat` (effect / loop landing + hitstop freeze; tool loops land when the blade stops, `Swing`), `reach` IK windows, emits `UseEffect` / `UseBeat` | done |
| `play/tools.rs` | `Holding` + `ToolItem`: the axe on the table, left-hand pickup, hand-over to the right hand, carry/chop grips, settle on every re-parent | new |
| `play/voxel.rs` | `VoxelGrid` (carve, split, `mesh`, `smooth_mesh`) and the shared `Lattice` + `surface_nets` mesher for any density field (each quad wound as a whole, split along its flatter diagonal) | done |
| `play/armor.rs` | Plate armour (bascinet, one cuirass with arm holes, pauldrons of 3 concentric spherical lames, rerebraces, vambraces, cuisses, greaves: 16 plates) built at startup on the async pool, one thread per plate, from the body mesh: ray-cast profiles → designed shells (gap and thickness along the normal) → Surface Nets + UVs (cylindrical with a seam split, band, or azimuthal about the pole) + tangents + Metal049C with mips generated at load; each plate rigid, fitted per frame to the skin under it (Kabsch / Müller, warm-started; pauldron lames rotate only about the shoulder joint, so they never cut into each other); `BodyPadding` derived from the built plates; armour stand + E, plates leave one by one | new |
| `play/grass.rs` + `assets/shaders/grass.wgsl` | Lush grass: chunked LOD blades, vertex-shader wind (gust fronts, flutter), trampling by `Mover`s, `GrassWind` (incl. a `gust` slot for wind magic) | new |
| `play/items.rs` | Item entities (log, stick, stone, apple): physics drops on `Phys::Debris`, magnet flight into the bag, hill scatter + respawn spots | new |
| `play/inventory.rs` | `Inventory` (required by `Player`, 20 stacks), I-key bag panel, pickup toasts | new |
| `play/trees.rs` | Voxel trees drawn smooth: notch carving on `UseBeat`, chips/shake/recoil/sound, felling into a `Phys::Debris` body, landing, breaking into logs/sticks/apples, regrow; `chop` / `fell` / `drop` log lines | new |
| `play/fx.rs` | `Burst` particles (chips, leaves) and `Sfx` one-shot sounds (first audio in the game) | new |
| `play/hud/` | Prompt (F), log, minimap with a STATS rim button, vitals (HP ST / BW MP), hotbar (Q art, E spell, weapon in hand, bag), `stats_page` (tabs STATS, ARTS & MAGIC and SKILLS; K; `GB_STATS=<tab>` smoke hook) | done |
| `play/deeds.rs` | `Deed { who, kind, tag, amount }`: the only input to progression. Producers: combat (hit, block, parry, kill), magic (cast, cast_hit), ranged (shoot, shoot_hit), traverse (swim / climb per metre, mantle), parkour (land Roll / Hard), trees (chop, fell); `skills` turns `ItemCollected` into gather | new |
| `play/record.rs` | `GB_RECORD=<dir>`: frame-locked capture (each frame advances 1/`GB_RECORD_FPS` s of game time; frames `GB_RECORD_FROM`..`GB_RECORD_TO` saved as PNGs, then quit) so slow software rendering still makes normal-speed video (`ffmpeg -framerate 30 -i f_%05d.png`) | new |
| `play/traits.rs` + `assets/data/traits.json` | The madlib "a [temper] fighter who walks [walk] and fights [fight]": NPCs roll one word per slot (seeded by entity); walk → `motion::compose::StyleScale` (exaggeration, holds, bounce; compose and select apply it per character), temper + fight → `ai::Difficulty` shifts (clamped); the log introduces each NPC | new |
| `play/hall.rs` | The Guild Hall complex on the flat terrace east of the courtyard (`world::TERRACE_*`, `terrace_distance`): a gate and path, a stone hall (west door, north door to the yard, pitched roof, plank floor, sconces) with swinging double doors (`Door`: open for anyone near, no collider), the front desk and ledger shelves, an indoor bounty board, the lodging (3 `bed`s, a table, 2 chairs), and the fenced training ground with the duel ring, dummies and a weapon rack | new |
| `play/beam.rs` | Balance beams: a grounded player standing on a `Beam` balances (`Balancing`): the stick walks along it (slowly) and its sideways part counters an inverted-pendulum sway (drift grows with speed, shrinks with the Balance skill); `select::Posture` leans the torso and narrows the stance, both arms reach out through `HandTargets`; past the wobble line the arms windmill, past the fall line you topple off. Deeds `balance` (per m), `beam` crossed / fell. `GB_BEAM=steady|drift` | new |
| `play/aim.rs` | Aiming helpers: `ballistic` (launch direction that lands a shot under gravity), `lead` (aim where a moving target will be), `Velocities` (every mover's velocity per frame). Used by `ranged` and `magic`; the player's aim point is a crosshair raycast (`input`) | new |
| `play/senses.rs` | Enemy detection: sight (cone, world blocks rays) and the player's aura as a magnet (`AuraPresence.power` pulls the enemy's fan of sense rays; captured only at `CAPTURE` pull, still blocked by objects); `Awareness` rises/decays/forgets; `ai` picks the player only once alerted. `Veiled` hushes the aura; `sneak` deeds. Vision volumes = procedural cone meshes with the wind-aura shader | new |
| `play/debug.rs` | Debug toggles: F12 list, 1–0 / Shift+1–3 flip 13 views (vision volumes, sense rays, aura radius, awareness, AI intent, IK targets, grass, balance tilt, projectile trails, wind shell, alt skin shader, ground layers, hide model pieces); `GB_DEBUG=1,2,4` | new |
| `play/wind_aura.rs` + `assets/shaders/wind_aura.wgsl` | Wind-aura material (fresnel shell with wind-combed streaks); every Aura Ward gets a wind shell and bends the grass outward (`GrassWind.gust`) | new |
| `play/skin.rs` + `assets/shaders/skin.wgsl` | Every character's skinned body gets its own extended material: ink look (toon bands, ink silhouette, shadow hatching), per-character shadow accent, and for enemies a presence rim warming with awareness plus a white blink + eye glint when alerted | new |
| `play/looks.rs` | Character looks by name: model height/breadth, accent, bone-attached procedural pieces (kasa, hood, topknot, headband, scarf, sash, pauldrons, pouch) with spring tails | new |
| `play/ground.rs` + `assets/shaders/ground.wgsl`, `weather.wgsl` | Terrain material (macro drifts, triplanar rock strata, grass breakup, worn paths from vertex alpha); stone/wood props weathered (moss, grime streaks, wear specks) | new |
| `play/ruins.rs` | The ruins on the north hill (raised plateau `world::RUINS_*`): torii, pillar ring, arches, broken wall, flagstones, arena ring, dais (future boss), stone lanterns, banners, rubble; footpath from the courtyard | new |
| `play/talk.rs` | Talking to `Talkable` NPCs: F (as `Intent.talk_pressed` when one is in focus) → `Talking`; the camera eases into the player's eyes and frames the NPC's face (first person, body hidden, 42° lens); a dialogue panel with numbered options (keys 1–7 / click); `Dialogue` written by `guild`; `GB_TALK=<name>` | new |
| `play/guild.rs` + `assets/data/guild.json` | The guild's people (Mira the receptionist, Captain Varro, adventurers Kael and Sena: `Civilian`s with routines, chatter you overhear), `Purse` gold, Mira's desk (turn in contracts for gold + XP, the Guild Card appraisal, selling items, bounties by victim name, rank applications, beds that sleep you to dawn), and the rank exam: a sparring duel with Varro in the ring (yield at 15 % hp or a ring-out); `GB_GOLD`, `GB_CARD`, `GB_EXAM` | new |
| `play/sky.rs` | Day/night: `Clock` (a day per 24 game minutes, start 09:00, `GB_HOUR`, F9 +1 h) moves the sun (east 06:00 → west 18:00) and a fainter, cooler moon at night; light colour warms at dawn / dusk; sky, fog and ambient follow; the minimap readout shows the time | new |
| `play/occupations.rs` + `assets/data/occupations.json` | Civilians at work: a `Civilian` (no brain fights it, it fights no one) with an `Occupation` routine of interactable kinds: walk to the nearest free one, press interact like the player (`interact` walks it in and plays the clip), stay on looping uses, move on. A Smith (anvil, well, chair, torch) and a Farmer (seed, water, harvest, well, noticeboard) | new |
| `play/contracts.rs` + `assets/data/contracts.json` | 12 contract templates; objectives are deed patterns × count (kills are tagged with the victim's name, so "Defeat the Rival" works); 3 open on the `Board`; finished → `stats::GrantXp`, log, next template; stats page WORK tab | new |
| `play/insignia.rs` | Title badge: a small glowing diamond on the left breast of anyone with a title, coloured by the newest title (hue from its id), placed each frame from the chest bone (`spine[2]`) and the body's facing | new |
| `assets/data/titles.json` | 12 titles (Blade Adept, Duelist, Slayer, Bulwark, Aura Master, Pyromancer, Frostweaver, Stormcaller, Spellblade, Wallrunner, Tidewalker, Woodsman): `requires {skills: level, deeds: lifetime count}`, flat `bonus`; earned in `skills::earn`, shown on the SKILLS tab | new |
| `play/skills.rs` + `assets/data/skills.json` | Use-based skills (lane C): 16 rows (`SkillBook`), RuneScape curve 1–99, XP capped per skill per 30 s window; undiscovered skills build affinity and roll `base·a/(a+4)` with a pity floor (per-character seeded); arts and spells are discovered once their skills reach the `unlocks` levels; `Sheet` (required by `Fighter`) for player and NPCs; skill levels → `stats::SkillBonus` | new |
| `play/stats.rs` | Universal `Stats` on every `Fighter` (level, Vigor Endurance Strength Dexterity Intelligence Faith, points, xp) → `Derived` (max hp / stamina / poise / magicules / Battlewill, might, spell, regen, cast speed); kills give the player xp, levels give points (`SpendPoint`) | new |
| `play/armory.rs` | Weapon chest: opening the courtyard chest shows the weapon rack; a click sends `combat::Equip` | new |
| `play/vfx.rs` | `bevy_hanabi` effects per element (trail, burst, cast motes, shock debris) with procedural sprites and per-particle motion algorithms (ABC flow fire, Vogel phyllotaxis frost, vortex arcane), plus CPU meshes (fractal midpoint-displacement lightning with branches, aura crescent, shock ring), impacts with glow and light flash; `GB_VFX_TEST` rig | new |
| `play/surfaces.rs` | CPU-generated tileable detail + normal maps (ground, stone, rock, wood, metal, cloth, sand) with mip chains; `world_uvs` box-projects world-scale UVs (`PROP_TILE` 1.5 m) and generates tangents; terrain UVs are (x, z) / `TERRAIN_TILE` (3 m) | new |
| `play/tuning.rs` | F1 live tuning panel (Exaggeration 0.9–1.4, Arm swing 0–2, Bounce 0–2), F5 saves `profiles/feel.json` | done |
| `play/monitor.rs` | F4 joint-relation monitor + `staging/relations.log` | new |
| `play/parkour.rs` | Graded landings: soft / momentum roll / hard land (`Owner::Move`); logs `land_*` lines | new |
| `play/moveset.rs` | Moveset JSON schema + loader (`assets/anim/movesets/*.json`); `WeaponDef.mesh_rotation_deg` / `mesh_offset` orient glTF weapon meshes so the blade runs along the root's +Y | done |
| `play/combat.rs` | Melee FSM (light chains, charged heavy, sprint attack, air plunge, guard / parry, roll / sidestep, stamina, poise), swept hit tests, **only `Hit` resolver**, hitstop, reactions, death / revive, weapon in the hand | done |
| `play/ai.rs` | Rule-based NPC brain → `Intent` (no network, no language model): spacing to `combat::reach`, circle-strafe, attacks in reach, reads the foe's `Melee::tell` to guard, parry late or dodge, retreats when hurt, walks home; `Difficulty` knob; `enroll` inserts `DamageScale` + `Revive` | new |
| `play/ragdoll.rs` | Impact ragdoll (F6 on the player): on `RagdollRequest` (combat above `Feel.ragdoll_threshold`, or F6 on the player) 11 jointed `Phys::Ragdoll` capsules spawn at the bones, limits centred on the pose at the hit; read back into a raw full-body `Source::Pose`, the root follows the pelvis; settled → stand on the ground facing the feet, `ual2/LayToIdle` get-up (`Owner::Ragdoll`); corpses stay ragdolled until combat revives them; a new request kicks the segments again | new |
| `play/ranged.rs` | Movesets of kind `ranged` (`bow`, `crossbow`, `knives`, from the chest): RMB aims (upper-body aim clip, half speed, crosshair), LMB fires; the bow draws (hold, power grows with the draw), the crossbow reloads, knives throw in fans of 3. Shots arc under gravity, sweep body capsules, emit `Hit` (`HitKind::Projectile`, × might); arrows and bolts stick in the world; `GB_SHOOT` hook | new |
| `play/magic.rs` | Arts (Q, **Battlewill**: built by landing / taking melee blows, trickles back) and spells (E, **magicules**: regen after a pause; MB5 cycles the bound spell). `ABILITIES` table: 5 arts (Crescent Edge, Flash Step, Aura Bullet, Earthbreaker, Aura Ward) and 4 spells (Cinder Bolt, Frost Bloom, Storm Lance, Orbit Motes), cast windows from measured clip peaks; shapes Bolt / Crescent / Quake / Lance (chains) / Orbs (Lissajous, homing) / Step / Ward (a shell that turns aside 60 % of damage and poise for 5 s; spinning gyroscope bands); mastery per landed blow → **Mastered** (cheaper, faster, stronger, bigger effects) at a permanent magicule cost; known = discovered (`skills::Sheet::abilities`); `GB_CAST` demo hook | new |
| `play/traverse.rs` | Swim (chest under the surface, bob, sprint costs stamina, wade out; **dive** with C toward the floor while 12 s of breath last, Space surfaces; `dive` deeds train Swimming; `GB_DIVE` hook), wall climb (jump at a tall `Climbable` face; stick climbs / shimmies; hand and foot holds on a grid; jump kicks off, dodge lets go, stamina runs out → fall), **ledge hang** at the top (hands on the edge, sideways edges along it, down drops back, jump or 0.5 s of pushing up pulls over), mantle over the top, vault onto waist-high blocks (`ActionKind::Mantle`, scripted rise-then-over path); `GB_CLIMB` smoke hook | new |

## Frame order

1. `Update / PlaySet::Input`: `input` writes the player's `Intent`.
2. `Update / PlaySet::Think`: `ai` writes NPC `Intent`.
3. `Update / PlaySet::Act`: action owners (`combat ranged magic interact traverse`) run on `Action`, emit `Hit`, and request `Mover.root_motion`. `interact::animate_props` runs before `interact::drive`; `tools` (take/return, hand-over, grip, place) and `trees::chop` run after `drive` and react to its `UseEffect` / `UseBeat`.
4. `Update / PlaySet::Select`: `select` → `MotionLayers`.
5. `FixedUpdate / PlaySet::Move`: `controller` moves capsules.
6. `PostUpdate / MotionSet::Compose`: `compose` writes bones. After `TransformSystems::Propagate`, `monitor` measures.

## Ownership: one writer per quantity

| Quantity | Writer | Readers |
|---|---|---|
| `Intent` | `input` (player), `ai` (NPC) | everything in Act, `select`, `controller` |
| `Mover.velocity/grounded/mode(Ground,Air)/accel/landing/air_time/yaw` | `controller` | `select`, `interact`, monitor |
| `Mover.mode` Swim/Climb/Ledge | `traverse` | controller skips those modes |
| `Mover.mode` Ragdoll | `ragdoll` | controller, select skip |
| `Mover.phase` | `select` | `select` |
| `Mover.impact`, `Mover.takeoff_speed` | `controller` | `parkour` (grade), `select` (leap poses) |
| `Mover.step_ease` | `controller` (running total of drawn-root step easing) | `select` → `ContactRequest.root_ease` → `contact` |
| `Mover.root_motion / move_scale / face` | the current `Action` owner | `controller` (root_motion cleared after the frame's last tick) |
| `Mover.external` (knockback) | `combat` | `controller` decays it |
| `Fighter` (hp, poise, stamina, dead) | `combat` only; others spend stamina through the `StaminaSpend` message (`traverse`: climbing, fast swimming) | `traverse` (out of stamina), `ai`, `hud` |
| `RagdollRequest` | `combat` inserts it (knockback above `Feel.ragdoll_threshold`) | `ragdoll` consumes it, then owns the body, `Mover.mode` Ragdoll, `MotionLayers.raw` and the get-up `Action` (`Owner::Ragdoll`) |
| `Action` | its `owner`; starting one needs `is_free()`; `Owner::Hit` may interrupt anything | `select` (layers), owners |
| `MotionLayers.layers`, `.contact` | `select` (characters with `Mover`); `armor` for armour-stand mannequins | `compose` |
| `RigPose` | `compose` | `armor` (plate fits) |
| `BodyPadding` | `armor` (inserted on whoever wears the set) | `compose` (`body::resolve`), monitor |
| Armour plates, `ArmorSet`, stand `PropState.on` (worn) | `armor` | `interact` (`offer`) |
| Grass chunks, `GrassWind` (`gust` for wind magic) | `grass` | |
| `MotionLayers.impulses` | `combat` and other play systems (append) | `compose` drains |
| `MotionLayers.raw` | `ragdoll` | `compose` |
| `HandTargets` | the action owner (`interact` now; `ranged`, `traverse` later); `tools` adds the right hand during the pickup hand-over | `select` fades → `ContactRequest.hands` |
| `ContactRequest.bob`, `accel.y` | `select` (scaled by `MotionFeel.bounce`) | `contact` (pelvis dip), `secondary` (nod) |
| `Holding`, `ToolItem`, tool parent/transform, table `PropState.on` | `tools` | `interact` (`offer`), `trees` (blade point) |
| Tree voxels, `Tree`, felled bodies, standing-tree part transforms (trunk, collider, canopy incl. the chop shake), tree `PropState.regrow` / `.shake` (set) | `trees` | `interact` (availability, shake decay) |
| Cursor grab / visibility | `camera::capture_cursor` only; `ui_mode` is derived each frame from the open pages (`BagOpen`, `TuningPanel.open`, F2) after the `PageToggles` set | input, monitor |
| `UseEffect` / `UseBeat` messages | `interact` emits | `tools`, `trees` |
| `Particle`s, `Sfx` audio entities | `fx` (from `Burst` / `Sfx` messages) | |
| `Item` entities, flying stand-ins, hill `Spot`s | `items` (spawned by `items`, `trees` via `items::spawn_item`) | `inventory` (via messages) |
| `Inventory` contents | `inventory` only (applies `ItemCollected`, answers `ItemRejected`) | `items` (room check before the magnet) |
| Bone `Transform`s | `compose` only (and `tweak` while active) | rendering, monitor |
| Visible pelvis height | `contact` only (it cancels the controller's step easing) | |
| Limb–body separation | `body::resolve` only | monitor |
| `Hit` message | `combat`, `ranged`, `magic` emit | **`combat` alone resolves** |
| `InteractPrompt` | `interact` | `hud` |
| `Feel`, `MotionFeel` | `tuning` | controller, compose |
| `Stats`, `Derived`, `DamageScale` | `stats` | combat (maxima, regen, might), magic (pools, spell power), HUD |
| `Deed` message | combat, magic, ranged, traverse, parkour, trees, skills (survive) emit | `skills`, `contracts` |
| `Board` (open, `ready`, `held` rank) | `contracts` (`TurnIn` → `Paid`, `RankUp`) | HUD, guild |
| `Purse`, `Guild` (bounty ledger, bed, exam, bout), `Dialogue` | `guild` | HUD, talk |
| `Talking`, `TalkFocus`, the camera during a talk | `talk` | input, occupations, camera (`ui_mode`) |
| `Sparring` (guild) / `Yielded` (combat); Varro's `Fighter.team` during a bout | `guild` | combat |
| `Honours`, `Standing` (holdings; NPCs seeded by `world`) | `contracts` (`honour`) | skills (bonus), insignia (gold), contracts (issuers), HUD |
| `Clock`, the sun's light and transform, `ClearColor`, `GlobalAmbientLight`, camera fog colour | `sky` | occupations, minimap |
| Civilians' `Intent`, `Occupation` | `occupations` (`work`) | interact, controller |
| `Sheet` (skill XP, affinities, discovered abilities), `SkillBonus` | `skills` (presets at spawn in `world`; the `GB_CAST` demo marks abilities discovered) | magic (`known`, casting), stats (`derive_with`), HUD |
| `Traits`, `StyleScale` | `traits` (on a new NPC) | compose, select (per-character style) |
| `Difficulty` | `traits` (rolled shifts), else `ai::enroll` (default) | `ai` |
| `SkillBook.1` (holders of unique skills) | `skills` | `skills::apply` |
| `Ward` (Aura Ward: time left, share) | `magic` | `combat` (scales damage and poise) |
| `Energy` (Battlewill, magicules), `Grimoire` (bindings, mastery, mastery debt) | `magic` (the stats page binds through `magic::Bind`) | HUD |
| `Loadout.current` | `combat` (`Equip` messages from `armory`) | select, HUD |
| `ArmoryOpen`, `StatsOpen` | `armory`, `hud::stats_page` | `camera` (cursor) |
| `Surfaces` (generated maps) | `world` (setup) | `world` palette and terrain |

## Data files

- `assets/anim/{ual1,ual2,sekiro}.glb`: donor libraries. `ual2.glb` is the model and the canonical skeleton.
- `assets/anim/poses.json` (optional): style plus per-key overrides. Hot-reloaded.
- `assets/anim/movesets/*.json`: one `Moveset` per file (schema in `play/moveset.rs`: `name, kind(unarmed|melee|ranged|magic|tool), weapon{glb path, hand, offset}, offhand, stance, light[], heavy[], sprint_attack, air_attack, guard{clip, parry}, aim{clip, draw, release, reload}, projectile, cooldown`). Every `track` is a bank name `"{lib}/{clip}"`. Shipped: `unarmed`, `katana`, `greatsword`, `dagger`, `axe` (melee). The player's loadout also names `bow crossbow knives fire frost`; they wait for the ranged / magic lanes.
- `assets/packs/weapons/*.glb`: longsword, shortsword, greatsword, claymore, knight_sword, battleaxe, mace, warhammer, spear, glaive, battle_staff, elder_staff.
- `assets/audio/kenney/impactWood_*`, `impactPlank_*`: Kenney Impact Sounds (CC0), chop / fall / pick-up sounds.
- `profiles/feel.json`: written by F5.
- `staging/relations.log` (gitignored): monitor output, rewritten each run.

## Monitor invariants (`play/monitor.rs`)

| Relation | Broken when |
|---|---|
| `knee_backward` / `elbow_inward` | flex ≥ 10° and the bend side points against natural (body front / out-down-back) with cos < −0.3 |
| `knee_pop` / `elbow_pop` | the bend side turns by cos < −0.5 in one frame |
| `foot_sink` | heel or ball more than 3 cm below the ground |
| `foot_float` | grounded, body and foot still, lowest of heel/ball more than 5 cm above the ground |
| `foot_slide` | grounded, foot down (< 2 cm), foot moving > max(0.6, speed / 2) m/s |
| `toe_inside` / `knee_inside` / `hand_inside` | the joint lies inside `Phys::World` geometry |
| `hand_miss` | `HandTargets` weight ≥ 0.95 and hand more than 5 cm from the target |
| `arm_in_body` / `legs_crossed` | `motion::body` capsules overlap by more than 1 cm |

Log line: `t character relation side value mode action speed`.

## Fixes this session

- Stairs: contact cancels the controller's per-frame step easing (`root_ease`), so the pelvis has one smoother. The two smoothers in series had made the body dip and rebound.
- Knees and elbows: `bend_pole` drops any bend component against the natural side and adds a small bias, so near-straight limbs keep a stable bend side. Limbs never extend past 99.9 % of their length, so they no longer lock and snap.
- Interaction: hands ride the moving grip part (`Interactable.grip`: lever arm, chest lid). The walk-in pivots on the spot before stepping when the target is behind.

## Skills, deeds, discovered arts (latest milestone, awaiting playtest)

- Lane C v1 (`docs/CHARACTER_SYSTEMS.md` answers: hybrid progression, discovered arts, exact numbers, permanent skills). The six stats and levels stay; skills rise from deeds on top and add small bonuses to `Derived` (`SkillBonus`, per level from each row's `bonus`).
- The player starts with Crescent Edge and Cinder Bolt. Everything else is discovered: e.g. Aura Arts 2 makes Flash Step discoverable, and casting / landing aura arts then builds its affinity until it is found (certain after 8 such deeds). Fire 1 → Cinder Bolt, Frost 1 → Frost Bloom, Lightning 1 → Storm Lance, Arcane 1 → Orbit Motes, Aura Arts 5 / 10 → Aura Bullet / Earthbreaker.
- The old mastered-ability "Skill" is renamed **Mastered** (`Grimoire::is_mastered`, `MASTERED_*`), so "skill" means only the new use-based skills.
- The Opponent has Swordsmanship 20 and Defence 12 and levels the same way.
- **F8** toggles the **Hunter** (team 3, bow, Archery 15): the brain's `archer` mode keeps 5–11 m away (back-pedals, closes in, strafes), aims, draws to full and releases (presses for crossbows / knives). `GB_HUNTER=1` at start. Guests are one `world::Guest` enum (F7 Rival, F8 Hunter).
- **Titles** (lane C): earned automatically from skill levels and lifetime deed counts (`Sheet::counts`), never lost, each a flat bonus; "Title earned: …" log and flourish. `titles.json` hot-reloads with `skills.json`.
- **Events** (lane C): world-unique skills, one holder at a time (freed when the holder is gone). **Second Wind** (+HP, +ST per level): survive a close call (under 15 % HP, alive 8 s later; `skills::close_calls` writes `survive`). **Spellscarred** (+MP, spell power): grows from being `struck` by spells. An NPC claiming one first is announced ("Somewhere, someone has claimed …").
- **Traits** (lane C): every NPC rolls a temper, a walk and a fighting manner; they change how it moves (bounce, pose exaggeration, holds) and fights (aggression, guard, parry, dodge, reaction, casting). The player picks theirs on the STATS tab: a clickable madlib ("I'm a [temper] fighter who walks [walk] and fights [fight]", `traits::SetTrait`); only the walk changes the player (style), the rest is flavour until the player has a brain.
- **Contracts** (lane C): a **noticeboard** stands just ahead of the spawn (F: read the notices → WORK tab). Three open at a time on K → WORK (Sparring Partner, Woodcutter's Errand, Bad Blood, Hold the Line, Flamework, Scale the Walls, Cold Water, Hunt the Hunter, Forager, Marksman, Courtyard Champion, Arcanist); progress comes from your deeds; finishing one pays character XP (`stats::GrantXp`) and posts the next. **Guild rank** F → S rises with contracts done (E 3, D 6, C 10, B 15, A 21, S 28); templates carry a `rank` and are only posted once you reach it (the Rival / Hunter bounties at D, Courtyard Champion at C). **Issuers**: characters with `contracts::Standing` (holdings) post their own work while they are about: the Smith (forge: logs, whetstones) and the Farmer (fields: apples, kindling); the WORK tab says who posted it. **Honours** (`honours.json`): Knight of the Courtyard (rank C + 2 titles; holds the Courtyard), Champion (rank B + Duelist), Lord of the Vale (rank A + knighthood; holds the Vale); each a flat bonus (folded in by `skills::publish`), holdings on the player's `Standing`, a log line, and a gold badge. Holdings yield their honour's `daily_xp` at every dawn (Courtyard 150, Vale 400). Unit-tested only (they need rank C+ in play).
- **Title insignia**: titled characters wear a glowing badge on the left breast; presets earn titles at once (`skills::entitle`), so the Opponent (Swordsmanship 20) wears Blade Adept. Placement checked on screen (left breast, just below the collarbone).
- **Senses, stealth, looks and the ruins** (latest):
  - Enemies notice you before they fight: by sight (walls hide you) or by feeling your aura, which pulls their sense rays like gravity when it is strong. Crouch to quieten it; the new **Veil** art (Stealth skill, from sneaking near unaware enemies) hushes it. Sparring bouts and peaceful mode (F10) are unchanged.
  - **Flash Step** blinks to your lock-on target (behind or beside it, facing it); cast in the air with no target it drops you to the ground in a **superhero landing**.
  - Every character is now distinct: ink-shaded skin, accent colours, proportions, and procedural pieces (your kasa, scarf and sash; Varro's topknot and pauldrons; Sena's hood; ...). Enemies' outlines warm as they notice you and flash white when alerted.
  - Aura Ward is wind: a streaked wind shell and the grass blown outward.
  - The ground has procedural detail and worn footpaths; stone and wood weather. North of the courtyard, a path climbs to the **ruins** under a red torii: the arena for the first boss.
  - F12 opens the debug toggle list.
- **Balance beam**: in the training ground, by the north fence. Standing on it you sway like an inverted pendulum; the stick walks along the beam and its sideways part is your balance (push against the lean). The torso leans with the sway, the stance narrows to the beam, the arms go out and tilt against it, windmill near the edge, and too far tips you off the side. Metres walked are `balance` deeds (the new **Balance** skill, whose levels damp the sway); crossing end to end is a `beam` deed (contract **Steady Feet**: cross three times). Verified headless: `GB_BEAM=steady` crossed the 9 m beam (`[beam] crossed tilt=-0.21`), `GB_BEAM=drift` fell after 2 m (`fell tilt=1.36`).
- **The Guild Hall**: through the gate east of the courtyard. Inside, Mira at the front desk (F: first-person talk). Finished contracts now wait until you turn them in at her desk for **gold** and XP; contracts completed make you *eligible* for the next rank, and the rank card is earned by **applying at the desk and beating Captain Varro in the training-ground ring** (both spar to 15 % hp; a ring-out also loses). She also appraises you (Guild Card), buys gathered items (Log 4, Stone 3, Stick 1, Apple 2 gold), pays bounties (Opponent 20, Rival 40, Hunter 40) and rents beds (10 gold: sleep to 06:00, fully restored). Kael and Sena sit at the lodging table chatting (you hear them nearby), read the bounties, sleep at night; Varro drills at the dummies. Both doorways have double doors that swing open as anyone approaches. Verified headless (`GB_CHOOSE`, `GB_ITEMS`, `GB_BOUNTIES`, `GB_PASSED`): selling 3 logs + 2 stones paid 18 gold, a Rival bounty 40, collecting issued the rank E card, the bed rents; both door leaves open as you walk up and close behind you (`[hall]` log lines, `GB_WALK`); Kael and Sena trade lines at the table; a recorded exam ended in a yield at the 15 % floor. Sleeping verified with `GB_SLEEP=1`: hurt to 30 hp at 09:06, rented bed, sat on it → "slept until dawn", 100 / 100 hp at 06:02.
- **Day and night**: a full day every 24 game minutes; at dusk the sky goes orange then deep blue, torches carry the night, the moon lights it faintly. F9 skips an hour. Civilians switch to a `night` routine (sit, tend the torch).
- **Occupations** (lane C): the Smith works the anvil, drinks at the well, sits, tends the torch; the Farmer plants, waters, harvests, drinks, reads the board. Civilians are team 0, never targeted by the brain, revive at home after 8 s, and roll traits like NPCs.
- **Bow on a real archery clip**: `rokoko/Hawkeye_Archery` (measured: draw 7.95 → 8.3 s, anchored at the face to 9.3 s, release 9.38 s). The aim layer scrubs from the nock (`aim.draw.start`) to full draw (`hold` 8.7) with the pull; the release plays from 9.38. The clip shoots side-on, so `aim.body_yaw_deg` (−56, set from `bow_align` measurements) turns the body; arrows leave along the aim from the chest. While the bow is up, `ranged::hold_bow` stands the stave upright (canted 0.18 rad) and grips it mid-stave; `combat::arm` restores the carry grip afterwards.
- **Measured, not eyeballed** (`staging/relations.log`): `ledge_hang` lines give the hand bones' height against the edge while hanging (now −0.01 / −0.065 m: wrists just under the top, fingers over it; `HANG_REACH` 0.37 from these numbers); `bow_align` lines give the angle between the drawn arrow line (string hand → bow hand) and the aim at each shot (now 0.6–1.7° in all: yaw from `body_yaw_deg` −56; pitch from the new `ContactRequest.aim`: while a ranged weapon is up, `select` passes the aim direction and `contact` bends the three spine bones about the horizontal axis across it, less the clip's own shot pitch `aim.pose_pitch_deg` (11° for the archery clip); unit-tested in `motion::contact`).
- **Faulds**: two nested hoops round the hips under the cuirass (`FAULD_TOP` 1.135 leaves a gap below its 1.18 bottom so the spine bends; lower over the hip sides than at the front / back, where the thigh folds). 18 plates now; torso padding 0.065 m.
- **Diving** (lane B): in the pond, C dives toward the floor (breath 12 s, refills at the surface ×3), Space comes up.
- Ward hits flare: sparks and a soft clank where a blow meets the shell.
- **F7** toggles a second enemy, the **Rival** (team 2, greatsword, purple; Swordsmanship 15, Defence 8). Teams differ, so the brain sends each NPC at its nearest foe: the Rival and the Opponent fight each other as readily as the player. A ragdolled Rival leaves once it is back up. `GB_RIVAL=1` brings it in at start; `GB_FIGHT=<moveset>` puts the player on the NPC brain (`ai::Autopilot`) with that weapon.
- Kill XP goes to the killer (the last to strike the victim, read from `Hit`), player or NPC: NPCs level up from their kills too.
- NPCs cast: the brain (`ai::cast`, pure and tested) throws the bound spell from 1.5–12 m past its reach and uses the bound art up close, each a `Difficulty.casting` (0.35) chance per half-second look, 2.5–5 s apart, never mid-swing or guarding. The Opponent casts Crescent Edge / Cinder Bolt; the Rival Earthbreaker / Frost Bloom.
- **Aura Ward** (art, 30 BW): discoverable at Defence 5 (block and parry to train it).
- `skills.json` hot-reloads (checked every second; a bad edit keeps the old book and logs the error). The SKILLS tab keeps 8 spare rows for added skills.
- Training dummies count as deeds (practice teaches); the 600 XP per skill per 30 s cap stops farming.
- Tests: 171 passed, 1 ignored. The headless screenshot of the SKILLS tab was not taken this round (`GB_STATS=skills GB_CAST=crescent_edge,cinder_bolt` with a long `GB_AUTOQUIT` does it).
- Playtest checklist:
  1. K → SKILLS: empty at first. Punch a dummy: "Discovered: Unarmed" with a flourish; levels log as they come.
  2. Cast Cinder Bolt at the dummy: Fire Manipulation appears; keep landing it and Lightning affinity shows under "Sensing".
  3. Swim, climb, mantle, roll-land, chop and fell a tree, pick things up: Swimming, Climbing, Breakfall, Woodcutting, Gathering.
  4. K → ARTS & MAGIC: undiscovered rows read "· · ·" with the skill levels they need (or SENSED once those are met).
  5. STATS tab: the derived values grow a little with skill levels.
  6. F7: the Rival joins; stand back and watch it and the Opponent duel (slams, frost, crescents); F7 again sends it away.
  7. Block and parry until Defence 5, then keep blocking: Aura Ward is discovered; Q with it bound shows the spinning shell, and blows hurt less.
  8. F8: the Hunter shoots from range; it and the others all fight each other.
  9. Reach Swordsmanship 10: "Title earned: Blade Adept" (K → SKILLS lists titles).
  10. Watch the log at start: each NPC is introduced with its traits; a heavy walker bounces, a light one glides; a reckless one presses, a careful one guards.
  11. Get under 15 % HP and survive 8 s: Second Wind (unique) may be discovered.
  12. K → STATS: click the madlib's words; "walks heavily" bounces, "lightly" glides.
  13. The Opponent wears a small glowing badge on its left breast (Blade Adept).
  14. K → WORK: three contracts with exact progress; finish one (e.g. land 30 blows) for XP and a new posting.
  15. Swim into the pond, press C: dive to the floor; Space or running out of breath brings you up.
  16. Armour stand: the faulds close the waist under the cuirass; put it on and raise a knee (sprint, kick) to check the thigh clears them.
  17. The Smith kneels at the anvil, then wanders to the well, the chair, the torch; the Farmer works the plots.
  18. F9 a few times: dawn, noon, dusk, night; the minimap readout shows the time; at night the Smith sits and tends the torch.
  19. Bow from the chest: hold RMB to aim (side-on), hold LMB to draw (string hand to the face), release to loose; the stave stands upright.
  20. K → WORK over a few contracts: the Smith's and the Farmer's postings appear ("from the Smith"); bring logs / stones / apples / sticks.
  21. Climb the 4 m wall by the east side: at the top you hang from the edge; A / D edges along it; S drops back; W (held) or Space pulls over. Check the hands sit on the edge.
  22. Walk through the gate east of the courtyard into the Guild Hall: doors swing open; F on Mira (first-person talk); Appraise me shows the Guild Card.
  23. Finish a contract: it waits ("report to the guild desk"); turn it in to Mira for gold. Sell your logs / stones; rent a bed and sleep (dawn, full health).
  24. With 3 contracts done, apply for rank E; talk to Varro in the yard; win the ring duel (or yield at 15 %); collect your card from Mira.
  26. Walk up behind the courtyard Opponent while crouched (it should not turn); stand in its view ("Opponent spots you"). F12 → 1, 2, 4 shows its vision cone, rays and awareness bar.
  27. Lock on (Tab) and press Q with Flash Step: you blink beside the target. Jump and press Q without a target: you drop to the ground in a superhero landing.
  28. Walk the footpath north from the courtyard to the ruins.
  25. The balance beam along the yard's north fence: step up from the stone block; walk with W, and push A / D against the lean (the torso leans, arms spread; let it go and the arms windmill, then you fall). Cross it end to end ("Crossed the beam.", Balance skill, the Steady Feet contract).

## Stats, magic, weapon chest, controls (previous milestone)

- Controls: **F** interact, **Q** art, **E** spell, **MB5** next spell, **K** stats page, **F6** ragdoll yourself, **F7** rival on / off, **F8** hunter on / off, **F9** +1 hour, **F10** peaceful, **F12** debug list; **F** on a guild NPC talks (keys 1–7 choose, Esc leaves). Weapons come from the courtyard chest, not number keys.
- Setting: arts run on Battlewill (aura of the body: no chant, almost no wind-up), spells on magicules (a short cast). Mastering an art or spell makes it cheaper, faster and stronger, and costs magicules for good. The player has no devouring skill: kills give experience only. Inspired by the user's Tensura notes; every name is original.
- Effects were checked headlessly on the CPU renderer (`GB_VFX_TEST=1`, `GB_CAST=<ids> GB_CAST_EVERY=0.3`, `GB_SPAWN=x,z,yaw`, `GB_SCREENSHOT_AT` in game seconds): the lightning, glow and particles render; sizes were raised for readability. Bloom is on (HDR camera, intensity 0.12).
- Heavy attacks held to full charge strike by themselves; a clank and a small camera kick mark the moment.
- The camera, hit tests and HUD were verified with screenshots; feel (cast timing, effect sizes, BW gain rates) needs the playtest.
- Playtest checklist:
  1. F the chest, wield each weapon from the rack.
  2. Q / E casts; MB5 cycles spells; more arts and spells are discovered through skills (K → ARTS & MAGIC to bind).
  3. Kill the Opponent: XP, LEVEL line, the dot on the STATS button; spend points with +.
  4. Watch BW fill as you land blows; MP refill after a pause.
  5. Armour stand: the set now covers upper arms and thighs.

## Melee, NPC brain, world surfaces (previous milestone)

- Moveset windows (`windup` / `active` / `cancel`) come from the donor clips' hand and foot speed peaks (forward kinematics on the GLB data), e.g. `ual2/Sword_Regular_A` strikes at 0.20–0.27 s of 0.43 s, `ual1/Punch_Jab` leads with the LEFT hand at 0.13–0.17 s, `sekiro/Zombie Kick` kicks with `foot_r` at 0.53–1.15 s. To re-check them in the bank: `GB_PROBE="ual2/Sword_Regular_A,..." cargo test -p guildbound clip_probe -- --ignored --nocapture` (`motion/bank.rs`).
- Weapons: katana, greatsword and dagger are `sword` primitives; the axe is `battleaxe.glb` turned by `mesh_rotation_deg` [90, 0, 0] with its grip (z = 0.17) moved to the origin. All are held at the axe's `FIST` point, blade along the thumb side (`rotation_deg` [0, 90, 90]). If a blade sits wrong in the hand, tune these two fields in the JSON.
- The Opponent (katana, team 1) now fights: `Difficulty::default()` = reaction 0.28 s, parry 25 %, dodge 20 %, guard 35 %, aggression 0.6, damage ×0.8; it revives at its spawn after 6 s.
- Surfaces: detail maps are near white (≈0.75–1.0) and multiply the palette colours and the terrain's vertex colours, so hues stay as tuned; textured palette colours were lifted ≈15 % to keep their brightness. Every prop spawned through `Kit` gets UVs and tangents. Trees, leaves, crops, water and flame stay untextured.
- Playtest checklist:
  1. Keys 1–5 (unarmed, katana, greatsword, dagger, axe): tap and hold LMB on the training dummies, sprint attack, jump attack (plunge).
  2. Fight the Opponent: it closes in, strafes, attacks, guards, sometimes parries or dodges, and gets up at its spawn after dying.
  3. Weapons sit in the right fist; the axe head faces the swing.
  4. Terrain, stone, wood, sand, cloth and metal show surface detail in the sunlight, with no seams or stretching on boxes.
  5. `staging/relations.log` shows `hit_*`, `parry`, `guard_break`, `knockdown`, `death` and `revive` lines.
  6. Ragdoll: F6 throws the player backwards; a greatsword heavy or plunge (knockback over 7 m/s) ragdolls the Opponent. Bodies fall, settle and get up facing their feet; a killed Opponent stays down until it revives at its spawn.
- Audio: Bevy's `wav` feature is on (`crates/game/Cargo.toml`); `audio::tests::every_sound_decodes` catches an unsupported sound format before it crashes the game.

## Grass and armour (previous milestone)

- Armour design facts: the bind pose equals the skeleton rest (FK) to 1 µm and the mesh is in model space, so plates are authored directly on the T-pose body and placed by `rigid fit(rest samples → skinned samples)`. A review simulated the first design on real clips (Python replica against `ual2.glb`) and found plates fitted to different bones passing through each other, so the set was redesigned: no belly lames (one cuirass, flared rolled bottom, arm holes cut below the hanging arms' armpits, pelvis skin in its profile), pauldron lames are concentric spheres about the shoulder joint with strictly increasing radii (`nest_radii`) and `Fit::Pivot` (rotation only), gap and thickness are scaled by the profile slope so steep areas keep `THICK` (11 mm, over the 4.8 mm lattice diagonal), and `BodyPadding` is measured from the meshes (`derive_padding`) instead of constants. Clearances (gap): cuirass 2.6 cm, vambrace 1.2 cm, greave 1.3 cm, bascinet 1.6 cm, pauldron lames ≥ 2.05 cm apart radially. Rotation extraction runs up to 32 Müller iterations and stops once a step is under 1 µrad (a cold start needs ~30, a warm frame a few).
- Grass: WGSL compiles only at runtime; if grass is missing, check the log for a `shaders/grass.wgsl` error. Startup logs `[grass] N blades ...` and `[armor] built N plates (V vertices), padding ...`. The flutter clock wraps at `FLUTTER_WRAP` (3600 s) and the gust-noise scroll at `SCROLL_WRAP` (2000 m); the shader rounds flutter frequencies to whole cycles per wrap and repeats its noise lattices over the scroll period, so neither wrap shows (constants duplicated in `grass.rs` and `grass.wgsl`: keep them equal).

## Axe, trees, bounce (earlier milestone)

- Clip facts behind the numbers: `TreeChopping_Loop` (0.967 s) is a one-handed right-hand chop that leads with the pinky edge; the axe lands at ≈0.17 s with the hand 0.54 m ahead. `PickUp_Table` (0.833 s) reaches with the LEFT hand (≈0.45 m ahead, 0.87 m high, 0.2–0.3 s) and brings it across to the right side of the chest by 0.5 s; the right shoulder pulls back out of reach.
- Calibrate from `staging/relations.log`: `chop` lines give the blade's distance to the bark (`value`, negative = inside) and its height; tune `trees::STAND`, `tools::{FIST, CHOP_TILT, AXE_GRIP}` from them.
- Playtest 1 (2026-10-16): legs folded too much on stairs. The pelvis dropped by the full step for the lower foot, even mid-swing; now it drops only as far as the leg cannot reach (`LEG_REACH` 0.97 of its length) and swing feet are ignored. The chop beat fired with the axe overhead (blade 1.3 m from the bark, 2.03 m up), so tool loops now land when the blade stops. The `chop` line also logs `hand=(x,y,z)` in the user frame (clip impact is (0, 1.06, 0.54)) and `clip_t`.
- Known rough edge: putting the axe down flies it from the right hand to the table over 0.22 s (the clip only reaches with the left hand).

## Next lanes

Each lane stays in its own stub files. The shared files (`play/mod.rs`, `select.rs`, `world.rs`) take small additive edits only: record them here.

- **A. Melee:** done (`combat.rs`, melee movesets, `ai.rs`, `ragdoll.rs`). Possible next steps: a face-down get-up clip (only `LayToIdle` exists, so bodies lying face down roll over during the get-up), and an active ragdoll (PD drive toward the animated pose).
- **B. Done** (traversal, ranged, magic). Diving, the bow's archery clip and the ledge hang (with edging) done; the hang uses the climb pose with hand IK on the edge (no hang clip in the libraries). NPC archers (F8 Hunter) and casters are done.
- **Next for magic:** done this round: NPC casters and the Aura Ward. Ward-hit sparks done. Faulds and ward mastery (each blow the ward turns is a `ward` deed: mastery and Defence XP) done.
  - **Tightrope / balance beam (user idea): done** (`beam.rs`). Possible next: a high beam / tightrope over water, NPCs (Sena) practising on it, a balance skill check in the rank exam.
- **C. Character systems: v1 done** (`deeds.rs`, `skills.rs`, `assets/data/skills.json`, SKILLS tab). Hot reload, titles, events (unique skills) and traits done. Player madlib, title insignia, contracts and the noticeboard done. Occupations, the day clock, guild rank, NPC issuers and honours with player holdings done. Holdings yield XP at dawn. Next: the player posting work from holdings, occupations, contracts, events, traits (the madlib → per-character `MotionStyle`), stat-weighted affinity, NPC casters using discovered spells.
- **D. Done:** `motion/tweak.rs` (F2). Possible: N to author new keys (`authored` tracks).
- **Parkour reference:** `docs/reference/parkour_jump.md` (frames pending: Vimeo needs a login, so drop the mp4 at `staging/ref/parkour.mp4`).

Rules: keep the module header docs in step with the code. Use the monitor log to back motion claims. Stop for a playtest at each milestone.
