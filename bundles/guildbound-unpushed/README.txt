Unpushed guildbound work (base: origin/overhaul-2026-10-16 @ 65e1255)

guildbound-unpushed.bundle holds both branches:
  overhaul-2026-10-16  7 commits: hand grips, doors, chest loot, sheathing, blocking,
                       command grab, CMU mocap import, move learning, GB_PRESS, docs
  hero-styled          2 commits: the door commit plus "yo" (hero character kit)

To push from your own clone of iobuli/guildbound:
  git fetch origin
  git fetch /path/to/guildbound-unpushed.bundle overhaul-2026-10-16:overhaul-2026-10-16 hero-styled:hero-styled
  git push -u origin overhaul-2026-10-16 hero-styled

patches-*/ hold the same commits as plain .patch files (git am).
