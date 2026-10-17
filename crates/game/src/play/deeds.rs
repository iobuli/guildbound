//! Deeds: anything a character does or suffers, as a tagged record, the only input to
//! progression (`skills`) and to contract progress (`contracts`). Producers write `Deed` and nothing else about progression:
//! `combat` (hit: a melee blow that landed, tagged with the wielded moveset; block, parry, kill: tagged with the victim's name;
//! struck: the victim took a blow, tagged Melee / Projectile / Spell; ward: an Aura Ward turned one),
//! `magic` (cast, cast_hit: tagged with the ability id), `ranged` (shoot, shoot_hit: tagged with
//! the projectile kind), `traverse` (swim, dive and climb per metre, mantle), `parkour` (land: tagged
//! `Roll` / `Hard`), `trees` (chop, fell), `skills` (survive: a close call lived through). `skills` also turns `items::ItemCollected` into
//! `gather` (tagged with the item kind, amount = count). Blows on training dummies count
//! like any other (practice teaches; `skills` caps XP per window so nothing can be farmed).
use bevy::prelude::*;

#[derive(Message, Clone, Debug, PartialEq)]
pub struct Deed {
    pub who: Entity,
    /// What happened: "hit", "block", "parry", "kill", "cast", "cast_hit", "shoot",
    /// "shoot_hit", "swim", "climb", "mantle", "land", "chop", "fell", "gather", "struck", "dive", "ward",
    /// "balance" (a metre on a beam), "beam" ("crossed" / "fell"),
    /// "survive".
    pub kind: &'static str,
    /// Detail matched by skill sources: a moveset, ability id, projectile kind, item, grade.
    pub tag: String,
    /// How many (1 per event or metre; the count for gather, blows for cast_hit).
    pub amount: f32,
}

impl Deed {
    pub fn new(who: Entity, kind: &'static str, tag: impl Into<String>, amount: f32) -> Self {
        Self {
            who,
            kind,
            tag: tag.into(),
            amount,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_message::<Deed>();
}
