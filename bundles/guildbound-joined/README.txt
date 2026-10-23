Joined guildbound branch: the other agent's 976c47b + hand grips, doors (fixed), chest loot,
sheathing, blocking, command grab, CMU mocap, move learning, GB_PRESS, the hero character kit,
and the hero wearing the ink skin. It builds on origin/overhaul-2026-10-16 = 976c47b, so pushing
is a fast-forward (no conflicts). 262 tests pass.

PowerShell, from D:\gb_overhaul:
  git rebase --abort                      # only if the earlier rebase is still in progress
  git fetch origin
  git fetch "D:\gb_overhaul\guildbound-joined\guildbound-joined.bundle" joined-push:bundle/joined
  git checkout -B overhaul-2026-10-16 bundle/joined
  git push origin overhaul-2026-10-16
