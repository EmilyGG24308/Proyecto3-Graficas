//! Fondo de estrellas. Cada estrella es una dirección fija en el infinito:
//! solo se le aplica la rotación de la vista (no la traslación), así que
//! nunca se acercan aunque la nave viaje.

use crate::framebuffer::Framebuffer;
use crate::math::{Mat4, Vec3};
use crate::noise::Rng;

struct Star {
    dir: Vec3,
    brightness: f32,
    tint: Vec3,
    phase: f32,
}

pub struct Starfield {
    stars: Vec<Star>,
}

impl Starfield {
    pub fn new(count: usize, seed: u64) -> Self {
        let mut rng = Rng::new(seed ^ 0x5747_A125);
        let palette = [
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.75, 0.85, 1.0),
            Vec3::new(1.0, 0.9, 0.7),
            Vec3::new(1.0, 0.75, 0.9),
        ];
        let stars = (0..count)
            .map(|_| {
                // Dirección uniforme en la esfera.
                let z = rng.range(-1.0, 1.0);
                let a = rng.range(0.0, std::f32::consts::TAU);
                let r = (1.0 - z * z).sqrt();
                let b = rng.next_f32();
                Star {
                    dir: Vec3::new(r * a.cos(), z, r * a.sin()),
                    brightness: 0.25 + 0.75 * b * b * b,
                    tint: palette[(rng.next_u64() % 4) as usize],
                    phase: rng.range(0.0, 10.0),
                }
            })
            .collect();
        Self { stars }
    }

    pub fn draw(&self, fb: &mut Framebuffer, view: &Mat4, proj: &Mat4, time: f32) {
        let (w, h) = (fb.width as f32, fb.height as f32);
        for s in &self.stars {
            let d = view.transform_dir(s.dir);
            if d.z >= -0.01 {
                continue; // detrás de la cámara
            }
            let x_ndc = proj.m[0][0] * d.x / -d.z;
            let y_ndc = proj.m[1][1] * d.y / -d.z;
            let x = ((x_ndc + 1.0) * 0.5 * w) as i32;
            let y = ((1.0 - y_ndc) * 0.5 * h) as i32;
            if x < 0 || y < 0 || x >= fb.width as i32 || y >= fb.height as i32 {
                continue;
            }
            let twinkle = 0.75 + 0.25 * (time * 2.3 + s.phase).sin();
            let idx = y as usize * fb.width + x as usize;
            let c = fb.get_pixel(idx) + s.tint * (s.brightness * twinkle);
            fb.set_pixel(idx, c);
        }
    }
}
