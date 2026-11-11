//! Framebuffer de color (RGBA8) más z-buffer (f32).

use crate::math::Vec3;

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    /// 4 bytes por pixel, en el formato que espera raylib (R, G, B, A).
    pub color: Vec<u8>,
    /// Profundidad en [0, 1]; 0 = plano cercano, 1 = plano lejano.
    pub depth: Vec<f32>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            color: vec![0; width * height * 4],
            depth: vec![f32::INFINITY; width * height],
        }
    }

    pub fn clear(&mut self, rgb: Vec3) {
        let [r, g, b] = to_rgb8(rgb);
        for px in self.color.chunks_exact_mut(4) {
            px.copy_from_slice(&[r, g, b, 255]);
        }
        self.depth.fill(f32::INFINITY);
    }

    #[inline]
    pub fn set_pixel(&mut self, idx: usize, rgb: Vec3) {
        let [r, g, b] = to_rgb8(rgb);
        let i = idx * 4;
        self.color[i] = r;
        self.color[i + 1] = g;
        self.color[i + 2] = b;
        self.color[i + 3] = 255;
    }

    #[inline]
    pub fn get_pixel(&self, idx: usize) -> Vec3 {
        let i = idx * 4;
        Vec3::new(
            self.color[i] as f32 / 255.0,
            self.color[i + 1] as f32 / 255.0,
            self.color[i + 2] as f32 / 255.0,
        )
    }
}

#[inline]
pub fn to_rgb8(c: Vec3) -> [u8; 3] {
    let c = c.clamp01();
    [
        (c.x * 255.0 + 0.5) as u8,
        (c.y * 255.0 + 0.5) as u8,
        (c.z * 255.0 + 0.5) as u8,
    ]
}
