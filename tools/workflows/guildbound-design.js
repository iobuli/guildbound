export const meta = {
  name: 'guildbound-design',
  description: 'Diagnose remaining movement/get-up defects and design hand-to-hand combat + lock-on for Guildbound',
  phases: [
    { title: 'Diagnose', detail: 'one agent per defect area, measures in its own worktree' },
    { title: 'Combat designs', detail: 'three independent designs from different angles' },
    { title: 'Judge', detail: 'three judges score the designs' },
    { title: 'Synthesize', detail: 'merge winner + best ideas into one build spec' },
    { title: 'Critic', detail: 'check the whole plan against the user requirements' },
  ],
}

// Context lives in docs/HANDOFF.md (read it first). $GODOT = the Godot 4.7.2 binary (see HANDOFF for Linux setup).
const CTX = `
Read docs/HANDOFF.md in the repo FIRST: it holds the user's non-negotiable direction, how to run Godot headless
($GODOT --headless --path . --import, then -- --selftest [--only=...]), the architecture of every file in game/, the
combat controls already specified in docs/CONTROLS_AND_PHYSICS.md, the free animation clips available, and the current
measured status (89/100 checks pass; all failures are the new get-up checks). If $GODOT is unset, set it up as HANDOFF says.
`

const DIAG = {
  type: 'object',
  properties: {
    area: { type: 'string' },
    findings: { type: 'array', items: { type: 'object', properties: {
      issue: { type: 'string' }, root_cause: { type: 'string' }, evidence: { type: 'string' } },
      required: ['issue', 'root_cause', 'evidence'] } },
    plan: { type: 'array', items: { type: 'object', properties: {
      file: { type: 'string' }, change: { type: 'string' } }, required: ['file', 'change'] } },
    tests: { type: 'array', items: { type: 'object', properties: {
      name: { type: 'string' }, metric: { type: 'string' }, pass_threshold: { type: 'string' } },
      required: ['name', 'metric', 'pass_threshold'] } },
    risks: { type: 'array', items: { type: 'string' } },
    patch_summary: { type: 'string', description: 'what you prototyped in your worktree and the measured before/after numbers; else empty' },
  },
  required: ['area', 'findings', 'plan', 'tests', 'risks', 'patch_summary'],
}

const AREAS = [
  { key: 'getup', title: 'Get-up universality and snaps',
    task: `Make the get-up pass all 11 landings reliably (run --only=getup at least 3 times; results vary run to run). Diagnose the phase-one untangle snaps (instrument per-frame body velocity, contacts via contact_monitor/get_colliding_bodies, targets vs bodies), the stairs shin-on-step-edge problem, the cliff-wall case (contacts/stand spot inside the wall; choose a heading/plan with clearance, e.g. candidate headings checked with shape queries), and late foot-landing snaps. Also fix the --getupshots camera for the wall case. Consider an explicit untangle stage (free trapped limbs one at a time, order chosen from contacts), collision-aware limb paths (shape-cast the planned limb capsules against the world), and relaxing the pop metric ONLY where you can prove a spike is a legitimate physical impact.` },
  { key: 'climb_air', title: 'Climbing legs and airborne pose',
    task: `Climbing: legs hang (feet never press the wall). Prototype foot contacts on the wall (raycast footholds below/beside the hips, two-bone leg IK, knees bent away from the wall, feet alternating with the hand reach cycle, ball of the foot as the contact, no clipping into the wall; stamina/motion unchanged). Falling: replace the stiff wide frog pose with a physically plausible procedural fall pose (arms reacting to vertical speed and drag, legs preparing to land, smooth jump -> fall -> landing blend). Verify with --only=climbanim,cliff,fall and report numbers (feet within N cm of the wall while climbing, nothing penetrates the wall).` },
  { key: 'stairs_feet', title: 'Stairs stepping and foot locking',
    task: `Implement the user's stairs model: the swing leg lifts and plants ON TOP of the next step; if the player stops, that foot stays on the step (weight between the two levels, pelvis height solved from the planted feet); walking back returns it to the floor. The visual body height must come from the planted feet, not be fitted after a capsule teleport. Also cut foot sliding: slow walk, crouch walk, push, stairs down, turn in place (targets: planted foot < 0.15 m/s in stance; turn in place < 0.3 m over 4 turns). Feet pivot only at heel/ball. Verify with --only=slopes,stairsanim,slide,run and report before/after.` },
  { key: 'pops', title: 'Locomotion pops',
    task: `Find the cause of each large locomotion pop (dodge end, push start, jump onto wall, swim enter/back/exit, carry drop / after throw, run->sprint, run->stop) by instrumenting per-frame part positions/targets around the event (--only=pops prints the worst part and state). Suspects: AnimationTree transition / one-shot fade snapping, MotionIK weight switches (feet_w/push_w/climb_w), BodyRig SNAP_DISTANCE teleports, inst.position swim lift / climb offset, targets updating once per drawn frame while physics steps twice (BodyRig.extrapolate is only on during get-ups; test enabling it always once overshoot is handled). Report before/after per pop line and propose pass/fail thresholds that real fixes meet.` },
]

const LENSES = [
  { key: 'physical', brief: `PHYSICAL-FIRST: strikes are real collisions between the attacker's hand/forearm/foot rigid bodies and the target's BodyRig parts, impulse from relative velocity and mass; strike motion is maths-driven (IK fist trajectory to a target point on the opponent, hip/torso rotation, weight transfer; Punch_Jab/Punch_Cross/Melee_Hook only as a style layer); reactions come from the target's active ragdoll (strength dip, impulse on the struck part, stagger steps through the foot planner, knockdown -> ragdoll -> GetUpPose). Fists stop on the body surface: never clip.` },
  { key: 'feel', brief: `FEEL-FIRST: responsiveness and readability. 120 ms input buffer, cancel windows, 3-hit light combo + charged attack on hold, RMB guard and 150 ms parry, directional dodge with i-frames and perfect-dodge Flurry, 60-90 ms hit-stop, camera kick, lock-on UX (MMB toggle, wheel cycles, soft target when unlocked, camera framing both fighters, strafe-circle movement, Space side-hop while locked), hit/guard/parry feedback, target health on the HUD.` },
  { key: 'architecture', brief: `ARCHITECTURE-FIRST: multiplayer-ready structure. The Player reads an intent/input source (local input, AI brain, later network) instead of Input directly, so NPC fighters reuse the exact same body/movement/ragdoll/get-up/combat code; a Combat component (moves as data in data/moves.json; damage through the stat engine: attack_damage, attack_speed scaling recovery, crit, armor via take_physical, lifesteal, tenacity reducing knockback/stagger); a LockOn component; a sparring partner AI (approach, keep range, attack, guard, dodge, recover) and a passive training dummy in the arena; deterministic self-test checks for every rule.` },
]

const runDiagnose = () => pipeline(AREAS,
  a => agent(`${CTX}\n\nYOUR AREA: ${a.title}\n${a.task}\n\nYou are in your own git worktree: instrument and prototype freely (nothing reaches the main branch unless adopted). Measure before you conclude. Return a precise diagnosis and an implementation plan another engineer can apply (exact files, functions, algorithms, constants) plus pass/fail tests with thresholds.`,
    { label: `diagnose:${a.key}`, phase: 'Diagnose', schema: DIAG, isolation: 'worktree' }))

const runCombat = async () => {
  const designs = await parallel(LENSES.map(l => () => agent(`${CTX}\n\nTASK: design HAND-TO-HAND MELEE COMBAT plus a LOCK-ON FIGHTING SYSTEM (unarmed now, must extend to weapons later). Read player.gd, character_visual.gd, motion_ik.gd, body_rig.gd, get_up_pose.gd, camera_rig.gd, char_model.gd, stats.gd, input_setup.gd, hud.gd, arena.gd, selftest.gd and docs/CONTROLS_AND_PHYSICS.md sections 2.2 and 4, docs/MAGIC_AND_STATS.md.\nYOUR ANGLE: ${l.brief}\nProduce a COMPLETE design anyway: controls; fighter state machine and integration with the Player states (carry/climb/swim exclusions, stamina costs); move list with startup/active/recovery scaled by attack_speed; how strikes are animated (procedural + which clips); contact-based hit detection with no clipping; damage through the stat engine; reactions (flinch, stagger, guard break, knockdown -> ragdoll -> get-up), guard/parry/dodge; lock-on (acquire, switch, lose, camera framing, strafe, side-hop); training dummy + sparring AI reusing the same character pipeline; HUD; file-by-file plan in an order that keeps the game running; headless self-test checks with thresholds. Be concrete. Do not edit files.`,
    { label: `design:${l.key}`, phase: 'Combat designs' })))
  const ds = designs.filter(Boolean)
  const JUDGE = { type: 'object', properties: {
    scores: { type: 'array', items: { type: 'object', properties: {
      design: { type: 'integer' }, total: { type: 'number' }, strengths: { type: 'string' }, weaknesses: { type: 'string' } },
      required: ['design', 'total', 'strengths', 'weaknesses'] } },
    best: { type: 'integer' }, graft: { type: 'array', items: { type: 'string' } } },
    required: ['scores', 'best', 'graft'] }
  const listing = ds.map((d, i) => `=== DESIGN ${i} ===\n${d}`).join('\n\n')
  const judges = await parallel([
    'fit to the user direction (solid physical body, no clipping, maths-driven motion, joints as pivots) and to the existing code',
    'implementability and risk in this Godot codebase (built and verified headlessly without breaking passing checks)',
    'player experience: responsiveness, readability, depth, lock-on quality',
  ].map((lens, j) => () => agent(`${CTX}\n\nJudge these combat designs through the lens: ${lens}. Score each 0-100, pick the best, and list specific ideas from the others worth grafting onto it.\n\n${listing}`,
    { label: `judge:${j}`, phase: 'Judge', schema: JUDGE })))
  const js = judges.filter(Boolean)
  const tally = ds.map((_, i) => js.reduce((s, j) => s + ((j.scores.find(x => x.design === i) || {}).total || 0), 0))
  const win = tally.indexOf(Math.max(...tally))
  log(`combat design scores: ${tally.map(t => Math.round(t)).join(' / ')} -> design ${win}`)
  const spec = await agent(`${CTX}\n\nWrite the FINAL build spec for hand-to-hand melee combat + lock-on, starting from DESIGN ${win} and grafting the judges' recommended ideas. Engineers will implement it in parallel with the movement fixes, so: (1) every file touched and exactly what changes, (2) precise integration points with player.gd / character_visual.gd / motion_ik.gd / body_rig.gd / camera_rig.gd / hud.gd / input_setup.gd / arena.gd / selftest.gd so merges are predictable, (3) move data and formulas, (4) AI opponent and training dummy, (5) self-test checks with thresholds, (6) milestones that each leave the game runnable.\n\n${listing}\n\n=== JUDGES ===\n${JSON.stringify(js, null, 1)}`,
    { label: 'synthesize:combat', phase: 'Synthesize' })
  return { spec, tally, win }
}

const [fixes, combat] = await parallel([() => runDiagnose(), () => runCombat()])
const fx = (fixes || []).filter(Boolean)
const critic = await agent(`${CTX}\n\nYou are the completeness critic. Below are diagnoses + plans for the remaining movement/get-up defects and the combat + lock-on build spec. The user asked for an ALL-OR-NOTHING complete fix plus the new combat system. List: requirements from the user's direction not covered; contradictions between plans (especially shared files: player.gd, motion_ik.gd, character_visual.gd, body_rig.gd, get_up_pose.gd, selftest.gd); missing tests; and a recommended integration order. Be specific.\n\n=== FIX PLANS ===\n${JSON.stringify(fx, null, 1)}\n\n=== COMBAT SPEC ===\n${combat ? combat.spec : '(missing)'}`,
  { label: 'critic', phase: 'Critic' })
return { fixes: fx, combat, critic }
