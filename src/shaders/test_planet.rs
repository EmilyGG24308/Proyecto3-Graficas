//! Shader de prueba para el primer planeta: terreno por ruido (océano,
//! playa, pasto, montaña) con iluminación de Lambert desde el sol y una
//! atmósfera en el borde. Sirve para validar el pipeline; luego cada planeta
//! tendrá su propio shader.

use crate::math::{Vec3, smoothstep};
use crate::noise::Perlin;
use crate::shader::{Shader, Uniforms, Varyings, lambert, rim};

pub struct TestPlanetShader {
    pub noise: Perlin,
}

impl Shader for TestPlanetShader {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let n = v.normal.normalize();
        let h = self.noise.fbm(v.local_pos * 2.0, 5);

        let ocean = Vec3::new(0.10, 0.35, 0.75);
        let sand = Vec3::new(0.93, 0.85, 0.55);
        let grass = Vec3::new(0.30, 0.75, 0.30);
        let rock = Vec3::new(0.55, 0.50, 0.48);

        let albedo = if h < 0.0 {
            ocean
        } else if h < 0.05 {
            sand
        } else {
            grass.lerp(rock, smoothstep(0.2, 0.4, h))
        };

        let diffuse = lambert(n, v.world_pos, u.sun_pos);
        let ambient = 0.08;
        let lit = albedo * (ambient + diffuse * 0.95);

        let atmo = smoothstep(0.5, 1.0, rim(n, v.world_pos, u.camera_pos)) * (0.25 + 0.75 * diffuse);
        lit + Vec3::new(0.35, 0.6, 1.0) * atmo * 0.6
    }
}
