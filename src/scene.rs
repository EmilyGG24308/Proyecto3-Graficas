//! La escena: qué objetos hay, dónde están en cada instante y cómo se dibujan.

use std::path::{Path, PathBuf};

use crate::camera::Camera;
use crate::collision::Collider;
use crate::config::*;
use crate::math::{Mat4, Vec3};
use crate::mesh::{self, Mesh};
use crate::noise::Perlin;
use crate::renderer::{DrawOptions, Renderer};
use crate::shader::Uniforms;
use crate::shaders::{SunShader, TestPlanetShader, ThrusterDraw, ThrusterParams, ThrusterShader, ToonShader};
use crate::ship::{self, ShipState};
use crate::starfield::Starfield;

/// Lo que la escena necesita saber de la nave para dibujarla.
#[derive(Clone, Copy)]
pub struct ShipView {
    pub state: ShipState,
    pub throttle: f32,
    pub boosting: bool,
    /// En vista de cabina no se dibuja el casco (estaríamos dentro).
    pub visible: bool,
}

pub struct Scene {
    stars: Starfield,
    sun_mesh: Mesh,
    planet_mesh: Mesh,
    ship_mesh: Mesh,
    flame_mesh: Mesh,
    sun_shader: SunShader,
    planet_shader: TestPlanetShader,
    toon: ToonShader,
    thruster: ThrusterShader,
}

/// Busca un recurso relativo a la carpeta actual o a la del proyecto.
pub fn asset_path(rel: &str) -> PathBuf {
    let local = Path::new(rel);
    if local.exists() {
        return local.to_path_buf();
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

impl Scene {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            stars: Starfield::new(STAR_COUNT, WORLD_SEED),
            sun_mesh: mesh::icosphere(4),
            planet_mesh: mesh::icosphere(TEST_PLANET_SUBDIV),
            ship_mesh: mesh::load_obj(asset_path("assets/models/nave.obj"))?,
            flame_mesh: mesh::cone(12, 4),
            sun_shader: SunShader { noise: Perlin::new(WORLD_SEED), speed: SUN_ANIM_SPEED, bands: 6.0 },
            planet_shader: TestPlanetShader { noise: Perlin::new(WORLD_SEED + 1) },
            toon: ToonShader { ambient: 0.36, outline: 0.35 },
            thruster: ThrusterShader { noise: Perlin::new(WORLD_SEED + 7) },
        })
    }

    pub fn sun_pos(&self) -> Vec3 {
        Vec3::ZERO
    }

    /// Posición del planeta de prueba en el instante `time` (segundos).
    /// Depende solo del tiempo transcurrido, nunca del número de cuadros.
    pub fn planet_pos(time: f32) -> Vec3 {
        let orbit = time * TEST_PLANET_ORBIT_SPEED;
        Vec3::new(orbit.cos(), 0.0, -orbit.sin()) * TEST_PLANET_ORBIT
    }

    /// Velocidad del planeta de prueba (derivada de `planet_pos`).
    pub fn planet_velocity(time: f32) -> Vec3 {
        let w = TEST_PLANET_ORBIT_SPEED;
        let a = time * w;
        Vec3::new(-a.sin(), 0.0, -a.cos()) * (TEST_PLANET_ORBIT * w)
    }

    /// Cuerpos sólidos en el instante `time`.
    pub fn colliders(&self, time: f32) -> Vec<Collider> {
        vec![
            Collider { center: self.sun_pos(), velocity: Vec3::ZERO, radius: SUN_RADIUS, name: "Sol" },
            Collider {
                center: Self::planet_pos(time),
                velocity: Self::planet_velocity(time),
                radius: TEST_PLANET_RADIUS,
                name: "Planeta",
            },
        ]
    }

    pub fn ship_triangles(&self) -> usize {
        self.ship_mesh.triangle_count()
    }

    pub fn render(&self, r: &mut Renderer, camera: &Camera, time: f32, ship: Option<&ShipView>) {
        let view = camera.view();
        let proj = camera.projection(RENDER_W as f32 / RENDER_H as f32);
        r.begin_frame(view, proj, Vec3::new(0.01, 0.01, 0.035));
        self.stars.draw(&mut r.fb, &view, &proj, time);

        let sun_pos = self.sun_pos();
        let base = Uniforms {
            time,
            camera_pos: camera.eye,
            sun_pos,
            model: Mat4::IDENTITY,
            object_center: Vec3::ZERO,
        };

        // --- Sol: rota despacio sobre su eje --------------------------------
        let sun_model = Mat4::translation(sun_pos)
            * Mat4::rotation_y(time * 0.05)
            * Mat4::uniform_scale(SUN_RADIUS);
        let u = Uniforms { model: sun_model, object_center: sun_pos, ..base };
        r.draw_mesh(&self.sun_mesh, &sun_model, &self.sun_shader, &u, DrawOptions::default());

        // --- Planeta: órbita * inclinación del eje * rotación * escala -------
        let planet_pos = Self::planet_pos(time);
        let planet_model = Mat4::translation(planet_pos)
            * Mat4::rotation_z(0.4)
            * Mat4::rotation_y(time * TEST_PLANET_SPIN_SPEED)
            * Mat4::uniform_scale(TEST_PLANET_RADIUS);
        let u = Uniforms { model: planet_model, object_center: planet_pos, ..base };
        r.draw_mesh(&self.planet_mesh, &planet_model, &self.planet_shader, &u, DrawOptions::default());

        // --- Nave -----------------------------------------------------------
        if let Some(sv) = ship.filter(|s| s.visible) {
            let model = ship::model_matrix(&sv.state);
            let u = Uniforms { model, object_center: sv.state.pos, ..base };
            // Dos caras: se ve el interior de la tobera.
            let opts = DrawOptions { double_sided: true, ..DrawOptions::default() };
            r.draw_mesh(&self.ship_mesh, &model, &self.toon, &u, opts);

            // Flama: al final porque es aditiva y no escribe profundidad.
            if sv.throttle > 0.02 {
                let len = 0.25 + sv.throttle * 1.6;
                let width = 0.17 + 0.05 * sv.throttle;
                // Coordenadas del modelo de la nave: la tobera termina en z = 1.14.
                let flame_model = model
                    * Mat4::translation(Vec3::new(0.0, 0.0, 1.1))
                    * Mat4::scale(Vec3::new(width, width, len));
                let u = Uniforms { model: flame_model, ..u };
                let draw = ThrusterDraw {
                    shader: &self.thruster,
                    params: ThrusterParams { intensity: 0.6 + 0.6 * sv.throttle, boost: sv.boosting },
                };
                let opts = DrawOptions { double_sided: true, depth_write: false, additive: true };
                r.draw_mesh(&self.flame_mesh, &flame_model, &draw, &u, opts);
            }
        }
    }
}
