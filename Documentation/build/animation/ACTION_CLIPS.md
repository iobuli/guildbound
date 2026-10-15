# Armature action binding

Owned source rig and clips. Existing HeroAnimator extended, root motion disabled, combat windows unchanged.

| State | Source | Mirror | Contact (source s) | Follow-through (source s) |
|---|---|---|---:|---:|
| Idle | Assets/Brawler Animations Sample/Animations/Fight_Idle.fbx | False | 0.0000 | 0.0000 |
| WalkF | Assets/Brawler Animations Sample/Animations/Walk_F.fbx | False | 0.0000 | 0.0000 |
| WalkB | Assets/Brawler Animations Sample/Animations/Walk_B.fbx | False | 0.0000 | 0.0000 |
| Light1 | Assets/Brawler Animations Sample/Animations/Atk_P_1.fbx | False | 0.2322 | 0.2850 |
| Light2 | Assets/Brawler Animations Sample/Animations/Atk_P_2.fbx | True | 0.3061 | 0.3383 |
| Heavy1 | Assets/Brawler Animations Sample/Animations/Atk_P_2.fbx | False | 0.3061 | 0.3383 |
| Guard | Assets/Brawler Animations Sample/Animations/Guard_Idle.fbx | False | 0.0000 | 0.0000 |
| GuardHit | Assets/Brawler Animations Sample/Animations/Guard_Hit.fbx | False | 0.0000 | 0.0000 |
| HitF | Assets/Brawler Animations Sample/Animations/Hit_F.fbx | False | 0.0000 | 0.0000 |
| HitB | Assets/Brawler Animations Sample/Animations/Hit_B.fbx | False | 0.0000 | 0.0000 |
| StepF | Assets/Brawler Animations Sample/Animations/Quickstep_F.fbx | False | 0.0000 | 0.7333 |
| StepB | Assets/Brawler Animations Sample/Animations/Quickstep_B.fbx | False | 0.0000 | 0.7333 |
| GrabStart | Assets/Brawler Animations Sample/Animations/Grab_Start.fbx | False | 0.0000 | 0.0000 |
| GrabThrow | Assets/Brawler Animations Sample/Animations/Grab_Throw_A.fbx | False | 0.0000 | 0.0000 |
| GrabBreak | Assets/Brawler Animations Sample/Animations/Grab_Break_A.fbx | False | 0.0000 | 0.0000 |
| Knockdown | Assets/FightingMotionsVolume1/FBX/knockdown_A.fbx | False | 0.0000 | 0.0000 |
| Getup | Assets/FightingMotionsVolume1/FBX/getup_A.fbx | False | 0.0000 | 0.0000 |

Timing markers: measured maximum forward reach of the striking hand, then first sample with 10% retraction. These are presentation candidates, not verified collision points. 61 samples per clip. Light2 mirrors the left-handed Punch2. Heavy1 replaces the kick with that punch because the rules sweep a fist. Heavy2 remains an unused authoring state.

Transitions blend over 0.07 s. Startup/active/recovery sample the same rule clock; no animation event can authorize damage. Reaction clips traverse their complete source duration in the rule recovery window. Step traverses its complete clip in its active-only window.

Limitations: no foot fitting, hand-to-contact fitting, paired throw victim, cast/recover clips or hit VFX in this first binding. Cast/recover temporarily use guard stance. These remain presentation work; this binding does not pass those gates.

Importer: CombatClipImports uses lockRootPositionXZ=false on four Brawler imports plus knockdown_A/getup_A; the two FightingMotions clips use lockRootHeightY=true and lockRootRotation=true to retain the fall and rise in the bones. Source model metas are ignored, so the postprocessor reapplies these on pack unpack/reimport.

