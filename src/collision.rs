//! Colisión básica: la nave (una esfera pequeña) contra cuerpos sólidos
//! (planetas con relieve y el sol) que pueden estar moviéndose en su órbita.

use crate::math::{Mat4, Vec3};
use crate::planet::Planet;

/// Forma de la superficie de un cuerpo.
#[derive(Clone, Copy)]
pub enum Surface<'a> {
    /// Esfera perfecta de este radio.
    Sphere(f32),
    /// Terreno de un planeta: el radio depende de la dirección.
    /// `inv_rotation` lleva direcciones de mundo al espacio local del planeta
    /// (es la transpuesta de su rotación actual).
    Terrain { planet: &'a Planet, inv_rotation: Mat4 },
}

/// Un cuerpo sólido con el que la nave puede chocar.
#[derive(Clone, Copy)]
pub struct Collider<'a> {
    pub center: Vec3,
    /// Velocidad del cuerpo (los planetas orbitan): el rebote se calcula
    /// relativo a ella para que la nave no se "hunda" en un planeta que la
    /// alcanza por detrás.
    pub velocity: Vec3,
    pub surface: Surface<'a>,
    /// Nombre para el HUD.
    pub name: &'static str,
}

/// Resultado de resolver una colisión en un paso.
#[derive(Clone, Copy, Debug)]
pub struct Contact {
    /// Rapidez con la que se golpeó la superficie (para efectos / HUD).
    pub impact_speed: f32,
}

impl<'a> Collider<'a> {
    pub fn sphere(center: Vec3, velocity: Vec3, radius: f32, name: &'static str) -> Self {
        Self { center, velocity, surface: Surface::Sphere(radius), name }
    }

    /// Radio de la superficie justo debajo del punto `p` (en mundo).
    pub fn radius_toward(&self, p: Vec3) -> f32 {
        match self.surface {
            Surface::Sphere(r) => r,
            Surface::Terrain { planet, inv_rotation } => {
                let dir_local = inv_rotation.transform_dir((p - self.center).normalize());
                planet.surface_radius_local(dir_local)
            }
        }
    }

    /// Distancia de un punto a la superficie (negativa si está dentro).
    /// Se mide a lo largo de la vertical (dirección al centro).
    pub fn signed_distance(&self, p: Vec3) -> f32 {
        (p - self.center).length() - self.radius_toward(p)
    }

    /// Empuja una esfera de radio `r` (centro `pos`, velocidad `vel`) fuera
    /// del cuerpo si lo está atravesando y corrige su velocidad:
    ///  - se elimina la componente que va hacia dentro (con un rebote leve),
    ///  - se aplica fricción a la componente tangencial (roza la superficie).
    pub fn resolve(&self, pos: &mut Vec3, vel: &mut Vec3, r: f32, restitution: f32, friction: f32) -> Option<Contact> {
        let d = *pos - self.center;
        let dist = d.length();
        let min = self.radius_toward(*pos) + r;
        if dist >= min {
            return None;
        }
        // Normal de la superficie (si justo está en el centro, empujar hacia arriba).
        let n = if dist > 1e-6 { d / dist } else { Vec3::Y };
        *pos = self.center + n * min;

        let rel = *vel - self.velocity;
        let vn = rel.dot(n);
        if vn >= 0.0 {
            // Ya se está alejando: solo se corrigió la posición.
            return Some(Contact { impact_speed: 0.0 });
        }
        let normal = n * vn;
        let tangent = rel - normal;
        let new_rel = tangent * (1.0 - friction) - normal * restitution;
        *vel = new_rel + self.velocity;
        Some(Contact { impact_speed: -vn })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushes_out_and_bounces() {
        let c = Collider::sphere(Vec3::ZERO, Vec3::ZERO, 5.0, "p");
        let mut pos = Vec3::new(0.0, 0.0, 5.1);
        let mut vel = Vec3::new(1.0, 0.0, -10.0);
        let hit = c.resolve(&mut pos, &mut vel, 0.5, 0.2, 0.1).unwrap();
        assert!((pos.length() - 5.5).abs() < 1e-5);
        assert!((hit.impact_speed - 10.0).abs() < 1e-5);
        assert!(vel.z > 0.0, "debe rebotar hacia fuera");
        assert!((vel.x - 0.9).abs() < 1e-5, "fricción tangencial");
    }

    #[test]
    fn moving_planet_carries_ship() {
        // El planeta viene hacia la nave a 3 u/s; la nave está quieta.
        let c = Collider::sphere(Vec3::ZERO, Vec3::new(0.0, 0.0, 3.0), 5.0, "p");
        let mut pos = Vec3::new(0.0, 0.0, 5.2);
        let mut vel = Vec3::ZERO;
        c.resolve(&mut pos, &mut vel, 0.5, 0.0, 0.0);
        assert!((vel.z - 3.0).abs() < 1e-5, "la nave sale con la velocidad del planeta");
    }
}
