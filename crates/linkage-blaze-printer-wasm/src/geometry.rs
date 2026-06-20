#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub min_x: f32,
    pub min_y: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub max_z: f32,
    empty: bool,
}

impl BoundingBox {
    pub fn empty() -> Self {
        Self {
            min_x: f32::MAX,
            min_y: f32::MAX,
            min_z: f32::MAX,
            max_x: f32::MIN,
            max_y: f32::MIN,
            max_z: f32::MIN,
            empty: true,
        }
    }

    pub fn extend(&mut self, x: f32, y: f32, z: f32) {
        self.empty = false;
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.min_z = self.min_z.min(z);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
        self.max_z = self.max_z.max(z);
    }

    pub fn to_flat_array(self) -> [f32; 6] {
        if self.empty {
            [0.0; 6]
        } else {
            [
                self.min_x, self.min_y, self.min_z, self.max_x, self.max_y, self.max_z,
            ]
        }
    }
}
