# Combat motion test architecture

The test build treats motion as one contact-driven pipeline:

1. The complete `fighter.glb` library is indexed (193 clips) and classified as locomotion,
   air/landing, evasion, combat, or other.
2. Every clip receives a six-group analogue containing duration, RMS motion, peak excursion,
   channel coverage, and safe retarget weights. Its restrained rest-relative rotation pose can
   be selected at runtime. Root translation is omitted so the controller remains responsible
   for collision and momentum.
3. The Ranger's measured bind skeleton adapts reach and contact geometry. Each foot uses a
   15-point heel/midfoot/forefoot patch; each hand records palm and backhand contact offsets.
4. Procedural residuals align the pose to terrain, target landmarks, support, and current
   momentum. The residual is deliberately bounded so the model keeps its identity and the
   source action remains readable.
5. Contact events transfer impulse and drive action state. The same punch contact defines the
   active frame, fist/chin surface test, target reaction, balance consequence, and buffered
   route to the next action.

## Paper mapping

- `D:\2412.02317v1.pdf` (HumanRig): anatomy priors are refined by actual target dimensions.
  The runtime application is bind-pose identity, measured limb reach, and explicit contact
  landmarks. Its trained skeleton/skin network is unnecessary for an already rigged Ranger.
- `D:\2602.19089v1.pdf` (ANI3DHuman): rigid structure is the identity base and a residual
  supplies non-rigid correction. Here the animation reconstruction is the base and terrain,
  IK, momentum, and contacts form the residual. The paper's offline diffusion/Gaussian
  training is not placed in the game loop.
- `D:\116_Memoria_TFG.pdf` (Overcoming Adversity): combat actions expose anticipation,
  active, and recovery windows plus range, movement, targeting, steering, feedback, links,
  and buffered cancels. The current punch uses those explicit phases and preserves a short
  buffered follow-up during recovery.
- `D:\2508.10898v1.pdf` and `D:\2411.18197v3.pdf`: the existing hierarchical rig,
  topology distance, temporal fitting, and identity-preserving pose layer remain the final
  stabilizer.

## Runtime budget

The library parses once at startup. Runtime work samples one selected prototype and only its
mapped rotation tracks. Foot terrain work is local: at most 15 height/normal samples per foot,
with swing prediction and no global terrain refinement. The existing 120 Hz controller and
12 Avian substeps remain unchanged. This is intended for the installed RTX 3060 without any
runtime neural model, diffusion pass, or Gaussian training.

## New tuning pages

- **AIM / CAMERA:** launch speed, carried momentum, mouse sensitivity, camera response, and
  motion lead.
- **DENSE CONTACT:** sole clearance, sample density, compliance, foothold lookahead, and
  contact preservation.
- **LIBRARY / RESIDUAL:** reconstruction blend, residual freedom, active-limb freedom,
  landmark strike bias, and body-dimension adaptation.
- **UPPER BODY WEIGHT:** chest translation, support-foot lean, acceleration lean, arm
  counterbalance, and response speed. The torso target follows `COP + h*a/g`, then shifts and
  rotates the chest so weight transfer reads through the whole body instead of the pelvis alone.

Hold Space or the gamepad south button to charge. Mouse or right-stick camera direction sets
the launch direction; the yellow ballistic arc previews the result. The cyan/orange sole rays
show the live dense contact patch, with greener points indicating a more coherent surface.

## Live model and motion reference tools

F6/F7 or the on-screen model buttons switch among the four NPC bodies. Loading a body samples
its height, shoulder and hip width, upper/lower-limb reach, hand landmarks, sole dimensions,
toe direction, and bind rotations. Those measurements rebuild the controller capsule, stride,
IK reach, contact patches, and combat landmarks, and are written to `profiles/rig_*.json`.

The bottom-left menu exposes all 193 fighter clips as a single selectable motion reference.
PageUp/PageDown or the menu arrows change it. The source slider continuously interpolates the
model's native animation (**raw base**, 0%) and the selected rest-relative fighter clip
(**raw reference**, 100%) at the same normalized controller phase. That mixed source then passes
through terrain contacts, body-dimension fitting, balance, combat landmarks, fingers, and the
temporal stabilizer. The slider therefore changes the source without bypassing the systems that
make it playable.

The trail overlay records both feet and both hands at three points in the pipeline: cyan is the
raw model animation, magenta is the reconstructed reference endpoint, and yellow is the final
fitted result. T or the menu button toggles the trails. This makes clipping, phase mismatch, and
over-correction visible while the character is being controlled normally.
