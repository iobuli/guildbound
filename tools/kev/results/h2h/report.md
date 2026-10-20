# Hand-to-hand plans by enemy weapon

## Strike timing (fractions of the clip window)

`game cutter`: taken from the game's own strike table (repertoire::segment), so Kev and the game agree.
`kev fk`: the game's cutter found no strike in this clip; Kev measured it from its motion.

| track | source | hand | windup | active | cancel | peak m/s | mid-swing end |
|---|---|---|---|---|---|---|---|
| ual1/Punch_Jab | game cutter | l | 0.2 | 0.44 | 0.59 | 10.69 | False |
| ual1/Punch_Cross | game cutter | r | 0.193 | 0.416 | 0.566 | 8.8 | False |
| ual2/Melee_Hook | game cutter | r | 0.412 | 0.676 | 0.826 | 38.87 | False |
| ual2/OverhandThrow | game cutter | r | 0.337 | 0.536 | 0.686 | 21.23 | False |
| ual2/Zombie_Scratch | game cutter | r | 0.247 | 0.441 | 0.591 | 22.03 | True |
| ual2/Shield_OneShot | kev fk | l | 0.08 | 0.2 | 0.24 | 8.69 | False |
| ual2/Shield_Dash | kev fk | r | 0.061 | 0.121 | 0.121 | 11.31 | False |

## Plans

| enemy | reach m | windup s | ranged | chain | cost | heavy | parry s | absorb | arc |
|---|---|---|---|---|---|---|---|---|---|
| axe | 1.57 | 0.149 | False | jab > cross | -12.25 | kick | 0.11 | 0.6 | 120 |
| bow | 0.7 | None | True | hook > jab > hook > jab | -17.1 | charge | 0.2 | 0.75 | 160 |
| crossbow | 0.7 | None | True | jab > hook > jab > cross | -16.4 | shoulder bash | 0.2 | 0.55 | 150 |
| dagger | 1.0 | 0.247 | False | hook > jab | -10.35 | kick | 0.129 | 0.6 | 120 |
| greatsword | 1.85 | 0.529 | False | hook > jab > cross > jab | -15.85 | shoulder bash | 0.186 | 0.4 | 120 |
| katana | 1.45 | 0.313 | False | hook > jab > cross | -9.6 | kick | 0.143 | 0.6 | 120 |
| knives | 0.86 | None | True | jab > cross > jab > cross | -15.15 | kick | 0.18 | 0.82 | 171 |

## Where the chain lands, and the chain picked for each body

The game's enemies wear the target rig; the chain above is planned on it. The other bodies show what the
planner would pick against the same weapon on a body of that size (targets.py).

| enemy | body |
|---|---|
| axe | jab > cross (head, head) |
| bow | hook > jab > hook > jab (torso, head, torso, head) |
| crossbow | jab > hook > jab > cross (head, torso, head, head) |
| dagger | hook > jab (torso, head) |
| greatsword | hook > jab > cross > jab (torso, head, head, head) |
| katana | hook > jab > cross (torso, head, head) |
| knives | jab > cross > jab > cross (head, head, head, head) |
