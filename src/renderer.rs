//! Pipeline de rasterización por software.
//!
//! Etapas por cada draw call:
//!  1. Vertex stage: cada vértice pasa por Model -> (mundo) -> View*Projection
//!     (clip space). Se calculan los varyings (posición de mundo, normal, uv…).
//!  2. Ensamblado de triángulos + rechazo trivial: si los 3 vértices quedan
//!     fuera del mismo plano del frustum, el triángulo se descarta.
//!  3. Recorte contra el plano cercano (Sutherland–Hodgman) en clip space,
//!     para que nada con w <= 0 llegue a la división de perspectiva.
//!  4. División de perspectiva (NDC) y transformación de viewport (pixeles).
//!  5. Backface culling según el signo del área en pantalla.
//!  6. Rasterización con funciones de arista y coordenadas baricéntricas,
//!     interpolación con corrección de perspectiva y prueba de profundidad.
//!  7. Fragment shader -> framebuffer.

use crate::framebuffer::Framebuffer;
use crate::math::{Mat4, Vec3, Vec4};
use crate::mesh::Mesh;
use crate::shader::{Shader, Uniforms, Varyings};

/// Contadores por cuadro para demostrar culling y LOD en el HUD.
#[derive(Clone, Copy, Default, Debug)]
pub struct Stats {
    pub objects_drawn: u32,
    pub objects_culled: u32,
    /// Triángulos que entraron al pipeline (después del culling de objetos).
    pub tris_in: u32,
    /// Triángulos descartados por estar fuera del frustum.
    pub tris_frustum: u32,
    /// Triángulos descartados por mirar hacia atrás.
    pub tris_backface: u32,
    /// Triángulos que llegaron a rasterizarse.
    pub tris_raster: u32,
    pub pixels_shaded: u32,
}

#[derive(Clone, Copy)]
pub struct DrawOptions {
    /// Si es true no se descartan las caras traseras (anillos, hojas…).
    pub double_sided: bool,
    /// Si es false, el objeto no escribe en el z-buffer (halos transparentes).
    pub depth_write: bool,
    /// Mezcla aditiva con lo que ya hay en el framebuffer (brillos).
    pub additive: bool,
}

impl Default for DrawOptions {
    fn default() -> Self {
        Self { double_sided: false, depth_write: true, additive: false }
    }
}

/// Vértice ya transformado a clip space, con sus varyings.
#[derive(Clone, Copy, Default)]
struct ClipVertex {
    clip: Vec4,
    var: Varyings,
}

/// Vértice en pantalla listo para rasterizar.
#[derive(Clone, Copy)]
struct ScreenVertex {
    x: f32,
    y: f32,
    /// Profundidad en [0, 1].
    z: f32,
    /// 1 / w_clip, para la interpolación con corrección de perspectiva.
    inv_w: f32,
    var: Varyings,
}

pub struct Renderer {
    pub fb: Framebuffer,
    pub stats: Stats,
    pub view: Mat4,
    pub proj: Mat4,
    view_proj: Mat4,
    /// Buffer reutilizable para los vértices transformados de un mesh.
    vcache: Vec<ClipVertex>,
}

impl Renderer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            fb: Framebuffer::new(width, height),
            stats: Stats::default(),
            view: Mat4::IDENTITY,
            proj: Mat4::IDENTITY,
            view_proj: Mat4::IDENTITY,
            vcache: Vec::new(),
        }
    }

    pub fn begin_frame(&mut self, view: Mat4, proj: Mat4, clear: Vec3) {
        self.view = view;
        self.proj = proj;
        self.view_proj = proj * view;
        self.stats = Stats::default();
        self.fb.clear(clear);
    }

    pub fn view_proj(&self) -> Mat4 {
        self.view_proj
    }

    /// Dibuja un mesh con su matriz de modelo y un fragment shader.
    pub fn draw_mesh(
        &mut self,
        mesh: &Mesh,
        model: &Mat4,
        shader: &dyn Shader,
        uniforms: &Uniforms,
        opts: DrawOptions,
    ) {
        self.stats.objects_drawn += 1;
        let mvp = self.view_proj * *model;

        // --- 1. Vertex stage -------------------------------------------------
        // Se transforma cada vértice una sola vez aunque lo compartan varios
        // triángulos (los meshes son indexados).
        let mut vcache = std::mem::take(&mut self.vcache);
        vcache.clear();
        vcache.reserve(mesh.positions.len());
        for i in 0..mesh.positions.len() {
            let p = mesh.positions[i];
            vcache.push(ClipVertex {
                clip: mvp.mul_vec4(Vec4::from_point(p)),
                var: Varyings {
                    world_pos: model.transform_point(p),
                    // Válido para escalas uniformes (todo lo que usamos).
                    normal: model.transform_dir(mesh.normals[i]).normalize(),
                    local_pos: p,
                    uv: mesh.uvs[i],
                    color: mesh.colors[i],
                    emissive: mesh.emissive[i],
                },
            });
        }

        // --- 2..7 por triángulo ---------------------------------------------
        for tri in &mesh.indices {
            self.stats.tris_in += 1;
            let a = vcache[tri[0] as usize];
            let b = vcache[tri[1] as usize];
            let c = vcache[tri[2] as usize];

            if outside_same_plane(&a.clip, &b.clip, &c.clip) {
                self.stats.tris_frustum += 1;
                continue;
            }

            // 3. Recorte contra el plano cercano.
            let mut poly = [ClipVertex::default(); 4];
            let n = clip_near(&[a, b, c], &mut poly);
            if n < 3 {
                self.stats.tris_frustum += 1;
                continue;
            }

            // 4. Perspectiva + viewport.
            let mut sv = [None; 4];
            for i in 0..n {
                sv[i] = Some(self.to_screen(&poly[i]));
            }

            // El polígono recortado (3 o 4 vértices) se triangula en abanico.
            let mut any_front = false;
            for k in 1..n - 1 {
                let (v0, v1, v2) = (sv[0].unwrap(), sv[k].unwrap(), sv[k + 1].unwrap());
                if self.raster_triangle(v0, v1, v2, shader, uniforms, opts) {
                    any_front = true;
                }
            }
            if any_front {
                self.stats.tris_raster += 1;
            } else {
                self.stats.tris_backface += 1;
            }
        }

        self.vcache = vcache;
    }

    fn to_screen(&self, v: &ClipVertex) -> ScreenVertex {
        let inv_w = 1.0 / v.clip.w;
        let ndc = v.clip.xyz() * inv_w;
        let w = self.fb.width as f32;
        let h = self.fb.height as f32;
        ScreenVertex {
            x: (ndc.x + 1.0) * 0.5 * w,
            // En pantalla Y crece hacia abajo.
            y: (1.0 - ndc.y) * 0.5 * h,
            z: ndc.z * 0.5 + 0.5,
            inv_w,
            var: v.var,
        }
    }

    /// Rasteriza un triángulo en pantalla. Devuelve false si se descartó por
    /// backface culling.
    fn raster_triangle(
        &mut self,
        mut v0: ScreenVertex,
        mut v1: ScreenVertex,
        mut v2: ScreenVertex,
        shader: &dyn Shader,
        u: &Uniforms,
        opts: DrawOptions,
    ) -> bool {
        // --- 5. Backface culling ---------------------------------------------
        // Un triángulo CCW en NDC queda en sentido horario en pantalla (porque
        // Y se invierte), así que su área con signo es negativa.
        let mut area = edge(v0.x, v0.y, v1.x, v1.y, v2.x, v2.y);
        if area >= 0.0 {
            if !opts.double_sided || area == 0.0 {
                return false;
            }
            // Cara trasera de un objeto de dos caras: se invierte la normal
            // para que la iluminación sea correcta desde este lado.
            for v in [&mut v0, &mut v1, &mut v2] {
                v.var.normal = -v.var.normal;
            }
        } else {
            // Se reordena para trabajar siempre con área positiva.
            std::mem::swap(&mut v1, &mut v2);
            area = -area;
        }
        let inv_area = 1.0 / area;

        // --- 6. Caja envolvente recortada a la pantalla ----------------------
        let w = self.fb.width as i32;
        let h = self.fb.height as i32;
        let min_x = (v0.x.min(v1.x).min(v2.x).floor() as i32).max(0);
        let max_x = (v0.x.max(v1.x).max(v2.x).ceil() as i32).min(w - 1);
        let min_y = (v0.y.min(v1.y).min(v2.y).floor() as i32).max(0);
        let max_y = (v0.y.max(v1.y).max(v2.y).ceil() as i32).min(h - 1);
        if min_x > max_x || min_y > max_y {
            return true;
        }

        // Funciones de arista: E(p) = (b-a) x (p-a). Son lineales en x e y,
        // así que se avanzan sumando constantes en lugar de recalcularlas.
        let e0_dx = -(v2.y - v1.y);
        let e0_dy = v2.x - v1.x;
        let e1_dx = -(v0.y - v2.y);
        let e1_dy = v0.x - v2.x;
        let e2_dx = -(v1.y - v0.y);
        let e2_dy = v1.x - v0.x;

        let px0 = min_x as f32 + 0.5;
        let py0 = min_y as f32 + 0.5;
        let mut row_w0 = edge(v1.x, v1.y, v2.x, v2.y, px0, py0);
        let mut row_w1 = edge(v2.x, v2.y, v0.x, v0.y, px0, py0);
        let mut row_w2 = edge(v0.x, v0.y, v1.x, v1.y, px0, py0);

        // Varyings pre-multiplicados por 1/w (corrección de perspectiva).
        let a = scale_var(&v0.var, v0.inv_w);
        let b = scale_var(&v1.var, v1.inv_w);
        let c = scale_var(&v2.var, v2.inv_w);

        let fb_w = self.fb.width;
        for y in min_y..=max_y {
            let (mut w0, mut w1, mut w2) = (row_w0, row_w1, row_w2);
            for x in min_x..=max_x {
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    let b0 = w0 * inv_area;
                    let b1 = w1 * inv_area;
                    let b2 = w2 * inv_area;
                    // z es lineal en pantalla (ya está dividido por w).
                    let z = b0 * v0.z + b1 * v1.z + b2 * v2.z;
                    let idx = y as usize * fb_w + x as usize;
                    if (0.0..=1.0).contains(&z) && z < self.fb.depth[idx] {
                        // Corrección de perspectiva: interpolar var/w y 1/w,
                        // y dividir.
                        let one_over_w = b0 * v0.inv_w + b1 * v1.inv_w + b2 * v2.inv_w;
                        let k = 1.0 / one_over_w;
                        let var = Varyings::blend3(&a, &b, &c, b0 * k, b1 * k, b2 * k);

                        // --- 7. Fragment shader --------------------------------
                        let mut col = shader.fragment(&var, u);
                        if opts.additive {
                            col = col + self.fb.get_pixel(idx);
                        }
                        self.fb.set_pixel(idx, col);
                        if opts.depth_write {
                            self.fb.depth[idx] = z;
                        }
                        self.stats.pixels_shaded += 1;
                    }
                }
                w0 += e0_dx;
                w1 += e1_dx;
                w2 += e2_dx;
            }
            row_w0 += e0_dy;
            row_w1 += e1_dy;
            row_w2 += e2_dy;
        }
        true
    }
}

/// Área con signo (x2) del triángulo (a, b, p).
#[inline]
fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

#[inline]
fn scale_var(v: &Varyings, s: f32) -> Varyings {
    Varyings::blend3(v, v, v, s, 0.0, 0.0)
}

/// Rechazo trivial: true si los tres vértices están fuera del mismo plano
/// del volumen de visión (-w <= x, y, z <= w).
fn outside_same_plane(a: &Vec4, b: &Vec4, c: &Vec4) -> bool {
    (a.x > a.w && b.x > b.w && c.x > c.w)
        || (a.x < -a.w && b.x < -b.w && c.x < -c.w)
        || (a.y > a.w && b.y > b.w && c.y > c.w)
        || (a.y < -a.w && b.y < -b.w && c.y < -c.w)
        || (a.z > a.w && b.z > b.w && c.z > c.w)
        || (a.z < -a.w && b.z < -b.w && c.z < -c.w)
}

/// Recorte de Sutherland–Hodgman contra el plano cercano (z >= -w).
/// Un triángulo recortado por un plano produce como mucho 4 vértices.
fn clip_near(input: &[ClipVertex; 3], out: &mut [ClipVertex; 4]) -> usize {
    let dist = |v: &ClipVertex| v.clip.z + v.clip.w;
    let mut n = 0;
    for i in 0..3 {
        let cur = &input[i];
        let next = &input[(i + 1) % 3];
        let dc = dist(cur);
        let dn = dist(next);
        if dc >= 0.0 {
            out[n] = *cur;
            n += 1;
        }
        if (dc >= 0.0) != (dn >= 0.0) {
            let t = dc / (dc - dn);
            out[n] = ClipVertex {
                clip: cur.clip + (next.clip - cur.clip) * t,
                var: Varyings::lerp(&cur.var, &next.var, t),
            };
            n += 1;
        }
    }
    n
}
