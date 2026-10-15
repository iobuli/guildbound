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
