# Voxel brainstorm - scored against team constraints

Scoring: each idea checked against C1-C6 below. Y = fits, N = violates, ~ = fits with care. Citations per hive rule.

## Shared voxel store [team]
- C1 ONE sparse chunked voxel store in the pure sim crate, keyed by integer coords (IVec3 chunk, local u8 cell) [team].
- C2 ONLY writer: event_log `VoxelEdit{chunk, cells, tick}` [team].
- C3 Colliders, LOD, meshes, mass all derived downstream on whole ticks [team].
- C4 No ML in the running game; CPU fixed-tick deterministic, event-sourced [team].
- C5 No GPL code [team].
- C6 Physics needs: greedy-merged box colliders per chunk, destruction as logged events, broken-piece mass/inertia from voxel count+layout, per-voxel-type friction. Animation needs: fast height+slope lookup, voxel hits emit the same combat hit event, clearance query before acrobatics, motion ML offline only [team].
- Layout proposal: chunk -> brick bitmask -> voxel material id, coarse levels derived by OR/majority on whole ticks [XCube s3.1 p3, Fig.3] [team].

## Ideas

| # | Idea | Source | Owner | C1 | C2 | C3 | C4 | C5 | C6 | Notes |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 3-level sparse hierarchy store with child bitmasks | [XCube s3.1 p3, Fig.3] | world/voxel | Y | Y | Y | Y | Y | Y | core of store |
| 2 | Coarse LOD derived by subdivide/prune rules from finest level | [XCube s3.1 p3, Fig.3] | rendering | Y | Y | Y | Y | Y | Y | derived only |
| 3 | Per-voxel material id -> friction table, mass density | [XCube p1, Fig.1] [team] | world/voxel | Y | Y | Y | Y | Y | Y | physics friction |
| 4 | Greedy box merge per chunk for Avian colliders | [team] | world/voxel | Y | Y | Y | Y | Y | Y | few contacts |
| 5 | Piece mass/inertia = sum over voxels (count, centroid, second moments) | [team] | world/voxel | Y | Y | Y | Y | Y | Y | integer sums, exact |
| 6 | Deterministic graph-cut to split broken region, logged as VoxelEdit | [Vox-E s3.3 p5, Eq.5] | world/voxel | Y | Y | ~ | ~ | Y | Y | fixed solver order needed |
| 7 | Masked merge of two grids as edit op | [Vox-E s3.3 p5, Eq.4; Fig.3 p4] | world/voxel | Y | Y | Y | Y | Y | Y | |
| 8 | Correlation diff score between chunk states (tooling/tests) | [Vox-E s3.2 p4, Eq.3] | tooling | Y | Y | Y | Y | Y | - | replay diff check |
| 9 | Per-column height+slope cache derived per tick | [team] | world/voxel | Y | Y | Y | Y | Y | Y | foot placement |
| 10 | Clearance query (AABB occupancy test on hierarchy) | [team] (hierarchy from [XCube s3.1 p3, Fig.3]; the early-out query itself is not in the paper) | world/voxel | Y | Y | Y | Y | Y | Y | coarse level early-out |
| 11 | Voxel hit -> same combat hit event | [team] | world/voxel | Y | Y | Y | Y | Y | Y | |
| 12 | Render-only PCA-SDF smooth surfaces | [MDR s3.3 p6] | rendering | Y | Y | Y | ~ | Y | - | bases baked offline |
| 13 | Foliage sway via modal oscillators | [DynamicTree s4 p5, Eq.6] | rendering | Y | Y | Y | Y | Y | - | cosmetic only |
| 14 | Shell-map skinning (no cages; cage method only cited as inspiration §3.1 p5) of voxel characters | [Gigavoxels-TER §2 p3-4; Fig.4 p5; §3.1 p5; §4.4 p10] | rendering | Y | Y | Y | Y | Y | - | render only |
| 15 | Visibility-based brick streaming | [Gigavoxels-TER s3.3 p5] | rendering | Y | Y | Y | Y | Y | - | |
| 16 | Offline coarse->detail generator for assets | [XCube s4.1 p6, Fig.6] | tooling | Y | Y | Y | Y | ? | - | code licence unknown |
| 17 | Offline text-guided voxel edit tool | [Vox-E s3.2 p4] | tooling | Y | Y | Y | Y | ? | - | licence unknown |
| 18 | Image -> coarse voxel block-out importer | [MDR abstract p1] | tooling | Y | Y | Y | Y | ? | - | licence unknown |
| 19 | Runtime neural generation / SDS in game | [Vox-E s3.2 p4; XCube s3.2 p3-4] | - | - | - | - | N | ? | - | rejected |

## Physics rows

| # | Idea | Source | Owner | Notes |
|---|---|---|---|---|
| P1 | Collider rebuild only for chunks touched by a VoxelEdit, applied on the next whole tick before PhysicsSystems::First | [team] | Physics + world/voxel | keeps replay exact, no mid-step collider swaps |
| P2 | Box-merge must leave no coplanar seams under feet (merge across chunk edges or use one ground box per column run) | [team] | Physics | seams cause the contact flips we already fight in standing |
| P3 | Piece inertia from voxel second moments (idea 5) uses the same functions as limb mass in phys_mass | [team] | Physics | one inertia path for limbs and debris |
| P4 | Friction per material feeds slip-under-load: slip when tangential load exceeds mu x normal at the sole | [team] (material id from [XCube p1, Fig.1]) | Physics | contact c drops toward 0 while slipping |
| P5 | Small broken pieces (under a mass threshold) sleep or despawn after N ticks, logged | [team] | Physics | caps contact count and cost |
| P6 | Ground height from idea 9 also used for capture point and sole rest offset on uneven ground | [team] | Physics + Animation | balance and contact use the same height as foot placement |

## Animation rows

| # | Idea | Source | Owner | Notes |
|---|---|---|---|---|
| A1 | Foot placement reads the idea-9 height/slope cache at whole ticks; foot IK target = column height + sole rest offset, ankle pitch/roll from slope | [team] | Animation + Physics | same ground as P6, so the ghost trail and contact agree |
| A2 | Stride and step height adapt to a step or ledge ahead: sample the cache at the predicted plant point (stride_cadence gives the distance) | [team] | Animation | stops toe-stubs on voxel edges |
| A3 | Acro gate: backflip / handspring only start if the coarse-level clearance query (idea 4) finds its swept volume empty; else fall back to a short hop | [team] (coarse-first from [XCube s3.1 p3]) | Animation | swept volume taken from the acro clip's pelvis path + 0.5 m body radius |
| A4 | Strike-vs-voxel hit emits the same combat hit event, and the follow-through / recoil layer plays identically; the VoxelEdit is a separate logged event on the same tick | [team] | Animation + hub | one hit path for bodies and world |
| A5 | Hit-response sway on struck props/foliage as a damped modal oscillator kicked by the strike impulse | [DynamicTree s4 p5, Eq.6] | Animation (render-only) | cosmetic, fixed-tick deterministic, never feeds the sim |
| A6 | Voxel-skinned characters render by deforming a shell mesh skinned by the capsule bones' final pose | [Gigavoxels-TER s2 p3-4; Fig.4 p5; s4.1 p7] | Animation + Grok Bot (render) | sim keeps capsules; skin is visual only |
| A7 | Motion ML (pose/clip generation) offline only, to author clips; never at runtime | [team], per idea 19 | Animation | licence unknown for all five papers |

## Physics read-through (DynamicTree §4, XCube §3)

- DynamicTree §4, Eq.5–7: the mesh is a mass-spring-damper, projected into modal space as independent 1-DOF oscillators m·q'' + c·q' + k·q = f (Eq.6), solved with explicit Euler, and rebuilt as D(t) = Σ φk·qk(t) (Eq.7). Its mode shapes φ come from the ML-generated spectrum, which is offline only for us.
- Physics change for us: don't use explicit Euler. It gains energy and blows up once ω·dt gets large. Use semi-implicit (symplectic) Euler, which is stable for ω·dt < 2 (at 120 Hz, that's any mode under about 38 Hz), or the exact closed-form damped step, which is unconditionally stable. Either way: fixed 1/120 s, fixed mode order, impulses from hit events only, so sway is deterministic and replayable. Render-only; it never feeds back into Avian.
- XCube §3 (Fig.3) only covers the hierarchy itself: subdividing coarse voxels into octants and pruning. It has nothing on collision, so idea 4 (greedy box merge) and P1–P2 stay [team], not cited to XCube. What XCube's coarse-to-fine levels do give physics is a coarse level that can serve as a cheap broadphase or early-out for clearance and debris queries.

## Relevance to the current blocker

Blocker: ragdoll standing fails (trunk collapse com y 1.03 -> 0.15 m, ~3 m slide, ~11.6 contact flips/s). Verified against all five PDFs:
- **None of the five papers addresses articulated-body balance, joint torque control, gravity compensation, foot contact or friction.** XCube, Vox-E and MDR are generative/perception ML on voxel grids; GigaVoxels-TER is rendering-side shell deformation; DynamicTree's only dynamics are linear modal oscillators for tree sway (M d'' + C d' + K d = f, per-mode m_i q_i'' + c_i q_i' + k_i q_i = f_i, explicit Euler) [DynamicTree s4 p5, Eq.5-6].
- The closest thing is DynamicTree Eq.6: a damped oscillator is the same form as a joint PD, but the paper gives no stability analysis, no contact, and its own limitation is that modal analysis is a global linear approximation [DynamicTree s7.1 p8]. It does not help with an inverted-pendulum trunk, an unpaired torque, or contact chatter.
- Verdict: **no** - the fixes for the blocker (gravity feed-forward, balance split inside one clamp, contact filter/hysteresis, the Sigma-tau leak check) come from the team's own physics [team], not from these papers. Do not cite them for it.

