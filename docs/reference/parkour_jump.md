# Reference: parkour running jump

Source: Pinterest pin 445152744423942375, "Jump Parkour - Body Mechanics" (Theodore Lemuel). It embeds Vimeo video 137029928. The only frame seen so far is the 236 px thumbnail: a grey mannequin mid-run toward a rooftop edge, about to leap.

**Status: frames pending.** Vimeo serves this video only to logged-in sessions, so `yt-dlp` gets HTTP 401 without cookies. Drop the mp4 at `staging/ref/parkour.mp4` and the frame sheets can be cut with the bundled ffmpeg:

```
D:\dev\refpkgs\imageio_ffmpeg\binaries\ffmpeg-win-x86_64-v7.1.exe -i staging/ref/parkour.mp4 -vf "fps=12,scale=320:-1,tile=6x4" staging/ref/sheet_%02d.png
```

## Phase breakdown (to fill from the frames)

| Phase | Frames | Seconds | Notes (contacts, COM, arms, legs) |
|---|---|---|---|
| run-in | | | |
| plant / anticipation | | | |
| takeoff | | | |
| rise | | | |
| apex | | | |
| descent | | | |
| contact | | | |
| absorb | | | |
| roll / recover | | | |

## Parameters in code (current defaults, from body mechanics)

| Constant | File | Default | Meaning | From reference |
|---|---|---|---|---|
| `LEAP_SPEED` | `play/select.rs` | 3.0 m/s | takeoff speed that switches to the leap poses | |
| `LEAP_START_WINDOW` / `OFFSET` / `RATE` | `play/select.rs` | 0.25 s / 0.2 s / 1.2 | leap push-off segment of `ual2/NinjaJump_Start` | takeoff + rise timing |
| `LEAP_FALL_AFTER` | `play/select.rs` | 0.9 s | airborne time before the fall pose | apex hang |
| `ROLL_IMPACT` / `ROLL_SPEED` | `play/parkour.rs` | 7.0 / 3.0 m/s | roll grade | |
| `HARD_IMPACT` | `play/parkour.rs` | 9.0 m/s | hard-landing grade | |
| `ROLL_OFFSET` / `ROLL_RATE` / `ROLL_GET_UP` | `play/parkour.rs` | 0.3 s / 1.6 / 0.7 | roll segment and when control returns | contact → recover timing |
| `ROLL_EXIT` | `play/parkour.rs` | 0.85 × run | speed carried out of the roll | |
| `HARD_LOCK` (+ per m/s, max) | `play/parkour.rs` | 0.35 s (+0.04, 0.8) | hard-landing lock | absorb depth/time |

Landing lines in `staging/relations.log` (`land_Roll`, `land_Hard`, `land_Soft`, with impact, takeoff and speed) give the in-game numbers to compare against.
