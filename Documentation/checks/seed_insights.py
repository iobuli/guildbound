"""One-off seeding of the insight ledger with the insights Claude currently owes Codex."""
import subprocess, sys, os

TOOL = os.path.join(os.path.dirname(os.path.abspath(__file__)), "insight.py")
rows = [
    ("CORRECTION", "Spike v2 'seam-safe morph verified' claim is retracted: it measured 0 coincident-vertex clusters, so its 0 gap proves nothing.",
     "Documentation/experiments/chr_spike/spike2_log.txt (seam clusters 0)", "GDD S9 character pipeline, AMEND-CHR-01",
     "Measure overlap/gap between neighbouring disconnected body pieces per key and test pose in codex_character_lab and post the numbers.", "verified"),
    ("RISK", "No installed pack contains clothing, hair or accessory meshes; every outfit in the concepts must be derived from owned bodies.",
     "Logs/sheets/characters.png + chr review entry", "GDD S9 gaps, M0.5 deformation gate",
     "Confirm the derivation route (facet body copy -> cut clothing volumes -> rigid kit pieces -> palette) or propose a cheaper fallback (material-only clothing).", "inferred"),
    ("FINDING", "Trained FIGHTER and HEAVY builds are still too close in the 20 m silhouette row; CASTER separates clearly.",
     "Documentation/concepts/development/trained-builds.png", "GDD S8 trained-build readability gate (4 of 5 reviewers at 10/20 m)",
     "Push heavy to fuller hips/thighs with padded horizontal bands and fighter to crossed straps with a narrower waist; re-render the silhouette row.", "verified"),
    ("FINDING", "The round rug in the dressing-corner study is not a kit brick.",
     "Documentation/concepts/development/dressing-corner.png", "KIT object recipes, derivation record",
     "Record it as a derived cloth piece (disc with a banner-cloth material) or remove it.", "verified"),
    ("DECISION", "Depth tool: Depth Anything V2 Small (Apache-2.0, local, optional) is the recommended route for concept-depth experiments.",
     "Codex entry 14:40 UTC depth recommendation", "GDD S8 depth maps",
     "Boss decides whether to install it now or defer; Codex keeps the garment-patch workflow until then.", "inferred"),
    ("HANDOFF", "HUD master sheet: Claude owns the cell manifest and slicing checks, Codex owns generation, cleaning and the HTML layer preview.",
     "Documentation/concepts/hud/hud_sheet_manifest.json", "AMEND-UI-01",
     "Post the raw sheet path when generated; Claude slices and verifies all 64 cells against the manifest (C11/C12).", "verified"),
]
for t, title, ev, imp, ask, conf in rows:
    subprocess.run([sys.executable, "-I", TOOL, "add", "--type", t, "--title", title, "--evidence", ev, "--impact", imp, "--ask", ask, "--confidence", conf], check=True)
