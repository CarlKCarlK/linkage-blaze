use core::{
    f32::consts::PI,
    ops::{Add, AddAssign, Index, IndexMut, Mul},
};

/// 3D position or vector [x, y, z].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3(pub [f32; 3]);

trait F32Ext {
    fn is_close_to(self, other: Self, tolerance: Self) -> bool;
}

impl F32Ext for f32 {
    fn is_close_to(self, other: Self, tolerance: Self) -> bool {
        (self - other).abs() <= tolerance
    }
}

impl Vec3 {
    pub const ZERO: Self = Self([0.0, 0.0, 0.0]);

    /// Return true when all components are within `tolerance`.
    #[must_use]
    pub fn is_close_to(&self, other: &Self, tolerance: f32) -> bool {
        self.0
            .iter()
            .zip(other.0.iter())
            .all(|(left, right)| left.is_close_to(*right, tolerance))
    }
}

impl From<[f32; 3]> for Vec3 {
    fn from(value: [f32; 3]) -> Self {
        Self(value)
    }
}

impl Index<usize> for Vec3 {
    type Output = f32;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for Vec3 {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl Add for Vec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self([self[0] + rhs[0], self[1] + rhs[1], self[2] + rhs[2]])
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self([self[0] * rhs, self[1] * rhs, self[2] * rhs])
    }
}

/// 3x3 rotation matrix, row-major: mat[row][col].
///
/// Columns are body-frame axes: col 0 = v0 (forward), col 1 = v1 (left), col 2 = v2 (up/back).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3(pub [[f32; 3]; 3]);

impl Mat3 {
    pub const IDENTITY: Self = Self([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    /// Rotation around z. Yaw = Rz: [[c,-s,0],[s,c,0],[0,0,1]].
    pub fn yaw(radians: f32) -> Self {
        let cos = libm::cosf(radians);
        let sin = libm::sinf(radians);
        Self([[cos, -sin, 0.0], [sin, cos, 0.0], [0.0, 0.0, 1.0]])
    }

    /// Rotation around y. Pitch = Ry: [[c,0,s],[0,1,0],[-s,0,c]].
    pub fn pitch(radians: f32) -> Self {
        let cos = libm::cosf(radians);
        let sin = libm::sinf(radians);
        Self([[cos, 0.0, sin], [0.0, 1.0, 0.0], [-sin, 0.0, cos]])
    }

    /// Rotation around x. Roll = Rx: [[1,0,0],[0,c,-s],[0,s,c]].
    pub fn roll(radians: f32) -> Self {
        let cos = libm::cosf(radians);
        let sin = libm::sinf(radians);
        Self([[1.0, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]])
    }

    /// Return v0, the forward axis stored in column 0.
    #[must_use]
    pub fn forward(&self) -> Vec3 {
        Vec3([self[0][0], self[1][0], self[2][0]])
    }

    /// Return true when all components are within `tolerance`.
    #[must_use]
    pub fn is_close_to(&self, other: &Self, tolerance: f32) -> bool {
        self.0
            .iter()
            .zip(other.0.iter())
            .all(|(left, right)| Vec3::from(*left).is_close_to(&Vec3::from(*right), tolerance))
    }
}

impl From<[[f32; 3]; 3]> for Mat3 {
    fn from(value: [[f32; 3]; 3]) -> Self {
        Self(value)
    }
}

impl Index<usize> for Mat3 {
    type Output = [f32; 3];

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for Mat3 {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl Mul for Mat3 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut out = [[0.0f32; 3]; 3];
        for row in 0..3 {
            for col in 0..3 {
                for component in 0..3 {
                    out[row][col] += self[row][component] * rhs[component][col];
                }
            }
        }
        Self(out)
    }
}

pub const fn degrees_to_radians(degrees: f32) -> f32 {
    degrees * (PI / 180.0)
}

//todo0000 make inline? and elsewhere?
//todo0000 why free functions? (may no longer apply)
// f32 comparison is now implemented by F32Ext.

#[cfg(test)]
mod tests {
    use super::{F32Ext, Mat3, Vec3, degrees_to_radians};
    use core::f32::consts::PI;

    #[test]
    fn test_degrees_to_radians() {
        assert!(degrees_to_radians(180.0).is_close_to(PI, 1e-6));
        assert!(degrees_to_radians(90.0).is_close_to(PI / 2.0, 1e-6));
    }

    #[test]
    fn test_vec3_add_and_scale() {
        let actual = Vec3::from([1.0, 2.0, 3.0]) + Vec3::from([4.0, -1.0, 0.5]) * 2.0;
        let expected = Vec3::from([9.0, 0.0, 4.0]);

        assert!(actual.is_close_to(&expected, 1e-6));
    }

    #[test]
    fn test_mat3_mul() {
        let left = Mat3::from([[1.0, 2.0, 3.0], [0.0, 1.0, 4.0], [5.0, 6.0, 0.0]]);
        let right = Mat3::from([[-2.0, 1.0, 0.0], [3.0, 0.0, 0.0], [4.0, 5.0, 1.0]]);
        let expected = Mat3::from([[16.0, 16.0, 3.0], [19.0, 20.0, 4.0], [8.0, 5.0, 0.0]]);

        assert!((left * right).is_close_to(&expected, 1e-6));
    }

    #[test]
    fn test_rotation_forward_axes() {
        let yaw_forward = Mat3::yaw(degrees_to_radians(90.0)).forward();
        let pitch_forward = Mat3::pitch(degrees_to_radians(90.0)).forward();
        let roll_forward = Mat3::roll(degrees_to_radians(90.0)).forward();

        assert!(yaw_forward.is_close_to(&Vec3::from([0.0, 1.0, 0.0]), 1e-6));
        assert!(pitch_forward.is_close_to(&Vec3::from([0.0, 0.0, -1.0]), 1e-6));
        assert!(roll_forward.is_close_to(&Vec3::from([1.0, 0.0, 0.0]), 1e-6));
    }

    #[test]
    fn test_mat3_is_close_to() {
        let actual = Mat3::from([[1.0001, 0.0, 0.0], [0.0, 0.9999, 0.0], [0.0, 0.0, 1.0001]]);
        let expected = Mat3::IDENTITY;

        assert!(actual.is_close_to(&expected, 0.001));
        assert!(!actual.is_close_to(&expected, 0.00001));
    }
}
