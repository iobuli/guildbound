# FreeGuild - merged build specification

Generated export. Authoritative GDD: latest MASTER_GDD block in TANDEM_REASONING.log.

# FreeGuild - Game Design Document v1 (core prototype)

Status: Reviewed design baseline v5.1, including the accepted amendments in the latest source block. P0 remains unchanged. Exported for the boss on 11 October 2026. Numbers marked (H) are tuning hypotheses, not verified results. UNKNOWN means not yet established. Section 12 is the reusable master build prompt.

---

## 1. Identity, audience, pillars, promise [Codex; Claude reviews]

Promise: Read a committed action, choose your response, and bring useful work home. The settlement becomes easier to move through and prepare in because it can rely on you.

Fantasy: a capable field member of a small guild. Close, grounded fights against people; a growing elemental attunement visible on the body; one guild hall that changes because of the order in which you chose to help.

Audience: single-player progression and self-expression. Players who enjoy reading bodies and space more than reading numbers.

Pillars (stable; everything else in this document is revisable):
1. Grounded physical combat. Bodies and space are read before numbers. Every exchange has preparation, contact and recovery.
2. Procedural animation layer. Authored motion is the source; a bounded layer makes contact, support and transitions physically true. The mesh never does what the game did not decide.
3. Elemental progression on the body. Growth is a new deliberate thing the body can do, never a hidden multiplier.
4. A single guild hall hub. The hall is a place, not a menu: board, clerk, sparring floor, preparation offer and the door to both routes.

Design rules:
1. Every service change alters a route, a threat or a tool, never only decoration.
2. The owned assets are a modular building kit. New forms come from recombining owned pieces by role (surface, frame, roof, threshold, enclosure, service prop, landmark), measured proportion changes, source-mesh edits where needed, material/texture derivatives and animation retiming/rig correction. Colour change alone is never the twist of a derived assembly (a cloth tint can be one small part of a larger assembly). Sources stay untouched; every derived piece traces to its source module.

## 2. Gold path and session loop [Claude]

Session loop: choose an obligation (stated risk, payment, unlock) -> leave through the hall door by one of two routes -> read and handle a humanoid opponent, or avoid them -> recover the objective into the single expedition slot -> return and deliver -> payment, service change and new option happen as one event -> the next trip is a different decision.

Gold path (each step has a pass condition; the driver in section 11 plays it):
1. Wake on the hall bench. PASS: camera, HUD, board, clerk, sparring floor and door are visible within 3 s; input legend shown.
2. Clerk/board: accept obligation A or B. PASS: route, risk, payment and service result visible before accepting.
3. Sparring floor: four skippable, replayable prompts (verb checks belong to the fixture/practice track) (light phrase, guard, step, ready cast), then a practice partner shows preparation/commitment/recovery and a guard that invites the throw/heavy choice. PASS: each verb performed once; partner reacts by contact direction and guard state.
4. Leave by the door; first opponent on the chosen route. PASS: opponent defeated or avoided; zero hits registered without contact.
5. Recover the objective. PASS: slot shows it; a second pick-up is impossible.
6. Return and deliver. PASS: payment + service change + new option in one event; the changed gate or preparation offer is visible from the hall door.
7. Attunement exercise on the sparring floor, offered by the clerk after the first delivery. PASS: charged cast unlocked and performed once.
8. Second obligation, completed: recover, return and deliver exactly once. PASS: second payment and second service verified. The fixture also demonstrates that a different route or tactic was available (a test scenario, never a restriction on the player).
9. After both persistent deliveries, a free practice replay is used to test defeat once (scripted by the fixture, never a forced loss in the tutorial), retry. PASS: back on the same bench; rewards and services intact; objective available again; zero coin needed.
10. Capstone duel on the sparring floor (full arc only). PASS: won with at least one guard break and one punish.
The P0 core prototype is accepted on steps 1-9. Step 10 is required for the full six-contract arc, not for P0.

## 3. Verbs, states, input, camera [Claude rules / Codex feel]

Verbs: move, run, light strike (two-hit phrase), heavy strike, guard (hold), lateral step, throw, cast (plain; charged after attunement), interact, traversal jump.

Input (remappable, legend on screen): WASD move, mouse look, Shift run, LMB light, F heavy, RMB guard, Ctrl step, R throw, Q cast (after attunement: release before the charge threshold = plain cast; reaching the threshold switches to the charged release; moving cancels only before commitment; threshold, maximum hold and auto-release are tuned in the M0.5 table; no infinite hold), E interact, Space jump, middle mouse lock toggle, Esc pause/menu. No attack or cast during a traversal jump in the prototype.

Action states: Idle/Move -> Prepare -> Active -> Recover -> Idle; plus Guard, Guard-broken, Step, Throw-reach, Throw-paired, Cast-prepare, Cast-charge, Cast-release, Hit-react, Defeated. An action buffer accepts the next input during late recovery (0.25 s, H) without cancelling commitment.

Camera: third person, keeps the target and both fighters' preparation/support area legible. Lock-on changes facing and framing only, never reach or aim snap. Must keep telegraphs visible at walls and corners (tested with real colliders).

Feel (Codex): calm loaded stance, compact preparation, committed path, one sharp contact accent, restrained recovery to guard. The second light's preparation grows out of the first's follow-through. A miss finishes its arc with no spark and no pause.

## 4. Combat and opponents [Claude rules / Codex feel]

Contact truth: damage only on verified geometric contact, at most once per target per action. Each hit of the light phrase is its own action with its own contact identity. A swept volume may enter a surface before time-of-impact correction: judge the RESOLVED visible contact pose within a calibrated tolerance (H), not zero penetration at every sampled pose. Support/contact fitting after resolution can never create another damaging contact or enlarge reach. Contact is swept from the limb's previous final position; world obstruction stops contact before any actor behind it. The logical outcome is decided before its visual accent. Animation never sets damage. Damage = attacker stat x move factor (light 1, heavy 2) (H).

| Option | Beats | Loses to | Cost (H) |
|---|---|---|---|
| Light phrase: start 0.25, active 0.10, recover 0.30 | throw reach-in, cast preparation | guard, step | recovery after the phrase |
| Heavy: start 0.55 (clear wind-up), recover 0.50 | guard (large capacity drain) | light during wind-up, step | long tell and recovery |
| Guard (hold, frontal) | light, heavy | throw, side/rear contact | finite capacity: light -15, heavy -45; holding blocks regen; regen starts 0.8 s after release; at 0 = guard break 1.0 s |
| Lateral step 0.35 s, evasive 0.08-0.22 | heavy, throw, cast | light phrase (recovery is hittable) | 0.4 s cooldown; no instant cancel into guard |
| Throw: reach 1.2 m, reach-in 0.30, needs legal staging and landing space | guard | any strike during reach-in, guard release, step | whiff recovery 0.6; illegal space = visible failed attempt, never a body through a wall; mutual throws = both stagger, neutral reset |
| Cast (plain): ready from the start, cooldown on commitment | distance | strikes during preparation, step | stationary preparation; moving before commitment cancels with no projectile; interruption after commitment spends the cooldown; no melee refund |
| Cast (charged, after attunement): held, then released | guard (pushes back), range | interruption during the hold, step | slower, stationary, larger impact radius with falloff, maximum area and push distance set in the M0.5 tuning table; area never reaches through walls; blocked and world-occluded targets defined there; hits the world before targets behind it; bounded effect area that cannot be farmed and never hides the opponent's next tell; a declared behaviour choice, not a multiplier |

Safe-tactic audit: turtling empties capacity and is thrown; mashing meets guard then punish; zoning with casts is closed down during preparation; reflex stepping is caught by light phrases.

Opponents: one humanoid kit, two patterns in the prototype, each punishing one habit.
- Pressure: approaches, clearly prepares a two-hit attempt, pauses, resets; punishes long casts and throw attempts. Forces guard/step.
- Defensive: visibly guards, backs off after a failed throw; offers the heavy/throw decision. Forces throw/heavy.
- Later (P1): rival pattern using the player's own verb set (step, throw, cast).
Opponents obey the same reach, commitment and capacity rules. Starting values (H, tuned in M0.5): player 100 HP; opponent 60 HP; light 6, heavy 12, plain cast 10, charged cast 14 damage at the base stat. Avoiding: an opponent engages within its watch radius (about 12 m, H) and leashes back to its post beyond about 20 m (H), so a route can be passed without fighting. Retrying never makes an opponent secretly faster or stronger. The first fight is strictly 1v1; multiple attackers are a later content gate.

Outcome bodies (Codex): hit = defender recoils from the real contact direction; block = the contacting limb arrests against the guard and the guard loads; guard break = guard opens, centre drops, recovery obvious; throw = visible reach-in, then the staged pair only if space is legal; miss = uninterrupted arc and recovery. Height-specific reactions follow contact location (presentation, not a rule). Contact hold 0.025-0.045 s on the involved fighters only (H); no global time scale; the hold never extends invulnerability or cooldowns.

Procedural animation layer (P0 system). Order per tick: collect intent -> advance the single collision motor and gravity -> sample the curated clip/transition -> form bounded limb targets -> sweep the active limb -> resolve earliest contact and one outcome per target/action -> fit the visible contact and support pose to that outcome -> render.
Invariants: the collision motor is the only actor mover; limb lengths are fixed; aim correction is bounded and can never move a planted foot; an unreachable target makes the strike miss or end safely; foot contacts come from reviewed clip phases; support pivots at the planted contact instead of sliding; feet release for steps and strikes that actually travel; only curated transitions are guaranteed continuous.

## 5. World, contracts, places, narrative frame [Codex]

The hall: one guild assembly built from the tavern and village kits: a strong framed threshold from supports and roof pieces, one cloth/banner accent, a quiet open sparring floor, a service edge from counter/bench/chest pieces; its roof and door silhouette stays recognisable as it grows. One traversable hall and courtyard. Board, clerk, sparring floor and preparation station are all visible from the entrance by shape and light. The door frames the two route approaches and their landmarks. First defeat returns to a known bench. Service changes are visible from home base.

Routes (each about 150 m, H):
- Exposed timber road: open, long sightlines, opponent visible early; favours casting and choosing to avoid or engage; one quiet fight space with lateral room.
- Sheltered path: cover, short sightlines, ambush corners; favours melee and stepping; teaches observation and retreat.
A safe route is always traversable at zero wallet with no item, even if longer.

Services are broken/restored ASSEMBLY PAIRS from the same kit (prototype set: 1 hall assembly, 1 gate pair, 1 preparation-station pair; the six-contract arc reuses this vocabulary). 1. Gate repair (the frame gains aligned planks and an open passage): a shortcut to the exposed route's objective (route). 2. Preparation offer (a dormant stall gains canopy, counter and a settled NPC routine): one optional out-of-combat item (tool). Each rebalances the other route, so the second trip is a fresh choice. Concrete order coupling: gate work opens the shorter return to the exposed recovery site; preparation work offers optional recovery before either route; neither locks an obligation or requires buying an item. Threat contrast comes from the two opponent patterns and sightlines. Lookout or training-post services are only candidates that may REPLACE a slot.

Contracts (six total; capstone is number six):
1. Recover the tool crate (exposed route) - prototype.
2. Recover supplies (sheltered path) - prototype.
3. Known-route skill test (a route revisited with a changed service).
4. Field variation under a condition that keeps telegraphs visible (dusk or fog on the exposed route).
5. Combined-tools challenge (requires the charged cast and a throw read).
6. Capstone duel on the sparring floor.
A two-opponent camp or a stationary crew defence may replace a field contract only after the multi-attacker gate passes; never added on top.

Narrative frame: opponents are people (deserters, rival crews, people touched by raw element). One sentence of context per contract on the board; the place tells the rest. No named external references anywhere.

## 6. Growth, economy, items, services [Claude]

Three linked outcomes, not three progress bars: combat competence is the player's own skill; guild trust grants contract access (from distinct completed work, never repeated enemies); town work changes a service or path. Projects can wait but never regress.

Elemental attunement (progression on the body):
- Band 1 (start): plain fire cast.
- Band 2 (after the first delivery, taught on the sparring floor): controlled charge (enter, hold, release, interruption, fixed recovery). Prototype scope ends here.
- P1 candidates, each needing a real mechanical choice and counterplay: attuned guard (a perfect block releases a short push-back burst but consumes the cast cooldown); a second element as a loadout choice (fast, straight, reach) after the first is proven. A recoloured punch is never progression.
- Techniques beyond the attunement (future candidates, outside P0 and the fixed full arc unless they replace an existing technique slot): 1-of-2 sidegrades per trust band; choices, never flat stat ladders, so learned timing stays valid.

Character creator and trained builds (AMEND-CHR-01, P1):
- Base: one derived owned humanoid body per sex, faceted on a copy (spike evidence: Documentation/experiments/chr_spike). Topology is final before any shape key; clothing layers carry the same keys.
- Creator axes (first version): build mix over fighter / caster / heavy shape keys inside one normalized envelope; 3 head-shape choices; 2 hair caps; skin and outfit swatches from the palette (guild red reserved for the sash marker). Canonical actor scale: no height slider and no limb-length sliders until a later gate proves contact/support safety.
- Mastery vs appearance: MASTERY is permanent and never decreases; it is earned only from resolved useful actions (strikes/throws that land, casts that land, deliberate frontal guards/brace reads), deduplicated and bounded per encounter, never from lost HP, whiffs, practice replays or dragging a fight out. Per unique contract delivery the TOTAL mastery increase across all axes is <= 0.08 (H), distributed by contribution. APPEARANCE (displayed body blend) is chosen or suggested in the dressing corner from earned mastery; "keep my current appearance" always works. Markers owned through mastery never disappear.
- Fairness: the build is presentation. Collision capsule, limb contact volumes, reach, CombatStats and rewards never derive from the mesh. Every approved morph stays inside a bounded visible contact envelope; if a body would need larger invisible hitboxes, its shape range is reduced instead.

Economy: one wallet. Each distinct contract pays once; replays pay nothing. Accept, retry and hall recovery cost zero. One optional preparation item (carry cap 2, H) that shortens an out-of-combat recovery and is never required. Nothing can be sold back. No unbounded farming or sink economy.

## 7. Save, failure, recovery [Claude]

Attempt states: available -> accepted -> carried -> delivered (terminal), or reset -> available. One carried objective at a time.
Delivery checks the active attempt and objective, then commits payment, service and access together exactly once.
Save: complete snapshot plus a recoverable previous file, written at the hall after accept, delivery, purchase, abandon, attunement completion and settings changes; never during combat. A completed lesson survives quitting immediately afterwards. Learned technique state is stored separately from transient charge, cooldown and paired-animation state. After reload a reset attempt offers a fresh objective; completed contracts remain free practice with no duplicated payment. Load always resumes in the hall with no active combat; an undelivered attempt resets without reward and its objective is available again. Never restore a pending throw, cast, scene reference or half-paid service.
Failure: defeat or abandon returns to the bench at full HP; rewards, equipment, trust and services persist; only the current objective is lost. No softlock: starter contracts are free and re-acceptable; a safe route always exists; no coin or item is needed to finish any contract.

## 8. Visual style, motion, audio, UI, accessibility [Codex]

Look: one wood/stone/cloth palette unifies the two building kits; combat floor and sightlines are kept clear before any clutter; the moving character silhouette is always the highest-contrast shape. Motion follows the kit principle: curated owned clips are the vocabulary, and timing, transitions, contact/support correction and restrained effects make one physical language. Warm cloth and wood, quiet stone and ground, controlled vegetation. Value contrast concentrates on interactive silhouettes; broad quiet ground under fights; foliage at the edge of decision spaces. Effects are capped so hands, body and telegraphs stay readable. Fire: warm core, a hand-sized charge whose size and pulse change together (colour is never the only cue), short readable trail.

Motion: see sections 3-4. Outcomes get different bodies before different colours.

Feel budgets (H, judged at 30/60/120 fps): action buffer 0.25 s; contact hold 0.025-0.045 s; one small contact spark with a fast-fading accent; heavy contact may add a low dust pulse; camera response optional, capped near 1 degree / 4 cm, reducible to zero.

Trained-build readability (AMEND-CHR-01): identity comes first from garment mass, pattern and stance, with the body morph as support (spike: safe morph magnitudes read subtly).
- Fighter: broader upper back/forearm surface within the envelope, compact loaded home stance, wider layered wrap bands; markers: wrap stitching/repair bands, compact shoulder patch.
- Caster: tapered vest/torso contour (never longer arms), composed braced casting stance, dark forearm bands with one then two short warm ember seams; ready/charge glow is FX-owned and only in those states.
- Heavy: fuller lower torso/thigh surface and padded vest silhouette within the same envelope, stable grounded stance, broad horizontal reinforced cloth bands.
At 20 m mass and stance carry identity; near the mirror the material markers reward inspection. The segmented joints of the owned body read as a carved, articulated figure (fits SD-A).
Trained-build mass (AMEND-CHR-01b): the revised concept silhouettes (trained-builds-v2) exceed the 2.7 cm maximum body shift measured in the morph lab; the visible mass is carried by garment volume and the same fixed contact envelope applies to the garment, so the contact-envelope gate must be run on body + garment together.
Dressing corner (the creator is a place, not a menu): beside the sparring floor, out of its combat space, built from T-BENCH + T-CHEST + V-RACK-CLOTHES + a frame derived from T-POST/T-WALL. E enters a close full-body three-quarter preview (front/side/back), tabs Frame / Head / Colours, a short preview track (idle, guard, slow step). Changes stay pending: Apply commits and saves once at the hall; Cancel/Esc restores. Free, revisitable, skippable, never blocks leaving. Shows a suggested trained-body change with the actions that earned it.
Depth maps (optional, offline): Depth Anything V2 SMALL (Apache-2.0) run locally on concept images. Use: sculpt reference and exploratory texture relief from an isolated, nearly flat garment patch aligned to the garment UVs (broad form removed). Estimated depth is view-relative, never measured geometry; Blender baking from the actual derived garment owns surface truth. Never owns topology, hitboxes, dimensions or skinning.

UI (AMEND-UI-01): uGUI Screen Space Overlay, original art, never a state source. Values: HP, guard capacity, fire readiness/charge, carried objective with return instruction when relevant; wallet and trust at the board/preparation station; learned techniques shown as learned/locked with the reason, separate from the ready/wait cooldown cue.
Look (from primitives): translucent deep-blue panes (fill #0B1A3A), thin cyan edges (#5FD7FF), angular corner brackets, header bars with a notch, white primary text (#EAF6FF), amber (#FFB347) for warnings, red (#FF4A5A) only for critical. Diegetic reason: the elemental attunement gives the guild member a cool "sense overlay"; cool UI over the warm timber world keeps contrast high. The two reference images the boss supplied informed colour and shape language only; none of their characters, text or artwork enters the game or the exports.
Layers (each a CanvasGroup with its own opacity and toggle): L0 screen accents (vignette, edge warning; optional, never a combat oracle); L1 panel backplates (9-sliced frames); L2 meters (tinted white masks, filled radial/horizontal); L3 icons; L4 runtime text and values (TextMeshPro, never baked into art); L5 context prompts (visible only when relevant, fade 2 s out of combat (H), never in the centre third); L6 alerts (3 s auto-dismiss (H), max 2 stacked, dismissing grants nothing); L7 modal windows (board, dressing corner, pause, settings; world dims to 60 %).
Opacity rules (contrast math measured: text #EAF6FF on plate #0B1A3A = 15.6:1; a plate at 55 % alpha over a bright sand world falls to 4.5:1, the minimum): every plate behind L4 text keeps alpha >= 0.70 (worst case 8.1:1), independent of master/layer opacity; L4 glyph alpha floor 0.85; critical alerts keep effective alpha >= 0.90; master opacity multiplies layers but is clamped by these floors; contrast >= 4.5:1 is measured text vs plate, never vs the world. Player settings: master HUD opacity, per-layer opacity for L0/L1/L5, text size, effects-on-HUD off, reduced motion (pulses and fades off).
Art production: one candidate sheet of cells traced to hud_sheet_manifest.json (64 cells, 8 layers, image type, 9-slice borders, tintable flag). Generated size is measured, never assumed; a sheet is accepted only when the measured slicer reports zero cells needing attention and the canonical 2048 export is assembled at native cell scale. Full-screen effects and tiles (vignette, low-HP pulse, damage wedge, scanline, grid, glow) are generated in code, not painted. Simple meter fills and pips (HP fill and trail, guard segments, charge fill, cooldown sweep, trust pips) are also drawn in code as filled masks; generated art supplies frames and decoration only, and any mechanical white-mask normalisation is recorded as a derivation. Corner brackets and lock-on markers are intended four-part sprites and count as one cell. Each sprite is traceable to its cell; replace single cells, never regenerate the sheet for one fix.
Fallback: the HUD works with primitive built-in shapes if the art layer is absent; the blue art is a replaceable presentation layer and never blocks gold-path state proof.

Accessibility (P0): remappable controls with a live legend, adjustable text size, toggle options for lock and guard, camera shake and hit flash reducible to zero, non-colour status cues, pause.

Audio: the installed content has zero audio files. The prototype is a silent proof that teaches all timing visually. Production sound is a listed gap; required cues: contact, block, guard break, step, throw, cast charge and release, footfalls, hall ambience.

Concept art pipeline (resolved): GPT image (Codex built-in) is the primary concept path; Higgsfield is an optional fallback, not a required system. Claude writes a structured spec per image; Codex renders, judges against this section and approves or rejects (max 3 attempts, then simplify). Spec fields: id; purpose (style_frame | pose_ref | texture_source | ui_language | blender_guide | palette); subject in primitives only; pose/action in plain anatomy; camera; light; palette (proposed targets until sampled in-engine); materials; source_piece_ids / verified_paths; reference_images (our contact sheets or assembly captures); assembly_changes; invariants_to_preserve; constraints (negative list: no text, logos or named styles); requested_transparency; acceptance_criteria; approval_state; use_in_unity. Size, count and lens are requested intent, recorded with the output, not guaranteed switches. Limits: no guaranteed seamless tiling, multi-view consistency, UV layout, exact dimensions or correct anatomy; texture sources need a seam/scale/UV pass before becoming material derivatives; front/side guides are references, not blueprints; pose references are aesthetic targets, never collision, reach, support or transition oracles. Generated additions the kit cannot build are rejected. Order: after the GDD, three frames (hall assembled from the kit; gate broken/restored; preparation stall dormant/active); pose and texture work only after the rig/source proof.

## 9. Asset-to-system map, twists, gaps [Claude evidence / Codex twists]

| System | Installed source | Twist |
|---|---|---|
| Locomotion | 8-way run/strafe with root motion, jump/fall/land, roll | retimed to the motor; feet from reviewed contact phases |
| Strikes, reactions | fighting-motion strikes; head/body/low hit and block reactions; knockdown/get-up | curated set, retimed to section 4 windows, contact hold and outcome bodies added by the layer |
| Throws | paired grab/throw clips | legal-staging check, failed-attempt variant |
| Casting | one/two-hand casts, "Load" hold + cast, casting enter/idle/exit/damage | plain and charged states; hand feedback |
| Hall NPCs | idles, talk, vending, carry, chant | routines change with services |
| Hall and village (kit roles: surface, frame, roof, threshold, enclosure, service prop, landmark) | modular tavern interior + furniture; village houses, stalls, fences, gate, well | warm relight, guild palette on cloth, board from crates/planks, gate broken/repaired states |
| Routes | terrain tiles, rocks, painterly trees, bushes, grass | one material palette; foliage density by sightline rule |
| Effects | element charge/missile/impact prefabs; sparks, dust, shields | shader repair to URP; scale and colour to palette; capped counts |

Evidence: Documentation/catalog-summary.txt, Documentation/catalog-clips.txt, Logs/catalog/, Logs/sheets/tavern.txt and town.txt plus their contact sheets. Installed clip existence does not prove usable retargeting; effect art needs new owned behaviour and shader repair; no skipped demo component becomes our implementation. Verified piece vocabulary (Logs/sheets tavern/town): floor tiles, straight/window/door walls, supports, straight/corner/side roofs, benches/tables/counter/chests; houses, awnings, fence/gate pieces, sheds, wood-path modules, terrain/rocks, foliage. Derivation record: every derived assembly lists its source pieces, transforms and material derivatives.

Character pipeline (AMEND-CHR-01): owned body -> facet on a copy -> build keys from one continuous displacement field (cluster-identical deltas, hands/feet/head protected) -> derived clothing and hair pieces carrying the same keys -> texture layers (palette + optional depth-derived relief + mastery markers) -> export to a derived folder -> Humanoid Avatar configured in Unity -> deformation and contact-envelope gates. Derived pack geometry stays out of any public repository (store licence: assets may ship inside the game, raw/derived files are not published).

Gaps (explicit, not hidden): final hero appearance (choose among owned rigs; rig proof decides); all audio; UI art (own generated blue system-window art at candidate stage, see S8; primitive uGUI is the fallback); creatures (none; all opponents human by design). Known prep work: effect materials with missing shaders (render magenta) and materials still on the built-in shader; several effect prefabs reference missing scripts (art ingredients only).

Stack: installed editor 6000.5.1f1 is not LTS. The boss's constraint is Unity LTS: target Unity 6.3 LTS (6000.3.x, supported to Dec 2027). No migration during this GDD task; after it, M0 runs a compatibility/migration gate on a preserved copy or branch, re-resolves package versions and reports actual incompatibilities.

## 10. Content budget and priorities [joint]

Prototype content: 1 hall + courtyard, 2 routes, 2 recovery sites, 3 NPC roles (clerk, stall keeper, repair crew), 1 opponent kit with 2 patterns, 1 element (plain + charged), 2 obligations, 2 services.
Systems (10): input/motor/camera; combat state + contact resolution; procedural animation layer; AI patterns; contract/attempt state; service/world flags; save snapshot; primitive HUD + access options; interaction; asset prep.
P0: gold path steps 1-9, all ten systems, plain + charged fire, contracts 1-2, both services.
P1 (full arc): contracts 3-6 including the capstone (gold path step 10). Character creator + trained builds + marker tiers (AMEND-CHR-01) are P1; P0 keeps one approved faceted hero body. Condition variant, rival pattern, lookout or attuned guard may only REPLACE an agreed contract/service slot, never expand the budget silently. Six contracts and two services remain the full content budget.
Not needed unless every P0 passes: second element, multi-attacker fights, crew defence, crafting, inventory beyond the slot and one prep item, extra sites, weapons, dialogue trees, day/night cycle, production audio (gap), final hero art (gap).

## 11. Milestones, gates, verification [Claude]

| Milestone | Learning question | Gate |
|---|---|---|
| M0 Stack + canvas | does it run on the LTS target? | 6.3 LTS compatibility gate passed on a preserved copy/branch; zero console errors; effect shaders repaired; curated clip list + retime tables committed |
| M0.5 Rig + combat spike | does cross-pack motion survive our corrections? | small spike with the chosen clips and a minimal owned collision motor, implementing enough contact and charge to prove the layer (M2 hardens this same system, never a second implementation); tuning table recorded (charge threshold/hold/area/push, HP, damage); then: one hero rig on the sparring floor does stance, light phrase, heavy, guard, step, throw, plain and charged cast against a partner and passes the layer gates at 30/60/120 fps; on failure change clips or rig before building anything else |
| M1 Greybox gold path | does the loop make sense with blocks? | steps 1-6 pass; step 9 as an isolated defeat/retry fixture with seeded completed-delivery flags and primitive services (it does not claim the M4 attunement/second-trip flow); save snapshot + reload tested |
| M2 Combat core | are fights readable? | layer gates pass; a novice explains hit/miss/block with HUD and effects hidden |
| M3 Opponents + services | do patterns force different reads; do services change the next decision? | both patterns beaten by a deliberate read; 4 of 5 testers name the changed route/threat/tool |
| M4 Attunement + hall | is progression visible on the body? | steps 7-8 pass; testers describe what the charge changed |
| M5 Feel + presentation (Codex lead) | does it look and feel like section 8? | reviewer >= 7/10 per area; three worst defects fixed before new features |
| M6 Acceptance | does it hold end to end? | P0 (two contracts): gold path steps 1-9 green twice in a row on a clean build. Full arc: complete all six distinct contracts, including field variations 3-5, then capstone 6; each pays once and replay pays zero; run the full arc twice from fresh profiles. |

Always playable from M1: if a change breaks the gold path and is not fixed within 20 minutes, isolate the responsible change and fix or safely revert only that change. Never erase another developer's or the boss's work in progress; no blind commits or pushes.

Verification (the author never grades their own work; numbers and captures are both required at every gate):
- Gold-path driver: Play Mode fixture plays the gold path, captures the Game View at each pass condition, writes results.json (HP/coin/flag deltas, positions, frame time, console error count).
- Layer gates (fixtures; every tolerance is a starting hypothesis (H) until measured on the chosen rig, never an engine capability claim):
  * contact: 0 hits without limb-volume contact; resolved visible contact pose within a calibrated penetration tolerance (H); world contact before any actor behind it.
  * grounding: declared planted-foot drift against its world anchor < 2 cm (H) per stance; sole penetration below the support surface < 1 cm (H); fixed-length reach (no bone length change > 1 mm, H); support handover consistent at every curated handover. Pelvis adjustment is bounded, never moves the capsule and never hides an unreachable stance. A pelvis-height inequality is not evidence of balance.
  * transitions: measure UNEXPLAINED discontinuity against expected authored/root motion or the predicted transition pose, not raw bone speed (a fast punch is valid motion). Report endpoint jump residual and angular residual at each curated handover; preparation and contact must stay readable.
  * frame rate: identical logical outcomes at 30/60/120 fps.
- Named test matrix, each scenario with an independently stated expected result (oracle); add: second delivery; quitting immediately after attunement; (stress counts supplement it, never replace it): miss; front block; back hit; terrain obstruction; throw whiff at a wall; mutual throw attempts; charge interrupt; empty guard capacity; duplicate delivery attempt; abandon; reload mid-attempt; interrupted save write; zero-wallet retry.
- Stress (supplementary): 1000 scripted exchanges and 10 000 random economy/save sequences report 0 contact violations, 0 duplicate rewards, 0 regressed services, 0 states needing coin or items to progress.
- Character gates (AMEND-CHR-01): Unity import of the derived body with blend shapes and a configured Humanoid Avatar; every clip, paired throw and cast across the morph envelope; clothing layers follow the keys; contact-envelope tests (visible misses never register, visible contact never misses, fixed reach, guard, feet, throws); clipping tolerance through the 8 combat and 4 cast poses; readability: 4 of 5 independent reviewers identify each trained archetype at 10 m and 20 m at the gameplay camera, repeated in grayscale with effects off (H).
- HUD gates (AMEND-UI-01): required values glance-readable at 1080p over the brightest and darkest world frames at the lowest allowed opacity; no L0-L6 element covers the centre third during combat; every plate behind text >= 0.70 alpha and text-vs-plate contrast >= 4.5:1 (measured with the actual colours); L4 stays above its floor at master opacity 0; reduced-motion removes all pulses/fades; the sheet passes the measured slicer with zero cells needing attention before it is called production; every sprite traces to its cell; HUD observes state and never mutates it (alerts replay no reward).
- Kit review: source piece list vs assembled guild silhouette vs before/after growth states; one cohesive place, each new form traceable to the kit.
- Reviewer evidence checklist: same fight with HUD/effects off and on; actual vs presented hit/block/miss counts; interrupted charge; empty guard capacity; throw whiff at a wall; repeated recovery inputs; feet on flat, slope and corner; service state before/after; attunement used on the second trip; retry at zero wallet; interrupted save loading into the hall.

## 12. Build prompt (master prompt) [joint; written last]

Mission: build the FreeGuild core prototype in Unity 6.3 LTS from this GDD. Grounded physical combat, a procedural animation layer, elemental attunement visible on the body, and one guild hall hub, built only from modified Asset Store content already in the project.

Read first: sections 1 (pillars), 2 (gold path), 10 (priorities), 11 (milestones and gates). Build in milestone order. Do not start M1 until M0.5 passes.

Rules:
- Unity 6.3 LTS (after the M0 compatibility gate on a preserved copy/branch); new Input System only; HUD in uGUI so captures include it.
- HUD layers L0-L7, opacity floors and the generated-art acceptance rule are specified in S8; HUD art is a replaceable layer and state proof never depends on it.
- Modifications are made on derived copies; source packs and any existing uncommitted work are preserved.
- Only installed store content, grown as a modular kit (recombination, proportion change, mesh edits, material derivatives, retiming); never colour-only; sources untouched; derivation recorded per assembly; pack demo scripts are never our code; no named external games, films, studios or brands in code, data or text.
- Implement the rules exactly as tabled in sections 4, 6 and 7; numbers marked (H) are starting values to tune, never claims of tested behaviour.
- The collision motor is the only actor mover; animation never sets damage; contact is verified geometrically.
- Distinguish in every report: verified assets, proposed tuning numbers, unresolved gaps.
- Decide, log the decision, continue. A gold-path break not fixed in 20 minutes: isolate and fix or safely revert only the responsible change; never erase other work in progress.
- Every gate needs both the fixture numbers and the reviewer captures. Zero console errors and zero missing references at every gate.
- Codex delivers code uncommitted; Claude verifies and commits; no pushes without the boss.

Deliverables: the playable prototype passing M6, reporting P0 acceptance (two contracts, steps 1-9) separately from full-arc acceptance (all six distinct contracts including field variations 3-5, then capstone 6; each pays once, replay pays zero; run twice from fresh profiles); final hero art, production sound and a second element stay explicit open items, never hidden acceptance dependencies; results.json and captures per gate; a decision log; the asset modification list per system.

---

# Art, state and systems - one build specification

This supplement connects the authoritative MASTER_GDD in TANDEM_REASONING.log to art ingredients, state ownership, HUD presentation and verification. It does not create extra gameplay systems. Runtime types and field names remain implementation proposals until code exists. The atlas embeds the latest GDD alongside this mapping.

## The ten systems and their visible contract

| Existing system | Owns | Presents through | Proof still required |
|---|---|---|---|
| Input, motor, camera | Intent, collision-constrained actor position and camera | Owned locomotion, clear body silhouette, context prompt | Motor alone moves actor; 30/60/120 outcomes; remapping |
| Combat state and contact | Action phase, verified contact, HP and guard | L2 meters, L4 values; tiny hit/block accents after resolution | Miss has no hit cue; frontal block differs from back hit; world first |
| Procedural animation | Bounded contact/support corrections and curated transitions | Planted feet, readable hands, local contact hold | Drift, penetration and transition residual measurements; fixed bones |
| AI patterns | Pressure or defensive decisions using the same rules | Role silhouette, preparation, guarded pose | Deliberate read beats each pattern; no hidden timing advantage |
| Contract/attempt state | One objective and available/accepted/carried/delivered status | L4 objective, L5 context; board's two P0 cards | Recover, return, abandon, reload; duplicate delivery pays zero |
| Service/world flags | Delivered contract changes gate or stall once | Owned-kit before/after assembly; L6 restored alert | Open gate changes route, stall supplies optional preparation |
| Save snapshot | Atomic hall snapshot and recoverable previous file | Brief save/delivery feedback; restored hall state | Interrupted write, reload mid-attempt, lesson persistence |
| HUD/access options | Read-only presentation, opacity and input preferences | L0-L7 layers; original blue/cyan visual language | Contrast at minimum alpha, clear combat centre, no state mutation from alpha |
| Interaction | Hall board, bench, lesson, stall; P1 dresser | Verb/key prompts, modal focus, Apply/Cancel | Out-of-combat validation, exit/cancel, no mandatory creator step |
| Asset prep | Derived copies, clip curation, shaders, meshes and recipes | Cohesive matte world, faceted body, generated HUD ingredients | Source hashes/derivation records, Unity Avatar, missing-reference/shader checks |

## One exchange, one source of truth

Input requests an action. Combat validates its state and resources. The motor supplies the collision-constrained position; clips supply intent; the procedural layer makes only bounded pose corrections. Swept limb/world contact decides the outcome. Only the resolved result changes HP/guard and selects reaction/feedback. HUD and effects observe that result. An animation frame, sparkle or UI fill never causes damage.

Plain fire is usable from the start. Completing the lesson after the first delivery unlocks held charge. Learned/unlearned charge is separate from ready/cooldown. The HUD's six-state art is a presentation vocabulary: RELEASE is a transition, INTERRUPTED is an outcome, and LOCKED means held charge is unlearned, never that plain fire is unavailable.

## Delivery merges body growth and hall growth

Recover objective -> return to hall -> validate active attempt -> atomically commit delivered ID, payment and service -> persist snapshot -> publish presentation feedback. Duplicate delivery changes nothing. P1 useful-action mastery is also committed at a unique delivery with a total increment capped at 0.08 (H); losing HP earns none. Mastery remains permanent; displayed normalized cosmetic blend is chosen/suggested at the dresser. Applying appearance is its own hall save transaction. Pending edits never change the saved body; Cancel/Esc discards them. Gameplay stats, reach and collision never derive from appearance.

No reward or service is granted by dismissing an alert. UI state never substitutes for saved contract state. Browser fixture rewards are illustrative, not the tuned economy.

## Layered blue GUI, edited after generation

The boss supplied two Pinterest pins as visual direction: [blue luminous status frame](https://za.pinterest.com/pin/31032684929823417/) and [separate status/skill panels](https://za.pinterest.com/pin/937522847419268736/). The search page exposed no images. These cues informed original angular cyan frames and transparent navy panes; source images are not bundled into the export. No referenced character, text, stats or rune artwork becomes our game design.

| Layer | Ingredient and ownership | Opacity rule |
|---|---|---|
| L0 Screen accents | Vignette, brief edge warning; never a combat oracle | Optional, reduced motion/effects off; quiet centre |
| L1 Backplates/frames | Generated original frame art plus code-defined panel fill | Panel alpha independent of text, meters and icons |
| L2 Meters | White masks tinted/filled from current/max HP, guard, readiness/charge | No baked values; no extra mana or XP meter |
| L3 Icons | Fire, carried objective, original guild seal | Shape and label communicate state without colour alone |
| L4 Text/values | Runtime text, counters and labels | Independent contrast backing; minimum alpha alone is insufficient |
| L5 Context | Interact key/verb and valid target context | Visible only when relevant, does not cover combat centre |
| L6 Alerts | Recovered, delivered, service restored, learned technique | Brief and bounded; replay cannot replay rewards |
| L7 Modal windows | Hall board, dressing preview, pause/access | Own background alpha; readable controls; modal focus and Cancel |

Use uGUI Screen Space Overlay for screen presentation. Candidate sprite manifests declare Sliced/Filled/Simple, borders and tintability. Runtime text stays separate from raster art. Proposed CanvasGroup settings must not multiply critical text below its contrast floor. The browser lab uses a 0.85 text floor plus an independent dark text backing; this is a prototype choice to test, not a universal contrast guarantee. Modal text has its own contrast surface.

Generation produced presentation v1; an explicit image edit removed invented weapons, compass, armour fit and unrelated contract narrative. The working atlas is a separate text-free sprite candidate. A requested 2048px grid is not evidence of the output size. Check measured pixel dimensions, alpha, every cell's actual occupancy, empty cells, bleed, white masks and nine-slice border placement before reuse. Mechanical cropping/packing must be recorded; failed candidates remain labelled.

## Art and experiment index

| Artifact | Useful for | Status / design consequence |
|---|---|---|
| trained-builds.png | Same hero's early/trained outfit mass and marker ideas | Concept only; fighter/heavy still need stronger silhouette distinction; caster coat must avoid throw clipping |
| dressing-corner.png | Owned bench/chest/rack/frame assembled as a physical creator | Recipe direction; drop illustrative rug in first assembly; mirror material/UI preview still proposed |
| playable-system-sheet.png | Hall, departure, contact, guard, throw, fire, return and services in one sheet | Art targets, not chronological first-trip order; held fire follows the first delivery/lesson. Sparks/impact are upper-bound studies to reduce during readability review |
| blue-gui-system-sheet-v2.png | Coherent combat, board, dresser, options, contact and fire states | Edited presentation; labels/fills are runtime layers, never this whole bitmap as a UI |
| Codex Blender lab | Independent owned-body morph and torso-shell feasibility | 35 manual pose/blend samples, actual failure + revision and FBX round trip; Unity gates unrun |

## Blender evidence changes the implementation plan

HumanM_BodyMesh is 10 disconnected topology islands with no coincident seam clusters. Zero gap over an empty coincident set was not a valid test. Codex's lab selects 282 nearby cross-island vertex pairs in a declared torso/joint region as diagnostics, not weld correspondences. The original 222 count discarded asymmetric nearest relationships with an index-order filter; that coverage error was corrected and the original report retained. The first spatial morph increased one candidate separation by 11.115 mm. The revision protects those interfaces with spatial attenuation and keeps the maximum added separation below 0.001 mm in the 35 sampled manual poses/blends. That preserves measured relationships; it does not repair pre-existing gaps or prove intersections absent.

The body remains 1,880 vertices on a copy of the 5,340-vertex source; 51 bones are retained. Bone endpoints do not change between blends of the same pose. Protected hand/foot/head vertices shift less than 0.001 mm (numerical noise). Maximum surface movement is about 26.741 mm. A torso-face shell (342 vertices, 557 faces) carries the same owned skin weights and morph keys; corresponding shell/body offsets remain approximately 12 mm. Corresponding offsets alone do not prove global no-clipping. Material regions and shell are technical studies, not finished clothing.

Blender FBX reimport preserves the body vertex count, 51 bones and Basis/fighter/caster/heavy key names. It has not proved Unity Avatar configuration, clip retargeting, grounding, throw alignment or fixed gameplay contact. Source SHA256 is unchanged. Derived .blend/.fbx stay local outside Assets and public git.

## Next build gates, in order

1. M0 stack compatibility on a preserved copy, asset/shader cleanup and clip table.
2. M0.5 single P0 faceted body: Unity Avatar plus the actual curated movement, 8 combat and 4 casting poses/paired throw. Measure contact and support at 30/60/120.
3. M1 gold path with primitive readable HUD; blue art is a replaceable presentation layer, never a blocker for state proof.
4. Harden combat/AI, two service deliveries, save/retry, then plain-to-held attunement in the existing milestone order.
5. P1 creator/morph envelope, derived clothes and markers only after their own clipping/contact/readability gates. Four of five independent reviewers must identify roles at actual 10 m/20 m camera framing, grayscale and effects off (H).
6. HUD at 1080p on bright/dark frames: required values glance-readable, contrast checked against actual backgrounds at minimum allowed alpha, combat centre clear, all sprite cells traceable. Browser tests and Blender samples supplement Unity acceptance; they never replace it.


---

# KIT LEXICON v2 - three separate libraries: OBJECTS | CHARACTERS | EFFECTS

Boss rule: objects, characters and effects are separate kits with separate grammars, recipes and concept specs. They meet only in a SCENE COMPOSITION (section 4), never inside one brick or one spec. Source: Logs/sheets/*.png/.txt + Documentation/catalog-clips.txt. Sizes/grid UNKNOWN until measured in M0 (H). Palette targets in section 5 apply to all three.

=====================================================================
## 1. OBJECT KIT (OBJ) - world, architecture, props, nature
=====================================================================
### 1.1 Look
- Village bricks (V-): faceted low-poly, flat-shaded, matte, earthy; timber houses on stone plinths, thatch/shingle roofs, crossed timber horns on gable peaks; chunky, slightly crooked props.
- Hall bricks (T-): clean chunky modular timber, near-black frames, mid-brown planks, rivet dots; wall infill currently GREY placeholder (material gap).
- Nature: faceted rocks/trees (V-ROCK, V-VEG) vs painterly soft foliage (A-) = the main style tension.

### 1.2 Brick IDs
HALL (T-): T-FLOOR, T-WALL, T-WIN, T-DOOR, T-POST, T-ROOF-L, T-ROOF-S, T-ROOF-CL1/CL2, T-ROOF-CS1/CS2; T-BAR, T-BENCH, T-TBL-SQ, T-TBL-RD, T-TBL-DIN, T-STOOL, T-STOOL-TALL, T-CHAIR, T-KEG, T-BARREL, T-CHEST, T-TORCH, T-CANDLE-1..4; T-GOBLET, T-TANKARD-E/F/FAT, T-TANKARD-SPILL, T-JUG, T-BOTTLE-1/2, T-PLATE.
VILLAGE (V-): V-HOUSE-01..05; V-AWN-A/B; V-FENCE-01A/01B, V-FENCE-CNR; V-GATE-SOLID, V-GATE-BROKEN, V-GATE-HALF; V-SHED-01/02; V-BOARDWALK-A/B; V-BANNER-A (red/bone quartered), V-BANNER-B (dark red); V-BENCH-1..3, V-CHAIR-A/B, V-BARREL-1/2, V-BASKET-1/2, V-TUB, V-BOWL, V-BOX, V-BUCKET, V-CRATE-1..3, V-JUG, V-PACKAGE, V-SACK-1/2, V-SACKS-SET, V-SACK-OPEN, V-TROUGH, V-VASE-1..3, V-RACK-CLOTHES, V-RACK-A, V-TRIPOD-POT, V-BROOM, V-LADDER, V-RAKE, V-POSTBOX, V-STONES, V-CART, V-WELL, V-SHIELD-1/2, V-TABLE, V-LOGS-1..3.
NATURE: V-ROCK-1..3, V-ROCK-S1/S2, V-PEBBLES; TER-GRASS-1/2, TER-PATH-A/B, TER-SAND, HILL-1/2, MTN-1, SKY-DOME, CLOUD-1/2; V-BUSH-1/2, V-FLOWER-R/Y/B, V-GRASS-1/2, V-PLANT-1/2, V-TREE-1/2, V-PINE; A-TREE-1..8, A-BUSH-1 (crimson), A-BUSH-2..4, A-GRASS-1/2.

### 1.3 Object grammar
- BAY = T-POST + infill (T-WALL | T-WIN | T-DOOR) + T-POST; bays chain, posts shared.
- ROOF RUN = T-ROOF-L per bay, T-ROOF-S on gable sides, T-ROOF-CL/CS at corners.
- THRESHOLD = 2 T-POST stretched 1.3-1.5x + T-ROOF-L lintel + banner pair.
- SERVICE EDGE = counter (T-BAR | V-TABLE) + storage (T-CHEST | V-CRATE) + vertical marker (V-RACK | T-TORCH).
- STATE PAIR = same bricks, damaged (missing/tilted pieces, piled props, no light, no cloth) vs restored (aligned, cloth, light).
- LANDMARK = one tall or bright silhouette per space, visible from the hall door.
- Allowed edits on derived copies: uniform scale 0.8-1.5x, single-axis stretch for posts/beams, piece removal/duplication, kitbash between kits, material derivatives. Never colour-only.

### 1.4 Object recipes
O1 GUILD LONG HALL: 3 x 5 bays; front gable T-DOOR centre + T-WIN each side; full roof run; THRESHOLD + V-BANNER-A pair at the front; crossed gable horns kitbashed from V-HOUSE peaks; inside: SERVICE EDGE (T-BAR + T-KEG + T-CHEST), board (free T-WALL between two T-POST with paper quads), T-BENCH rows, T-TORCH + T-CANDLE light pools, rear T-DOOR to the sparring floor.
O2 SPARRING FLOOR: TER-PATH earth, V-FENCE-01 ring, V-BOARDWALK watchers' edge with V-BENCH, V-SHIELD-1/2 trophies on the fence, V-RACK-A gear, V-POSTBOX banner post landmark.
O3 GATE PAIR: damaged = V-GATE-BROKEN in a 2 x T-POST frame, V-STONES + V-LOGS piled, V-LADDER leaning, unlit; restored = V-GATE-SOLID swung open in the same frame, T-ROOF-L lintel, V-BANNER-B, T-TORCH lit, path cleared.
O4 STALL PAIR: dormant = V-SHED-01, V-CRATE-3 stack, V-SACKS-SET, V-BROOM, no cloth; active = V-AWN-A recoloured red/bone over V-TABLE + V-BASKET-1/2 + V-JUG + V-VASE-1..3 + V-RACK-CLOTHES.
O5 EXPOSED ROAD: TER-PATH strip, broken V-FENCE-01 runs, abandoned V-CART, V-LOGS, low V-BUSH only, V-PINE far landmark; site: V-SHED-02 + V-CRATE-1 (tool crate) + V-RAKE + V-LADDER.
O6 SHELTERED PATH: A-TREE clusters, V-ROCK-1..3 cover, A-BUSH-1 crimson at the bend, A-GRASS; site: V-SACKS-SET + V-BASKET + V-TROUGH.
O7 OPPONENT POST: V-TRIPOD-POT + V-LOGS + V-SACK-OPEN + V-BARREL (effect anchor point for a smoke loop, see FX kit).

### 1.5 Object style directions (boss picks from wave 1)
SD-A CARVED FOLK: all objects read as chiselled timber/stone with matte pigment; A- foliage retuned to fewer, larger clumps. SD-B PAINTED SOFT: objects get brushed gradients derived from the A- foliage. Recommendation: SD-A, with A- foliage kept soft only at the edges of decision spaces.

### 1.6 Object concept specs (CS-OBJ)
Common: no text, logos or named styles/artists/studios/games/films; faceted geometry visible; matte; palette section 5; NO characters and NO effects in object specs (empty scenes, light only).
CS-OBJ-01 exploded-hall: O1 as an exploded assembly diagram, each brick floating slightly apart along its snap axis; three-quarter aerial 30 deg, orthographic feel; even studio light, background #9FB2C8. Acceptance: a viewer can count brick types; nothing un-buildable. Use: hall blueprint + derivation record.
CS-OBJ-02 hall-exterior (SD-A and SD-B variants): O1 + O2 + V-WELL from eye level 1.7 m, three-quarter, 35 mm feel, late-morning sun from the left; both route mouths visible; hall roof/door silhouette strongest shape; sparring ring ground quiet. Use: layout + palette + style choice.
CS-OBJ-03 hall-interior: counter service edge, keg and chest, board between two posts with blank paper sheets, benches, warm torch/candle pools #F2A65A, rear door open to daylight; from just inside the front door.
CS-OBJ-04 gate-pair: two panels, identical camera and light; damaged vs restored (O3). Acceptance: change obvious at thumbnail size in silhouette, without colour.
CS-OBJ-05 stall-pair: two panels, identical camera; dormant vs active (O4).
CS-OBJ-06 exposed-road day + dusk: O5 + O7; long sightlines by day; dusk fog that keeps a 15 m silhouette area readable (an empty marked spot, no figure).
CS-OBJ-07 sheltered-path: O6; dappled light, ambush bend, supplies under trees; path always visible.
CS-OBJ-08 texture-sources: three flat swatches (worn chiselled timber, cool rough stone, coarse wool in red and bone); even light, no perspective, aim for tileable edges.
CS-OBJ-09 ui-language: plank health bar with iron caps, stacked-plank guard segments, iron ring ember slot (empty ring, effect added in-engine), crate-outline objective slot, pinned paper contract card with wax-red seal, post-and-lintel button frames; no letters or numbers.

=====================================================================
## 2. CHARACTER KIT (CHR) - bodies, rigs, outfits, motion
=====================================================================
### 2.1 Look
Two body families: (1) segmented BLOCK mannequins in loud flat colours with weapon sets (they match the faceted object kit in form, not in colour); (2) SMOOTH humanoid dummies and mocap rigs (grey, red, blue, purple). One high-detail grey figure (C-MOCAP). No final hero.

### 2.2 Body IDs
C-MOCAP (rig_MCUnity), C-BLUE, C-RED, C-DUMMY-M, C-DUMMY-F, C-MORRO; C-BLOCK-{BRUTE, KARATE, NINJA, SORCERESS, 2H, ARCHER, KNIGHT, MAGE, CROSSBOW, HAMMER, SPEAR, SWORD, CRAFTER, BERSERKER, HEAVY}. Hand-held pieces from the block sets (swords, shield, hammer, spear, crossbow, bow, staff, pointed hat) are CHR accessories, not objects.

### 2.3 Motion IDs (clip families as character bricks; all Humanoid)
M-LOCO (8-way run/strafe with root motion, idle, jump/fall/land, roll, stun), M-STRIKE (fighting-motion jab/hook/upper, body/high/low roundhouse, side kick, low kicks; light strings L1-R3), M-REACT (head/body/low damage, head/body/low block, knockdown, knockdown loop, get-up, directional get-hit, knockback), M-THROW (paired grab/throw A/B), M-CAST (one-hand direct L/R, two-hand Load hold + Cast, omni burst, casting enter/idle/exit/damage; male and female sets), M-WEAPON (12 styles: idle/walk/run/jump/fall/land/attack; 2-hand sword set of 11 attacks), M-TOWN (idles, talk, emotions, vending, carry, pick-up, chant, drunk walk, greet/nod, crafting).

### 2.4 Character grammar
- FIGURE = BODY (one owned humanoid rig) + OUTFIT LAYERS (derived materials and kitbashed accessory meshes on derived copies) + PALETTE (section 5) + SILHOUETTE KEY (one shape that identifies the role at 20 m) + MOTION SET (curated M- families).
- ROLE = FIGURE + behaviour pattern (from the GDD) + a stance clip that defines the home pose.
- Silhouette first: two roles must be told apart in solid black at thumbnail size.
- Edits on derived copies only: material derivatives, accessory kitbash, proportion tweaks within the rig's tolerance; motion twists = retiming, curated transitions, procedural layer.
- Style link to objects: SD-A turns figures into faceted carved figures (bridging block mannequins and the faceted world); SD-B keeps smooth bodies with painted materials.

### 2.5 Character roster (recipes)
CH1 HERO / guild field member: C-DUMMY-M or C-MOCAP body (rig proof decides); practical layered work clothes (wrapped forearms, padded vest, belt + one pouch, soft boots), red/bone sash = silhouette key; motion: M-LOCO, M-STRIKE, M-REACT, M-THROW, M-CAST.
CH2 PRESSURE opponent: same body family; forward lean, light layers, wrapped fists; ochre + charcoal; silhouette key: forward-hunched shoulders + head wrap.
CH3 DEFENSIVE opponent: same body; upright broad stance, padded forearm guards; slate + moss; silhouette key: wide shoulders + raised forearm pads.
CH4 CLERK: smooth body; long apron/robe; M-TOWN chant/talk/idle; stands at the service edge.
CH5 STALL KEEPER: smooth body; rolled sleeves, cap; M-TOWN vending/greet.
CH6 REPAIR CREW: block or smooth body; tool belt; M-TOWN carry/pick-up/crafting.
CH7 TRAINING PARTNER: hero body, plain undyed outfit; stance + strike/guard set.

### 2.6 Character concept specs (CS-CHR)
Common: plain light background (transparent where possible); NO architecture and NO effects; proportions fit a standard humanoid rig (head about 1/7.5 of height); hands and feet clear; no text.
CS-CHR-01 hero-turnaround: CH1 front/side/back, neutral A-pose; SD-A faceted rendering (plus an SD-B variant if the boss has not picked yet).
CS-CHR-02 opponent-silhouettes: CH2 and CH3 in signature poses, each beside its solid-black silhouette; acceptance: told apart by silhouette alone.
CS-CHR-03 hall-cast lineup: CH4, CH5, CH6, CH7 standing in a row at equal scale with the hero for proportion.
CS-CHR-04 combat-pose-sheet: neutral figure, 8 key poses: home stance, light contact, heavy wind-up, heavy contact, guard loaded by a block, guard broken, lateral step, throw reach-in. Aesthetic targets only, never collision truth.
CS-CHR-05 casting-pose-sheet: hero in cast ready, plain release, charged hold (feet planted, braced), charged release; NO effect drawn (effects come from the FX kit and are layered in-engine).

=====================================================================
## 3. EFFECTS KIT (FX) - element and impact effects
=====================================================================
### 3.1 Look
Two sources: element sets (fire, frost, light, lightning), each with charge, missile tiny/small/normal/mega, muzzle normal/big, explosion tiny/small/normal/mega, dome, shield, pillar, sphere, beam, slash hit, orbit, glow; generic sets: sparks (by colour), dust, smoke, magic shields, portals, auras, hits (electro, explosion, stones), slashes. Known prep: missing shaders (magenta) and missing-script components on some prefabs.

### 3.2 Effect IDs
X-FIRE-{CHARGE, MISSILE-T/S/N/M, MUZZLE-N/B, EXPLODE-T/S/N/M, DOME, SHIELD, PILLAR, SPHERE, BEAM, SLASHHIT, ORBIT, GLOW}; same pattern for X-FROST-, X-LIGHT-, X-LIGHTNING- (plus X-LIGHTNING-IMPACT-BLUE/GOLD); X-SPARK-{colour}-{STEADY, EXPLODE, FLASH}; X-DUST-{PUFF, GROUND, LOOP, HEMI}; X-SMOKE-{PUFF, GROUND, LOOP, HEMI, VORTEX}; X-SHIELD-{BLUE, PINK, YELLOW}; X-PORTAL-{BLUE, GREEN, RED, YELLOW}; X-AURA-{LIGHTNING, STAR, PLEXUS}; X-HIT-{ELECTRO, EXPLOSION, STONES, ...}; X-SLASH-{CHARGE-BLUE/PURPLE/RED, ELECTRO, SNOW, STONE}.

### 3.3 Effect grammar
- EFFECT EVENT = element x stage x scale. Stages: READY -> CHARGE -> RELEASE (muzzle) -> TRAVEL (missile) -> IMPACT (explosion/hit) -> LINGER (smoke/dust, short).
- SCALE TIERS bind to gameplay strength, never to clip names: tiny = ready/contact accent, small = plain cast, normal = charged cast, mega = reserved (not in P0).
- ANCHORS: every effect attaches to a declared point: hand bone, foot bone, contact point, projectile, ground point, object anchor (e.g. O7 smoke).
- BUDGET: effects never cover the hands, face or the opponent's next tell; max one linger per impact; contact accents stay hand-sized.
- PAIRING: every effect stage has a body cue from the CHR kit first (outcomes get different bodies before different colours).
- Edits: shader repair to URP, scale, colour to palette, lifetime trims, particle count caps, behaviour written by us (pack scripts never used).

### 3.4 Effect recipes
F1 FIRE PLAIN: READY X-FIRE-GLOW tiny at the palm -> RELEASE X-FIRE-MUZZLE-N -> TRAVEL X-FIRE-MISSILE-S (short trail) -> IMPACT X-FIRE-EXPLODE-S -> LINGER X-SMOKE-PUFF short.
F2 FIRE CHARGED: CHARGE X-FIRE-CHARGE growing with hold (size and pulse together) -> RELEASE X-FIRE-MUZZLE-B -> TRAVEL X-FIRE-MISSILE-N (bounded steering) -> IMPACT X-FIRE-EXPLODE-N + push-back -> LINGER X-SMOKE-GROUND brief.
F3 CONTACT ACCENTS: hit X-SPARK-WHITE-EXPLODE tiny; block X-SPARK-YELLOW-FLASH tiny at the guard; guard break X-DUST-HEMI low; heavy contact X-DUST-GROUND pulse; footfalls/landing X-DUST-PUFF.
F4 WORLD: O7 opponent post X-SMOKE-LOOP small; restored gate torch flame (T-TORCH + small fire glow); hall candles.
F5 ATTUNEMENT LESSON: sparring-floor ring marker X-FIRE-ORBIT tiny around the hero's hand once when the charge is learned.
Reserved (not P0): frost, light, lightning sets for a later element; portals and auras only by AMEND.

### 3.5 Effect concept specs (CS-FX)
Common: dark neutral background, NO architecture and NO characters except a plain grey hand/silhouette placeholder for scale; no text.
CS-FX-01 fire-stage-strip: F1 and F2 as two horizontal strips, stage by stage (ready, charge growth in 3 sizes, muzzle, missile, impact, linger), scale tiers labelled by size only.
CS-FX-02 contact-accent-sheet: F3 accents at real size beside a grey fist and forearm guard for scale; acceptance: accent never larger than the hand.
CS-FX-03 palette-check: fire core #FFD27A and edge #E2561F against the object palette swatches, plus a desaturated version to prove the effect still reads by shape.

=====================================================================
## 4. SCENE COMPOSITION (where the three kits meet)
=====================================================================
A scene spec = one OBJ recipe + CHR roles + FX recipes, each referenced by ID, never redefined. Composition rules: character silhouette is always the highest-contrast moving shape; effects stay below the hand-size budget except a charged impact; object clutter stays out of the fight space.
CS-SCN-01 key-moment: O2 at dusk + CH1 blocking CH2 + F3 block accent + O1 threshold torch pools + O3 restored gate in the background.
CS-SCN-02 first-delivery: O1 interior + CH1 at the counter with CH4 + O4 active stall visible through the door.

=====================================================================
## 5. SHARED PALETTE (targets sampled from bricks; verify in-engine)
=====================================================================
timber dark #4A3122 | timber mid #7A5233 | timber light #B38552 | frame #2A211C | stone #8C8A85 | stone dark #55534F | thatch #9C9268 | guild madder red #8E2F25 | guild bone #E6DAC2 | grass #6E9A4A | sun foliage #C9D85A | sky #9FB2C8 | fire core #FFD27A | fire edge #E2561F | torch pool #F2A65A | opponent ochre #B07A2A / charcoal #2E2E2E | opponent slate #5B6770 / moss #5E6B3A.

=====================================================================
## 6. RENDER PLAN (Codex renders with GPT image; max 3 attempts each)
=====================================================================
Wave 1 OBJECTS (style lock): CS-OBJ-01, CS-OBJ-02 (SD-A + SD-B), CS-OBJ-04, CS-OBJ-05 -> boss picks SD-A or SD-B.
Wave 2 OBJECTS + FX: CS-OBJ-03, 06, 07, 08, 09; CS-FX-01, 02, 03.
Wave 3 CHARACTERS (after the M0.5 rig proof picks the body): CS-CHR-01..05.
Wave 4 SCENES: CS-SCN-01, 02 (only after the three kits each have approved images).
Files: Documentation/concepts/{objects,characters,effects,scenes}/ (created at render time); each logged with id, attempt, outcome, approval_state.
