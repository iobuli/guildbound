# HUD brainstorm - Claude (logical lane)

Status: proposal for the boss and Aura. No code, no HUD edits.

Independence note: I wrote this after reading Aura's pasted list in the same message, so it is not a clean independent pass. Where an idea overlaps Aura's list I say so. My grounding is the reference board, Assets/Game/UI/Kit/LAYOUT.md, HudView.cs fields, the kit sprites, and the rules' ActionState timing. My attempt to fetch the design articles failed, so I have not verified their text myself. I do not cite them here.

## Principle I am designing from

The HUD should answer the three questions a fighter asks in the first second of each exchange: what is my body doing, what is the other body about to do, and what do I owe or earn. Most of the current board answers the first and none of the second. Opponent intent is the largest blind spot in a timing-based fighter.

## Best ideas, ranked

1. **Opponent tell ring (new, high value).** Show the opponent's current ActionState window on their minimap blip: a filling arc through Startup, a flash through Active, a fade through Recovery. The rules already expose Timing and Elapsed. This makes the 0.25 s buffer and the cancel-on-hit window readable. Cost: one ring on the blip, data already available. Risk: must not reveal anything the rules hide from the player. It only shows what the player could infer from the animation, so it stays fair.

2. **Input buffer pip (new).** When a press is buffered inside the 0.25 s late-recovery window, show a small pip on the fire slot or under the guard segments. It clears when the buffer is consumed or expires. Players learn the buffer by seeing it, without a tutorial. Cost: a flag from the driver, no new art (icon_slot_diamond or glow_blob).

3. **Defeat as a ledger line (new, closes W6).** A defeat writes a line to the board's ledger ("Carried to the bench by X. Repair 40 g.") instead of a game-over panel. The sad path carries consequence and information. Cost: one string and one board list, reusing the alert pattern.

4. **Earnings as a trend, not a total.** The wallet shows a small sparkline of the last 7 contract payouts beside the number. Progress reads as a rate, which is what a player needs to plan the next purchase. Cost: a float array, one line renderer or a tiny Image strip. Overlaps Aura's odometer idea; the trend is the part I would keep.

5. **Objective tick on the compass tape.** The objective's bearing becomes a tick on the existing heading tape, with distance in steps rather than metres. This ties the quest's "why" to the world without a second map. Cost: one extra tick on the tape, which already slides at 5 px per degree.

6. **Contextual density (three weathers).** The HUD changes density from game state, not a player slider:
   - Town: board, wallet and map live; combat panel parked.
   - Approach: objective row expands.
   - Combat: everything except HP, guard, fire and alerts fades to its layer floor.
   Cost: drives the existing per-layer opacity values from state. Keeps the centre third empty, as LAYOUT.md already requires.

7. **Fold panels to pages (overlaps Aura).** Each panel can fold to a spine on the ledger, so the player chooses what is on screen. I would make it a hold-to-pin, not a menu. Overlaps Aura's dossier idea; I list it for convergence only.

## What I would not do

- A 3-row always-on quest list with the same text weight as combat. It competes with the fight for attention.
- A party or roster column that is decorative. If the column stays, it must mean something (trust, or who is in the bout), or it should go.
- Importing the Fantasy Warrior art. Licence and pack ownership make it a no-go; restyle our kit instead.

## Where I would start, if the boss picks

Ideas 1 and 2 first. They make combat legible, need no art, and read directly off the rules. Ideas 3 and 5 next, for the bench loop and the world's "why". Idea 6 last, once the content is stable.

## Convergence points for Aura

- Overlap: quest glyphs, cost-to-earn shop, portrait-as-trust, folding panels, odometer. Aura's list leans toward presentation; mine leans toward legibility of the fight.
- Disagreement: I would drop the always-on 3-row quest list in favour of ideas 3 and 5. Aura's list keeps it.
- Open question for the boss: does the opponent tell (idea 1) count as player-facing information or a debug aid? I recommend player-facing, but it changes what the rules must publish.
