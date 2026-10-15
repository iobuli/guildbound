use std::ops::{Add, Div, Mul, Sub};

/// A vector on the ground plane (world X and Z).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub z: f32,
}

impl V2 {
    pub const ZERO: V2 = V2 { x: 0.0, z: 0.0 };

    pub const fn new(x: f32, z: f32) -> Self {
        Self { x, z }
    }

    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.z * o.z
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn normalized(self) -> V2 {
        let len = self.length();
        if len > 0.0 { self / len } else { V2::ZERO }
    }

    /// Signed angle in radians from self to o, positive turning from +X toward +Z.
    pub fn signed_angle_to(self, o: V2) -> f32 {
        (self.x * o.z - self.z * o.x).atan2(self.dot(o))
    }

    /// Rotate by radians, positive turning from +X toward +Z.
    pub fn rotated(self, radians: f32) -> V2 {
        let (s, c) = radians.sin_cos();
        V2::new(self.x * c - self.z * s, self.x * s + self.z * c)
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.z + o.z)
    }
}

impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.z - o.z)
    }
}

impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, s: f32) -> V2 {
        V2::new(self.x * s, self.z * s)
    }
}

impl Div<f32> for V2 {
    type Output = V2;
    fn div(self, s: f32) -> V2 {
        V2::new(self.x / s, self.z / s)
    }
}

// ---- f64 3D helpers (overhaul §API: the one shared Quat/Vec3/Mat3 module) ----

/// f64 3-vector.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
impl Vec3 {
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const Y: Vec3 = Vec3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn dot(self, o: Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
    pub fn clamp_length(self, max: f64) -> Vec3 {
        let l = self.length();
        if l > max && l > 0.0 {
            self * (max / l)
        } else {
            self
        }
    }
    pub fn to_f32(self) -> [f32; 3] {
        [self.x as f32, self.y as f32, self.z as f32]
    }
    pub fn from_f32(a: [f32; 3]) -> Vec3 {
        Vec3::new(a[0] as f64, a[1] as f64, a[2] as f64)
    }
}
impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl std::ops::Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        self * -1.0
    }
}

/// f64 unit quaternion (w, x, y, z).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
impl Default for Quat {
    fn default() -> Self {
        Quat::IDENTITY
    }
}
impl Quat {
    pub const IDENTITY: Quat = Quat {
        w: 1.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub fn conj(self) -> Quat {
        Quat {
            w: self.w,
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
    pub fn normalize(self) -> Quat {
        let n = (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if n > 1e-12 && n.is_finite() {
            Quat {
                w: self.w / n,
                x: self.x / n,
                y: self.y / n,
                z: self.z / n,
            }
        } else {
            Quat::IDENTITY
        }
    }
    pub fn rotate(self, v: Vec3) -> Vec3 {
        let u = Vec3::new(self.x, self.y, self.z);
        let t = u.cross(v) * 2.0;
        v + t * self.w + u.cross(t)
    }
    /// Rotation vector (axis·angle), shortest arc.
    pub fn to_rotvec(self) -> Vec3 {
        let q = if self.w < 0.0 {
            Quat {
                w: -self.w,
                x: -self.x,
                y: -self.y,
                z: -self.z,
            }
        } else {
            self
        };
        let s = (q.x * q.x + q.y * q.y + q.z * q.z).sqrt();
        if s < 1e-9 {
            return Vec3::new(q.x, q.y, q.z) * 2.0;
        }
        Vec3::new(q.x, q.y, q.z) * (2.0 * s.atan2(q.w) / s)
    }
    pub fn from_rotvec(r: Vec3) -> Quat {
        let a = r.length();
        if a < 1e-12 {
            return Quat {
                w: 1.0,
                x: r.x * 0.5,
                y: r.y * 0.5,
                z: r.z * 0.5,
            }
            .normalize();
        }
        let (s, c) = (0.5 * a).sin_cos();
        let k = s / a;
        Quat {
            w: c,
            x: r.x * k,
            y: r.y * k,
            z: r.z * k,
        }
    }
    /// Integrate a world angular velocity over dt.
    pub fn integrate(self, w: Vec3, dt: f64) -> Quat {
        (Quat::from_rotvec(w * dt) * self).normalize()
    }
}
impl Mul for Quat {
    type Output = Quat;
    fn mul(self, o: Quat) -> Quat {
        Quat {
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        }
    }
}

/// f64 row-major 3x3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3(pub [[f64; 3]; 3]);
impl Mat3 {
    pub const ZERO: Mat3 = Mat3([[0.0; 3]; 3]);
    pub fn diag(a: f64, b: f64, c: f64) -> Mat3 {
        Mat3([[a, 0.0, 0.0], [0.0, b, 0.0], [0.0, 0.0, c]])
    }
    pub fn mul_vec(&self, v: Vec3) -> Vec3 {
        let m = &self.0;
        Vec3::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        )
    }
    pub fn to_f32(&self) -> [[f32; 3]; 3] {
        std::array::from_fn(|i| std::array::from_fn(|j| self.0[i][j] as f32))
    }
    pub fn from_f32(m: [[f32; 3]; 3]) -> Mat3 {
        Mat3(std::array::from_fn(|i| {
            std::array::from_fn(|j| m[i][j] as f64)
        }))
    }
    /// Inverse via adjugate; None if singular.
    pub fn inverse(&self) -> Option<Mat3> {
        let a = &self.0;
        let c00 = a[1][1] * a[2][2] - a[1][2] * a[2][1];
        let c01 = a[1][2] * a[2][0] - a[1][0] * a[2][2];
        let c02 = a[1][0] * a[2][1] - a[1][1] * a[2][0];
        let det = a[0][0] * c00 + a[0][1] * c01 + a[0][2] * c02;
        if !det.is_finite() || det.abs() < 1e-15 {
            return None;
        }
        let r = [
            [
                c00,
                a[0][2] * a[2][1] - a[0][1] * a[2][2],
                a[0][1] * a[1][2] - a[0][2] * a[1][1],
            ],
            [
                c01,
                a[0][0] * a[2][2] - a[0][2] * a[2][0],
                a[0][2] * a[1][0] - a[0][0] * a[1][2],
            ],
            [
                c02,
                a[0][1] * a[2][0] - a[0][0] * a[2][1],
                a[0][0] * a[1][1] - a[0][1] * a[1][0],
            ],
        ];
        Some(Mat3(std::array::from_fn(|i| {
            std::array::from_fn(|j| r[i][j] / det)
        })))
    }
}
