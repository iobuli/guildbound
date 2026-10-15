//! PoseNet: a small neural network that knows what a natural body pose looks like, learned from every clip in
//! the animation library. Given where the head, hands and feet are (relative to the pelvis, in the character's
//! facing frame) and how high the pelvis is, it answers with every body bone's rotation. Possession uses it to
//! pose the whole body from a few dragged joint balls, and to fill in every frame between key poses.
//!
//! Plain Rust, no dependencies: a 3-layer perceptron (~100k weights, about 0.05 ms per pose), Adam training,
//! inputs and outputs normalised, rotations as the 6D form (two matrix columns) that networks learn smoothly.

pub mod math;
pub mod net;
pub mod pose;

pub use net::Mlp;
pub use pose::{Dataset, BONES, EFFECTORS, FEATURES, OUTPUTS};
