# M1 Greybox gold path - design (Claude, 2026-10-11)

Status: PAPER DESIGN. GDD S12: "Do not start M1 until M0.5 passes." M0.5 (one hero rig on the sparring floor doing every verb against a partner, layer gates at 30/60/120 fps) has not passed; Sparring.unity is not built yet. Nothing here is built. Numbers marked (H) are hypotheses. Owners are proposals; Aura assigns.

## 1. Gate (GDD S11, verbatim intent)

- Gold path steps 1-6 pass in a greybox: wake on the bench; accept A or B at the board/clerk; sparring prompts + practice partner; leave by the door and defeat or avoid the route opponent with zero hits without contact; recover the objective (second pick-up impossible); return and deliver (payment + service + new option in one event, the change visible from the hall door).
- Step 9 as an isolated defeat/retry fixture with seeded completed-delivery flags and primitive services (it does not claim the M4 attunement/second-trip flow).
- Save snapshot + reload tested.
- Evidence: fixture numbers AND captures (GDD S11 verification). Zero console errors, zero missing references.

## 2. What exists vs what M1 needs

| Need | Exists (commit) | Gap |
|---|---|---|
| Contract/attempt state, wallet, trust, services | GuildState (e4454d3), gold-path fixture steps 2-9 (25b6a5a) | completion lost by a save during a practice replay (bug, fix staged, see 6) |
| Save snapshot + previous file | SaveStore (e4454d3) | not called by live code; inventory is write-only (AMEND-INV-01 item 4) |
| Motor, contact, throw, cast | CombatWorld (cd8035f) | the world is a hard-coded 12 x 12 box at the origin (Tuning.ArenaHalfSize = 6): movement clamp, throw-landing legality and projectile world-stop all assume it. Routes have walls, corners and other positions. |
| Opponent patterns | OpponentBrain Pressure/Defensive | no post, no watch radius (12 m H), no leash (20 m H), so a route cannot be avoided |
| Driver | SparringDriver: one fight, fighter ids 0/1 hard-coded | one level-wide world with player + N opponents; id -> view map |
| Interaction (E) | IHudModel.Prompt/PromptKey exist | no interactable rule, no pickup/deliver/board triggers in the world |
| Sparring prompts (step 3) | none | four skippable, replayable prompts: light phrase, guard, step, ready cast |
| Services visible in the world | GuildState.Services | no gate pair / stall pair; no collider change for the shortcut |
| Gold-path driver | EditMode logic fixture only | PlayMode driver with captures + results.json |

## 3. Greybox layout (H; Aura owns placement, these are constraints with starting numbers)

Top-down, +z away from the bench. Grid = the 4 x 4 m floor tile Aura measured (corner pivot).

```
 z=+30  ======== DOOR (courtyard threshold, 4 m wide) ========  -> fork: exposed road (left), sheltered path (right)
        |  GATE PAIR in left wall (-10, 0, 20)          PREP STALL (+10, 0, 16) |
        |              SPARRING FLOOR 12 x 12, centre (0, 0, 19)                 |
 z=+8   ======== open hall front: supports + roof edge, no wall =================
        |  BOARD (-7, 0, 2)                                                     |
        |  CLERK counter (-7, 0, -2)                                            |
 z=-8   |                     BENCH (0, 0, -6), faces +z                        |
        x=-10                                                              x=+10
```

Constraints (each is checkable):
- Step 1: from the bench camera pose, board, clerk, sparring floor and door are all inside the view frustum (GeometryUtility.TestPlanesAABB on their bounds) within 3 s.
- Step 6: from the door camera pose, the gate pair and the prep stall are inside the frustum (the service change is "visible from the hall door").
- Routes: each about 150 m (H) of walkable path from the door to its recovery site.
  - Exposed timber road: 8 m wide (H), long sightlines, one fight space at least 12 x 12 m around the opponent's post (the same size the throw rules were tested in). A bypass stays more than 12 m (watch radius) from the post somewhere, so the route can be passed without fighting.
  - Sheltered path: 3-4 m wide (H), at least three corners, cover blocks that break line of sight. The opponent's post is behind a corner.
- Gate pair (c1 service): broken gate = blocked frame in the courtyard's left wall; repaired = open passage, a return from recovery site A of about 50 m (H) instead of 150 m. The shortcut must be a real collider change, not decoration (GDD design rule 1).
- Prep stall pair (c2 service): dormant stall (no canopy, no keeper) vs active stall; E at the active stall grants the preparation item up to the cap (2, H).
- Safe route: both routes are traversable at zero wallet with no item (GDD S5).
- Ground: flat (y = 0) for M1. Slopes, steps and the traversal jump are M2 grounding gates, not M1.

## 4. Work items (proposed owner; every rules item lands with fixtures)

W1. World geometry for the motor [Claude]. New interface in Guildbound.Combat (pure C#):
- `Vector3 Slide(Vector3 from, Vector3 to, float radius)`: move with wall sliding.
- `bool Blocked(Vector3 from, Vector3 to, float radius, out Vector3 hit)`: projectile path, charged-cast area occlusion, throw landing path.
- `bool Standable(Vector3 at, float radius)`: throw landing legality.
Default implementation BoxArena(halfSize, centre) reproduces today's behaviour exactly, so the existing 36 fixtures prove no regression. Play layer: PhysicsGeometry over Unity colliders on one "World" layer (SphereCast / CheckSphere). Fixtures: wall slide and corner; throw at a wall = visible failed attempt (matrix item "throw whiff at a wall"); projectile stops at a wall before a fighter behind it; identical outcomes at 30/60/120 fps with walls.

W2. Level driver [Claude]. GuildDriver owns GuildState, InventoryModel, SaveStore and ONE CombatWorld for the level (player + every route opponent), with an id -> FighterView map. SparringDriver stays the sparring scene's driver. The player-input read (SparringDriver.ReadPlayer) moves to one shared helper used by both; no copy.

W3. Opponent posts [Claude]. OpponentBrain gains Post, WatchRadius 12 m (H), LeashRadius 20 m (H), optional line of sight through W1. States: hold post -> engage -> leash back. Fixtures: a path that stays outside the watch radius never engages; pulling past the leash returns the opponent to its post; retry never makes the opponent faster or stronger.

W4. Interaction [Claude rule, Aura prompt art]. Nearest interactable within 1.5 m (H) and within 60 degrees of facing (H); E; refused while an action is running or in combat (reason string, like the inventory). Targets: board (opens the L7 board, RequestAcceptContract exists), clerk (Deliver), objective prop (PickUp), active prep stall (GrantPreparation). Prompt through IHudModel.Prompt/PromptKey.

W5. Hall saves [Claude]. Write at accept, delivery, abandon, attunement completion and settings changes; never during combat; load always resumes on the bench with no active combat (GDD S7). Closes AMEND-INV-01 item 4: InventoryModel exposes owned garment ids + worn id and restores from the snapshot; round-trip fixture (inventory -> snapshot -> file -> new InventoryModel -> same 12 slots).

W6. Defeat and abandon [Claude]. Player defeated -> GuildState.ResetAttempt -> bench at full HP, opponents back at their posts at full HP, objective prop restored at its site, save (abandon). Step 9 fixture in the M1 scene with seeded delivered flags.

W7. Services in the world [Aura scene, Claude rule]. A ServiceView shows the broken or restored child assembly for a service id and toggles its colliders; because PhysicsGeometry reads live colliders, the shortcut opens physically. Fixture: measured path length from site A to the door drops after c1 is delivered.

W8. Sparring prompts (step 3) [Claude rule, Aura HUD]. Pure C# sequence: light phrase, guard, step, ready cast; each skippable and replayable; PASS when each verb has been performed once (read from CombatWorld.Started / Contacts, never from input alone).

W9. Gold-path driver [Claude]. PlayMode driver that plays steps 1-6 with scripted intents (not the keyboard), captures the Game View at each pass condition and writes results.json (HP/coin/flag deltas, positions, frame time, console error count).

## 5. Order

M0.5 passes -> W1 -> W2 + W3 + W4 -> W5 + W6 -> greybox scene (Aura) -> W7 + W8 -> W9 -> M1 gate run.
W1, W3, W4, W5, W6 and W8 are pure rules with EditMode fixtures and never touch the rig. They could be built before M0.5 passes, but GDD S12 forbids starting M1 before M0.5, so that needs the boss's OK (question Q1).

## 6. Rules bug found while designing (fix staged, applies on Aura's go)

GuildState.Accept on a delivered contract (free practice replay) sets its attempt to Accepted. Two readers treat only AttemptState.Delivered as "completed": GuildState.ToSnapshot (the delivered list) and PlayInventoryContext.Delivered (keepsakes). The hall saves right after accept (GDD S7), so a save taken during a practice replay reloads that contract as Available (not completed), and its keepsake disappears from the inventory while the replay runs. Payment stays safe because Paid is saved separately. Fix: both read Paid (completed = paid once). Fixture: SaveDuringPracticeReplay_KeepsTheContractCompleted (fails before the fix with Attempts["c1"] == Available).

## 7. Test matrix additions (each with its own expected result)

| Scenario | Expected |
|---|---|
| Walk into a wall at run speed | slides along it; never inside it |
| Throw with a wall behind the target | visible failed attempt; nobody moves through the wall |
| Plain cast at an opponent behind a wall | projectile stops at the wall; 0 damage |
| Pass the exposed route on the bypass | opponent never engages |
| Pull the opponent past 20 m | it returns to its post |
| Defeat on a route | bench, full HP; rewards, services, trust kept; objective back at its site; zero coin needed |
| Save requested in combat | refused; no file written |
| Save during a practice replay, reload | contract still completed; replay pays nothing |
| Interact while striking | refused with a reason |
| Deliver c1 | gate shortcut collider opens; path length site A -> door drops |
| Reload mid-attempt (exists) | hall, attempt reset, objective available |

## 8. Questions (boss or Aura)

Q1. Build the rules-only items (W1, W3-W6, W8) before M0.5 passes, or hold all of M1 until it does (GDD S12)?
Q2. Opponent watch: radius only, or radius + line of sight? The sheltered path ("cover, ambush corners, teaches observation") only works with line of sight.
Q3. When an opponent leashes back to its post: keep its lost HP, or restore it? (GDD: retrying never makes an opponent secretly stronger; it is silent on leashing.)
Q4. The prep item at the active stall: free up to the cap, or priced? The GDD lists "purchase" as a save event but sets no price, and nothing may be required to finish.
Q5. The PlayMode gold-path driver (W9) needs a test assembly that can see the Play layer. Play and UI live in Assembly-CSharp, which an asmdef cannot reference. Options: (a) add Guildbound.UI and Guildbound.Play asmdefs (touches Aura's UI folder: asmdef files only, plus an editor asmdef for HudBuilder); (b) drive W9 from a scene MonoBehaviour started through the Aura play-mode tool, with results.json as the only output. I recommend (a).
