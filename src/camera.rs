//! Cámara: genera las matrices de vista y proyección.

use crate::math::{Mat4, Vec3};

#[derive(Clone, Copy)]
pub struct Camera {
    pub eye: Vec3,
    /// Dirección unitaria hacia donde mira.
    pub forward: Vec3,
    /// Vector "arriba" de la cámara (la nave puede rotar, así que no es fijo).
    pub up: Vec3,
    /// Campo de visión vertical en radianes.
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn view(&self) -> Mat4 {
        Mat4::look_at(self.eye, self.eye + self.forward, self.up)
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        Mat4::perspective(self.fov_y, aspect, self.near, self.far)
    }
}

/// Controlador de cámara libre (yaw/pitch). Se usa para depurar mientras la
/// nave no existe; más adelante la cámara seguirá a la nave.
pub struct FreeFly {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl FreeFly {
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(cp * sy, sp, -cp * cy)
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    pub fn camera(&self, fov_y: f32, near: f32, far: f32) -> Camera {
        Camera { eye: self.pos, forward: self.forward(), up: Vec3::Y, fov_y, near, far }
    }
}

// ---------------------------------------------------------------------------
// Cámara que sigue a la nave
// ---------------------------------------------------------------------------

use crate::collision::Collider;
use crate::config::*;
use crate::ship::ShipState;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CameraMode {
    /// Detrás y arriba de la nave, con seguimiento suave.
    ThirdPerson,
    /// Dentro de la cabina, mirando por el domo.
    Cockpit,
}

pub struct ShipCamera {
    pub mode: CameraMode,
    eye: Vec3,
    up: Vec3,
    look: Vec3,
    initialized: bool,
}

impl ShipCamera {
    pub fn new() -> Self {
        Self { mode: CameraMode::ThirdPerson, eye: Vec3::ZERO, up: Vec3::Y, look: Vec3::ZERO, initialized: false }
    }

    pub fn toggle(&mut self) {
        self.mode = match self.mode {
            CameraMode::ThirdPerson => CameraMode::Cockpit,
            CameraMode::Cockpit => CameraMode::ThirdPerson,
        };
    }

    /// Actualiza la cámara con el estado (ya interpolado) de la nave.
    /// El suavizado es exponencial con `dt`, así que no depende de los FPS.
    pub fn update(&mut self, s: &ShipState, speed: f32, dt: f32, colliders: &[Collider]) -> Camera {
        // Al ir rápido la cámara se aleja un poco y el FOV se abre: sensación
        // de velocidad.
        let speed_t = (speed / (SHIP_THRUST / SHIP_DRAG * SHIP_BOOST_MULT)).clamp(0.0, 1.0);
        let fov = (FOV_Y_DEG + 12.0 * speed_t).to_radians();

        match self.mode {
            CameraMode::ThirdPerson => {
                let dist = CAM_DISTANCE * (1.0 + 0.6 * speed_t);
                let target_eye = s.pos - s.forward * dist + s.up * CAM_HEIGHT;
                let target_look = s.pos + s.forward * CAM_LOOK_AHEAD + s.up * (CAM_HEIGHT * 0.35);
                if !self.initialized {
                    self.eye = target_eye;
                    self.look = target_look;
                    self.up = s.up;
                    self.initialized = true;
                }
                let k = 1.0 - (-CAM_FOLLOW * dt).exp();
                self.eye = self.eye.lerp(target_eye, k);
                self.look = self.look.lerp(target_look, k);
                self.up = self.up.lerp(s.up, k).normalize();
                // Si la cámara quedaría dentro de un planeta (volando pegado
                // a la superficie), se empuja hacia fuera.
                for c in colliders {
                    let d = self.eye - c.center;
                    let min = c.radius_toward(self.eye) + CAM_SURFACE_MARGIN;
                    if d.length() < min {
                        self.eye = c.center + d.normalize() * min;
                    }
                }
                Camera {
                    eye: self.eye,
                    forward: (self.look - self.eye).normalize(),
                    up: self.up,
                    fov_y: fov,
                    near: NEAR,
                    far: FAR,
                }
            }
            CameraMode::Cockpit => {
                // Posición del piloto dentro del domo (en unidades del modelo
                // escaladas): un poco adelante y arriba del centro.
                let eye = s.pos + s.forward * (0.05 * SHIP_SCALE) + s.up * (0.45 * SHIP_SCALE);
                // Al volver a tercera persona, partir desde aquí.
                self.eye = eye;
                self.look = eye + s.forward;
                self.up = s.up;
                Camera { eye, forward: s.forward, up: s.up, fov_y: fov, near: NEAR, far: FAR }
            }
        }
    }
}
