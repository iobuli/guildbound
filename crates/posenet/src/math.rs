//! Just the vector and quaternion maths PoseNet needs (x, y, z, w quaternions; right-handed, Y up).

pub type V3 = [f32; 3];
pub type Q = [f32; 4];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn normalize(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-9 { scale(a, 1.0 / l) } else { [0.0, 0.0, 1.0] }
}

pub fn qmul(a: Q, b: Q) -> Q {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

pub fn qconj(q: Q) -> Q {
    [-q[0], -q[1], -q[2], q[3]]
}

pub fn qrot(q: Q, v: V3) -> V3 {
    let u = [q[0], q[1], q[2]];
    let t = scale(cross(u, v), 2.0);
    add(add(v, scale(t, q[3])), cross(u, t))
}

pub fn qyaw(yaw: f32) -> Q {
    let h = yaw * 0.5;
    [0.0, h.sin(), 0.0, h.cos()]
}

/// The rotation's matrix columns X and Y: the continuous 6D form.
pub fn to6(q: Q) -> [f32; 6] {
    let x = qrot(q, [1.0, 0.0, 0.0]);
    let y = qrot(q, [0.0, 1.0, 0.0]);
    [x[0], x[1], x[2], y[0], y[1], y[2]]
}

/// Back from 6D: Gram-Schmidt the two columns, the third is their cross product, then matrix -> quaternion.
pub fn from6(r: &[f32]) -> Q {
    let x = normalize([r[0], r[1], r[2]]);
    let y0 = [r[3], r[4], r[5]];
    let y = normalize(sub(y0, scale(x, dot(x, y0))));
    let z = cross(x, y);
    // rotation matrix columns x, y, z
    let (m00, m11, m22) = (x[0], y[1], z[2]);
    let tr = m00 + m11 + m22;
    let q = if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        [(y[2] - z[1]) / s, (z[0] - x[2]) / s, (x[1] - y[0]) / s, 0.25 * s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [0.25 * s, (y[0] + x[1]) / s, (z[0] + x[2]) / s, (y[2] - z[1]) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(y[0] + x[1]) / s, 0.25 * s, (z[1] + y[2]) / s, (z[0] - x[2]) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(z[0] + x[2]) / s, (z[1] + y[2]) / s, 0.25 * s, (x[1] - y[0]) / s]
    };
    let l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    [q[0] / l, q[1] / l, q[2] / l, q[3] / l]
}

/// Angle between two rotations (radians).
pub fn angle(a: Q, b: Q) -> f32 {
    // atan2 of the difference rotation's vector part over its scalar: stable where acos is not (near 0)
    let d = qmul(qconj(a), b);
    2.0 * (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().atan2(d[3].abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_d_round_trips_any_rotation() {
        for i in 0..200 {
            let t = i as f32 * 0.37;
            let axis = normalize([t.sin(), (t * 1.3).cos(), (t * 0.7).sin() + 0.2]);
            let h = (t * 2.1) % std::f32::consts::TAU * 0.5;
            let q = [axis[0] * h.sin(), axis[1] * h.sin(), axis[2] * h.sin(), h.cos()];
            assert!(angle(q, from6(&to6(q))) < 1e-3, "{q:?}");
        }
    }
}
