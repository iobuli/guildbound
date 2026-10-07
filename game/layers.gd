class_name Layers
extends RefCounted
## Collision layers shared by the world, characters, props and projectiles.

const WORLD := 1
const PLAYER := 2     # character capsules
const HITBOX := 4     # reserved (hurtboxes)
const PROPS := 8
const DEBRIS := 64    # ragdoll pieces and dropped weapons: they collide with the world only
