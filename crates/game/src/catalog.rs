//! What every model file under assets/packs is, read once from its glTF header (no loading): a rigged character
//! or a static prop, and its animation clips.
use serde_json::Value;

pub struct AssetInfo {
    /// Path under assets/.
    pub path: String,
    pub rigged: bool,
    pub clips: Vec<String>,
}

pub fn scan() -> Vec<AssetInfo> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<AssetInfo>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, out);
            } else if p.extension().is_some_and(|x| x == "glb" || x == "gltf") {
                let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                if !rel.contains('#') && !rel.contains("/Exports/FBX") {
                    out.push(read(&p, rel));
                }
            }
        }
    }
    let root = std::path::PathBuf::from(format!("{}/assets", crate::ROOT));
    let mut out = Vec::new();
    walk(&root.join("packs"), &root, &mut out);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

pub fn read(file: &std::path::Path, path: String) -> AssetInfo {
    let j = header(file).unwrap_or_default();
    let clips = j["animations"].as_array().into_iter().flatten().filter_map(|a| a["name"].as_str()).filter(|n| !n.starts_with("Unarmed-")).map(str::to_owned).collect();
    AssetInfo { rigged: j["skins"].as_array().is_some_and(|s| !s.is_empty()), clips, path }
}

/// The JSON chunk of a .glb, or the whole .gltf.
fn header(file: &std::path::Path) -> Option<Value> {
    use std::io::Read;
    let mut f = std::fs::File::open(file).ok()?;
    if file.extension().is_some_and(|x| x == "gltf") {
        return serde_json::from_reader(std::io::BufReader::new(f)).ok();
    }
    let mut head = [0u8; 20];
    f.read_exact(&mut head).ok()?;
    let len = u32::from_le_bytes(head[12..16].try_into().ok()?) as usize;
    let mut json = vec![0u8; len];
    f.read_exact(&mut json).ok()?;
    serde_json::from_slice(&json).ok()
}

/// Browser group of a model: (sort key, title) from its path.
pub fn pack_of(path: &str) -> (u8, String) {
    let p = path.trim_start_matches("packs/");
    let mut it = p.split('/');
    match it.next().unwrap_or("") {
        "custom" => (0, "Your characters (Creator)".into()),
        "hero" => (1, "Hero".into()),
        "npc" => (2, "Townsfolk".into()),
        "_sources" => match it.next().unwrap_or("") {
            "quaternius" => (3, format!("Quaternius - {}", it.next().unwrap_or("").replace('_', " "))),
            "chars_staged" => (4, format!("Character parts - {}", it.next().unwrap_or(""))),
            src => (5, format!("Other packs - {}", src.replace('_', " "))),
        },
        x => (6, x.replace('_', " ")),
    }
}

/// Which source a hero-library clip came from, by its name prefix (the build names them so).
pub fn hero_group(clip: &str) -> &'static str {
    let prefix = clip.split('_').next().unwrap_or("");
    match prefix {
        "u1" => "UAL 1 - everyday moves, swimming, pistol, sword, spell",
        "u2" => "UAL 2 - sword combos, shield, farming, zombie, parkour",
        "ual" => "UAL picks used by combat",
        "nc" => "Human Basic Motions - walk, run, sprint, jump",
        "mx" => "Fighting extras - blocks, deaths, turns, magic",
        "mage" => "Mage casts",
        "hp" | "bp" | "hk" | "lk" | "bk" | "hd" | "hb" | "bb" | "lb" | "ld" | "bd" | "move" | "knockdown" | "getup" | "win" => "RPG hero - strikes, hits, blocks",
        _ => "Other",
    }
}
