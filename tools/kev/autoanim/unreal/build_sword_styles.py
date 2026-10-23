"""Export the first sword-chain combo for Trunks (Sword) and Tapion to Kev's skeleton."""
from pathlib import Path

import retarget_glb as R
from sparking_library import DATA, ROOT, catalog, export


def build(character: str, label: str) -> None:
    clips = catalog()["clips"]
    prefix = f"A_{character}_C_ATC_a001_R1_1_"
    sections = []
    for part in ("IN", "HT", "OT"):
        name = prefix + part
        item = next(c for c in clips if c["character"] == character and c["name"] == name)
        export(item["id"])
        sections.append(DATA / f"{item['id']}.json")

    output = ROOT / f"assets/anim/{label.lower()}_sword.glb"
    partial = output.with_name(output.stem + ".partial.glb")
    partial.unlink(missing_ok=True)
    R.convert(ROOT / "assets/anim/ual2.glb", partial, sections,
              sequence=f"{label}_Sword_Combo", animation_only=True)
    partial.replace(output)
    print(f"Wrote {output.relative_to(ROOT)}")


if __name__ == "__main__":
    build("0080", "Trunks")  # Trunks (Sword)
    build("0660", "Tapion")
