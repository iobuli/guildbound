# Update method (how anything in the design gets changed)

Every change follows the same five steps. No step is skipped because a change looks small.

1. **Propose** in `TANDEM_REASONING.log`: a named change id, what changes, why, which GDD sections/kit entries/assets it touches, and whether it is P0 or P1.
2. **Counter** once: the other agent steel-mans or challenges it. Disagreement that survives goes to the boss as a two-option question.
3. **Apply** to the single source of truth for that kind of change (table below), never to a copy.
4. **Check**: run `python -I Documentation/checks/design_check.py` (exit 0 required; FAIL blocks the change; WARN must be explained in the log entry).
5. **Record**: commit with a message naming the change id; append a STATE block if milestone, tuning, images or assemblies changed; regenerate exports (atlas/PDF) only after checks pass.

| Change kind | Source of truth | Id prefix | Extra checks before commit |
|---|---|---|---|
| Design rule, scope, numbers | latest `BEGIN_MASTER_GDD` block (new version appended, never edited in place) | `AMEND-<AREA>-nn` | C02 sections 1-12, C03 IP clean, C04 P0/P1; every new number marked (H) until measured |
| Kit bricks, recipes, specs | latest `BEGIN_KIT_LEXICON` block | `AMEND-KIT-nn` | C05 three libraries + scenes, C06 IP clean; new brick verified on a contact sheet |
| Concept images | `Documentation/concepts/concept-prompts.json` + image files | spec id `CS-*` | C10 every image has a file + approval_state; images text-free unless a presentation board; anything the kit cannot build is captioned as illustrative |
| HUD art | `Documentation/concepts/hud/hud_sheet_manifest.json` + sheet PNG | cell id | C11 64 unique cells, C12 sheet 2048x2048 RGBA; replace single cells, never the whole sheet for one fix |
| Characters / derived meshes | `Documentation/experiments/*` (local) | `LAB-nn` | C13: derived meshes never in git; evidence = numbers + renders; Blender evidence never claimed as Unity proof |
| Exports | `FreeGuild-Design-Atlas.html/.pdf` | none | C08 IP clean + required sections, C09 matches latest GDD version |
| Operating rules | `TANDEM_*.manifest.md` (both identical) | boss only | C01 identical |

Always-on guards: C13 licence (no pack or derived meshes in git), C14 archives/Library untracked, C15 no source edits during design, C16 log append-only.

Build phase (after M0): the same five steps apply to code, with Unity checks added: compile clean (zero console errors), the gold-path driver, the layer fixtures and the reviewer checklist from GDD section 11.
