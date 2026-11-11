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
//!
//! Paralelismo: la pantalla se divide en franjas horizontales. Cada
//! triángulo se anota en las franjas que toca ("binning") y cada franja se
//! rasteriza en un hilo distinto (rayon). Como dos franjas nunca comparten
//! pixeles, no hace falta sincronizar el z-buffer, y dentro de una franja los
//! triángulos se dibujan en el mismo orden en que llegaron (resultado
//! determinista).

use rayon::prelude::*;

use crate::framebuffer::{Framebuffer, to_rgb8};
use crate::math::{Mat4, Vec3, Vec4};
use crate::mesh::Mesh;
use crate::shader::{Shader, Uniforms, Varyings};

/// Alto (en pixeles) de cada franja que se rasteriza en paralelo.
const BAND_ROWS: usize = 8;

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
#[derive(Clone, Copy, Default)]
struct ScreenVertex {
    x: f32,
    y: f32,
    /// Profundidad en [0, 1].
    z: f32,
    /// 1 / w_clip, para la interpolación con corrección de perspectiva.
    inv_w: f32,
    /// Varyings YA multiplicados por 1/w.
    var_w: Varyings,
}

/// Triángulo preparado: orientado con área positiva y con su caja en pantalla.
#[derive(Clone, Copy)]
struct ScreenTri {
    v: [ScreenVertex; 3],
    inv_area: f32,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

pub struct Renderer {
    pub fb: Framebuffer,
    pub stats: Stats,
    pub view: Mat4,
    pub proj: Mat4,
    view_proj: Mat4,
    /// Planos del frustum en mundo (a, b, c, d) normalizados, para culling.
    frustum: [Vec4; 6],
    // Buffers reutilizables entre draw calls (evitan pedir memoria cada vez).
    vcache: Vec<ClipVertex>,
    tris: Vec<ScreenTri>,
    bins: Vec<Vec<u32>>,
}

impl Renderer {
    pub fn new(width: usize, height: usize) -> Self {
        let bands = height.div_ceil(BAND_ROWS);
        Self {
            fb: Framebuffer::new(width, height),
            stats: Stats::default(),
            view: Mat4::IDENTITY,
            proj: Mat4::IDENTITY,
            view_proj: Mat4::IDENTITY,
            frustum: [Vec4::default(); 6],
            vcache: Vec::new(),
            tris: Vec::new(),
            bins: vec![Vec::new(); bands],
        }
    }

    pub fn begin_frame(&mut self, view: Mat4, proj: Mat4, clear: Vec3) {
        self.view = view;
        self.proj = proj;
        self.view_proj = proj * view;
        self.frustum = extract_frustum(&self.view_proj);
        self.stats = Stats::default();
        self.fb.clear(clear);
    }

    // -----------------------------------------------------------------------
    // Consultas para culling y LOD (las usa la escena antes de dibujar)
    // -----------------------------------------------------------------------

    /// true si una esfera (en mundo) toca el volumen de visión.
    pub fn sphere_visible(&self, center: Vec3, radius: f32) -> bool {
        self.frustum
            .iter()
            .all(|p| p.x * center.x + p.y * center.y + p.z * center.z + p.w >= -radius)
    }

    /// Radio aproximado en pixeles que ocupa una esfera en pantalla.
    /// Si la cámara está dentro o muy cerca, devuelve un valor enorme.
    pub fn projected_radius_px(&self, center: Vec3, radius: f32) -> f32 {
        let depth = -self.view.transform_point(center).z;
        if depth <= -radius {
            return 0.0; // completamente detrás de la cámara
        }
        if depth <= radius {
            return f32::INFINITY; // la cámara está encima o dentro
        }
        // Tamaño angular real de una esfera: r / sqrt(d² - r²) (de cerca una
        // esfera se ve más grande que r / d).
        let dist = self.view.transform_point(center).length();
        let tan_half = radius / (dist * dist - radius * radius).max(1e-6).sqrt();
        tan_half * self.proj.m[1][1] * self.fb.height as f32 * 0.5
    }

    // -----------------------------------------------------------------------
    // Draw call
    // -----------------------------------------------------------------------

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

        // --- 1. Vertex stage (en paralelo) -----------------------------------
        // Se transforma cada vértice una sola vez aunque lo compartan varios
        // triángulos (los meshes son indexados).
        let mut vcache = std::mem::take(&mut self.vcache);
        vcache.clear();
        (0..mesh.positions.len())
            .into_par_iter()
            .with_min_len(512)
            .map(|i| {
                let p = mesh.positions[i];
                ClipVertex {
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
                }
            })
            .collect_into_vec(&mut vcache);

        // --- 2..5 Ensamblado, recorte, proyección y backface -----------------
        let mut tris = std::mem::take(&mut self.tris);
        tris.clear();
        let (w, h) = (self.fb.width as f32, self.fb.height as f32);
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
            let mut sv = [ScreenVertex::default(); 4];
            for i in 0..n {
                sv[i] = to_screen(&poly[i], w, h);
            }

            // El polígono recortado (3 o 4 vértices) se triangula en abanico.
            let mut any_front = false;
            for k in 1..n - 1 {
                if let Some(t) = setup_triangle(sv[0], sv[k], sv[k + 1], opts.double_sided, w, h) {
                    tris.push(t);
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

        // --- 6..7 Rasterización + fragment shader (en paralelo) --------------
        if !tris.is_empty() {
            self.stats.pixels_shaded += self.rasterize(&tris, shader, uniforms, opts);
        }
        self.tris = tris;
    }

    /// Reparte los triángulos en franjas y rasteriza cada franja en un hilo.
    fn rasterize(&mut self, tris: &[ScreenTri], shader: &dyn Shader, u: &Uniforms, opts: DrawOptions) -> u32 {
        let mut bins = std::mem::take(&mut self.bins);
        for b in bins.iter_mut() {
            b.clear();
        }
        for (i, t) in tris.iter().enumerate() {
            for band in (t.min_y as usize / BAND_ROWS)..=(t.max_y as usize / BAND_ROWS) {
                bins[band].push(i as u32);
            }
        }

        let width = self.fb.width;
        let pixels = self
            .fb
            .color
            .par_chunks_mut(width * 4 * BAND_ROWS)
            .zip(self.fb.depth.par_chunks_mut(width * BAND_ROWS))
            .zip(bins.par_iter())
            .enumerate()
            .map(|(band, ((color, depth), bin))| {
                let y0 = (band * BAND_ROWS) as i32;
                let mut count = 0;
                for &ti in bin {
                    count += raster_band(&tris[ti as usize], y0, color, depth, width, shader, u, opts);
                }
                count
            })
            .sum();

        self.bins = bins;
        pixels
    }
}

/// Rasteriza la parte de un triángulo que cae dentro de una franja.
/// `color` y `depth` son solo las filas de esa franja (empiezan en `y0`).
/// Devuelve cuántos pixeles pasaron la prueba de profundidad.
#[allow(clippy::too_many_arguments)]
fn raster_band(
    t: &ScreenTri,
    y0: i32,
    color: &mut [u8],
    depth: &mut [f32],
    width: usize,
    shader: &dyn Shader,
    u: &Uniforms,
    opts: DrawOptions,
) -> u32 {
    let rows = (depth.len() / width) as i32;
    let y_start = t.min_y.max(y0);
    let y_end = t.max_y.min(y0 + rows - 1);
    if y_start > y_end {
        return 0;
    }
    let [v0, v1, v2] = &t.v;

    // Funciones de arista: E(p) = (b-a) x (p-a). Son lineales en x e y, así
    // que se avanzan sumando constantes en lugar de recalcularlas.
    let e0_dx = -(v2.y - v1.y);
    let e0_dy = v2.x - v1.x;
    let e1_dx = -(v0.y - v2.y);
    let e1_dy = v0.x - v2.x;
    let e2_dx = -(v1.y - v0.y);
    let e2_dy = v1.x - v0.x;

    let px0 = t.min_x as f32 + 0.5;
    let py0 = y_start as f32 + 0.5;
    let mut row_w0 = edge(v1.x, v1.y, v2.x, v2.y, px0, py0);
    let mut row_w1 = edge(v2.x, v2.y, v0.x, v0.y, px0, py0);
    let mut row_w2 = edge(v0.x, v0.y, v1.x, v1.y, px0, py0);

    let mut count = 0;
    for y in y_start..=y_end {
        let (mut w0, mut w1, mut w2) = (row_w0, row_w1, row_w2);
        let row = (y - y0) as usize * width;
        for x in t.min_x..=t.max_x {
            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                let b0 = w0 * t.inv_area;
                let b1 = w1 * t.inv_area;
                let b2 = w2 * t.inv_area;
                // z es lineal en pantalla (ya está dividido por w).
                let z = b0 * v0.z + b1 * v1.z + b2 * v2.z;
                let idx = row + x as usize;
                if (0.0..=1.0).contains(&z) && z < depth[idx] {
                    // Corrección de perspectiva: se interpolan var/w y 1/w,
                    // y se divide.
                    let k = 1.0 / (b0 * v0.inv_w + b1 * v1.inv_w + b2 * v2.inv_w);
                    let var = Varyings::blend3(&v0.var_w, &v1.var_w, &v2.var_w, b0 * k, b1 * k, b2 * k);

                    // --- 7. Fragment shader -----------------------------------
                    let mut col = shader.fragment(&var, u);
                    let ci = idx * 4;
                    if opts.additive {
                        col = col
                            + Vec3::new(color[ci] as f32, color[ci + 1] as f32, color[ci + 2] as f32) / 255.0;
                    }
                    let [r, g, b] = to_rgb8(col);
                    color[ci] = r;
                    color[ci + 1] = g;
                    color[ci + 2] = b;
                    if opts.depth_write {
                        depth[idx] = z;
                    }
                    count += 1;
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
    count
}

fn to_screen(v: &ClipVertex, w: f32, h: f32) -> ScreenVertex {
    let inv_w = 1.0 / v.clip.w;
    let ndc = v.clip.xyz() * inv_w;
    ScreenVertex {
        x: (ndc.x + 1.0) * 0.5 * w,
        // En pantalla Y crece hacia abajo.
        y: (1.0 - ndc.y) * 0.5 * h,
        z: ndc.z * 0.5 + 0.5,
        inv_w,
        var_w: v.var,
    }
}

/// Backface culling + preparación del triángulo. None si se descarta.
fn setup_triangle(
    v0: ScreenVertex,
    mut v1: ScreenVertex,
    mut v2: ScreenVertex,
    double_sided: bool,
    w: f32,
    h: f32,
) -> Option<ScreenTri> {
    let mut v = [v0, v1, v2];
    // --- 5. Backface culling -------------------------------------------------
    // Un triángulo CCW en NDC queda en sentido horario en pantalla (porque Y
    // se invierte), así que su área con signo es negativa.
    let mut area = edge(v0.x, v0.y, v1.x, v1.y, v2.x, v2.y);
    if area == 0.0 {
        return None;
    }
    if area > 0.0 {
        if !double_sided {
            return None;
        }
        // Cara trasera de un objeto de dos caras: se invierte la normal para
        // que la iluminación sea correcta desde este lado.
        for sv in v.iter_mut() {
            sv.var_w.normal = -sv.var_w.normal;
        }
    } else {
        // Se reordena para trabajar siempre con área positiva.
        v.swap(1, 2);
        area = -area;
    }
    (v1, v2) = (v[1], v[2]);
    let v0 = v[0];

    // Caja envolvente recortada a la pantalla.
    let min_x = (v0.x.min(v1.x).min(v2.x).floor() as i32).max(0);
    let max_x = (v0.x.max(v1.x).max(v2.x).ceil() as i32).min(w as i32 - 1);
    let min_y = (v0.y.min(v1.y).min(v2.y).floor() as i32).max(0);
    let max_y = (v0.y.max(v1.y).max(v2.y).ceil() as i32).min(h as i32 - 1);
    if min_x > max_x || min_y > max_y {
        return None;
    }

    // Varyings pre-multiplicados por 1/w (corrección de perspectiva).
    for sv in v.iter_mut() {
        sv.var_w = Varyings::blend3(&sv.var_w, &sv.var_w, &sv.var_w, sv.inv_w, 0.0, 0.0);
    }
    Some(ScreenTri { v, inv_area: 1.0 / area, min_x, max_x, min_y, max_y })
}

/// Área con signo (x2) del triángulo (a, b, p).
#[inline]
fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
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

/// Extrae los 6 planos del frustum de la matriz View*Projection
/// (método de Gribb–Hartmann). Cada plano apunta hacia dentro del volumen.
fn extract_frustum(m: &Mat4) -> [Vec4; 6] {
    let row = |i: usize| Vec4::new(m.m[i][0], m.m[i][1], m.m[i][2], m.m[i][3]);
    let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
    let planes = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 + r2, r3 - r2];
    planes.map(|p| {
        let len = p.xyz().length();
        p * (1.0 / len)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frustum_culls_sphere_behind_camera() {
        let mut r = Renderer::new(64, 36);
        let view = Mat4::look_at(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
        let proj = Mat4::perspective(1.0, 16.0 / 9.0, 0.1, 100.0);
        r.begin_frame(view, proj, Vec3::ZERO);
        assert!(r.sphere_visible(Vec3::new(0.0, 0.0, -10.0), 1.0));
        assert!(!r.sphere_visible(Vec3::new(0.0, 0.0, 10.0), 1.0));
        assert!(!r.sphere_visible(Vec3::new(100.0, 0.0, -10.0), 1.0));
        // Toca el borde: visible.
        assert!(r.sphere_visible(Vec3::new(0.0, 0.0, 1.0), 1.5));
    }
}
