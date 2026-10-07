# Combat frame data

Generated from the clips by `HeroAnim.swing_window` (the striking limb's fast stretch) with
`godot --headless --path . -- --selftest --only=framedata --write-framedata`. Frames at 60 fps at each move's
clip speed with attack speed 1.5 (neutral: the attack-speed factor, clamped to x0.75..x1.4, divides every
column). A chained light hit rushes its wind-up (clip time up to 0.22 s before the blow plays
`HeroAnim.WARP` x faster), so its real startup is shorter. Spell rows (F, any weapon, upper body only):
startup = frames until the spell leaves the hand, then the rest of the clip; the legs stay free throughout.

Parry: guard pressed within 15 frames (0.25 s) before any blow lands parries it (attacker staggered
0.9 s); within the last 6 frames (0.10 s) it is a PERFECT parry (1.8 s stagger, slow motion). A held
guard blocks (chip damage). Parried projectiles are deflected only with a shield or enough spell power.

| style | move | clip | startup | active | recovery | total |
|---|---|---|---|---|---|---|
| sword_shield | light 1 | SS_slash | 24 | 11 | 37 | 72 |
| sword_shield | light 2 | SS_slash_3 | 34 | 11 | 35 | 80 |
| sword_shield | light 3 | SS_slash_5 | 22 | 15 | 31 | 68 |
| sword_shield | light 4 | SS_attack_3 | 28 | 22 | 40 | 90 |
| sword_shield | heavy | SS_attack_2 | 12 | 26 | 27 | 65 |
| sword_shield | special | SS_kick | 14 | 23 | 26 | 63 |
| greatsword | light 1 | GS_slash | 21 | 21 | 24 | 66 |
| greatsword | light 2 | GS_slash_4 | 3 | 77 | 14 | 94 |
| greatsword | light 3 | GS_attack | 20 | 16 | 29 | 65 |
| greatsword | heavy | GS_high_spin_attack | 47 | 21 | 29 | 97 |
| greatsword | special | GS_jump_attack | 47 | 52 | 14 | 113 |
| greatsword | sprint_attack | GS_slide_attack | 0 | 90 | 17 | 107 |
| dagger | light 1 | SS_slash_4 | 46 | 9 | 36 | 91 |
| dagger | light 2 | SS_slash | 18 | 8 | 27 | 53 |
| dagger | light 3 | SS_slash_3 | 25 | 8 | 26 | 59 |
| dagger | light 4 | SS_attack_2 | 9 | 20 | 20 | 49 |
| dagger | heavy | SS_attack_4 | 16 | 11 | 16 | 43 |
| dagger | special | BOW_melee_kick | 13 | 30 | 29 | 72 |
| bow | light 1 | BOW_melee_punch | 9 | 11 | 26 | 46 |
| bow | heavy | BOW_melee_kick | 13 | 30 | 29 | 72 |
| bow | special | BOW_melee_kick | 13 | 30 | 29 | 72 |
| unarmed | light 1 | FIST_jab | 11 | 14 | 21 | 46 |
| unarmed | light 2 | FIST_cross | 12 | 11 | 34 | 57 |
| unarmed | light 3 | FIST_hook | 14 | 11 | 20 | 45 |
| unarmed | light 4 | FIST_front_kick | 11 | 15 | 25 | 51 |
| unarmed | heavy | FIST_roundhouse | 2 | 20 | 36 | 58 |
| unarmed | special | FIST_uppercut | 13 | 24 | 8 | 45 |
| sword_shield (F) | light spell | SS_casting_2 (upper body) | 14 | 1 | 21 | 36 |
| sword_shield (F) | heavy spell | SS_casting_2 (upper body) | 24 | 1 | 29 | 54 |
| greatsword (F) | light spell | GS_spell_cast (upper body) | 16 | 1 | 23 | 40 |
| greatsword (F) | heavy spell | GS_spell_cast (upper body) | 27 | 1 | 31 | 59 |
| bow (F) | light spell | MAG_1h_cast_spell_01 (upper body) | 32 | 1 | 47 | 80 |
| bow (F) | heavy spell | MAG_1h_cast_spell_01 (upper body) | 53 | 1 | 64 | 118 |
