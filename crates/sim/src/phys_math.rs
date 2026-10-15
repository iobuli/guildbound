//! Physics: shared small-vector maths for the phys_* modules. Pure f32, no deps, deterministic.

pub type V3 = [f32; 3];
pub const G: f32 = 9.81;

pub fn add(a: V3, b: V3) -> V3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
pub fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
pub fn scale(a: V3, s: f32) -> V3 { [a[0] * s, a[1] * s, a[2] * s] }
pub fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
pub fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
pub fn len(a: V3) -> f32 { dot(a, a).sqrt() }
/// Unit vector, or None when |a| <= 1e-6.
pub fn normalize(a: V3) -> Option<V3> { let l = len(a); (l > 1e-6).then(|| scale(a, 1.0 / l)) }
