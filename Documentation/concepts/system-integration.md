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
