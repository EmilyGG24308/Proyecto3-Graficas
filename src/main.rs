//! Proyecto 3 — Software Renderer: sistema solar explorable.
//!
//! raylib se usa SOLO para: abrir la ventana, leer teclado/mouse, subir el
//! framebuffer a una textura y dibujar el HUD 2D. Todo el 3D (transformación,
//! recorte, rasterización, profundidad y shaders) está en este proyecto.

mod camera;
mod collision;
mod config;
mod framebuffer;
mod math;
mod mesh;
mod noise;
mod planet;
mod planets;
mod renderer;
mod scene;
mod shader;
mod shaders;
mod ship;
mod starfield;

use raylib::prelude::*;

use camera::{CameraMode, FreeFly, ShipCamera};
use config::*;
use math::Vec3;
use renderer::Renderer;
use scene::{Scene, ShipView};
use ship::{Ship, ShipInput};

/// Posición inicial: afuera de la órbita más lejana, mirando hacia el sol.
fn spawn_ship(scene: &Scene) -> Ship {
    let outer = scene.planets.iter().map(|p| p.def.orbit.radius).fold(SUN_RADIUS * 3.0, f32::max);
    let start = Vec3::new(0.0, 6.0, outer + 30.0);
    Ship::new(start, Vec3::ZERO - start)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--snapshot") {
        snapshot(&args);
        return;
    }

    let mut scene = match Scene::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error cargando la escena: {e}");
            std::process::exit(1);
        }
    };

    let (mut rl, thread) = raylib::init()
        .size(WINDOW_W, WINDOW_H)
        .title("Proyecto 3 - Software Renderer")
        .resizable()
        .build();
    rl.set_target_fps(144);
    rl.disable_cursor();
    // ESC no cierra: se usa para soltar el mouse. Se sale cerrando la ventana.
    rl.set_exit_key(None);

    // Textura donde se copia el framebuffer cada cuadro.
    let img = Image::gen_image_color(RENDER_W as i32, RENDER_H as i32, Color::BLACK);
    let mut screen_tex = rl.load_texture_from_image(&thread, &img).expect("textura");
    screen_tex.set_texture_filter(&thread, TextureFilter::TEXTURE_FILTER_POINT);

    let mut renderer = Renderer::new(RENDER_W, RENDER_H);
    let mut ship = spawn_ship(&scene);
    let mut ship_cam = ShipCamera::new();
    let mut free_cam: Option<FreeFly> = None;
    // Palanca virtual del mouse (x = guiñada, y = cabeceo), en [-1, 1].
    let mut stick = (0.0f32, 0.0f32);
    let mut time = 0.0f32;
    let mut mouse_locked = true;
    let mut show_debug = true;

    while !rl.window_should_close() {
        // Delta time acotado para que una pausa larga no "teletransporte" nada.
        let dt = rl.get_frame_time().min(0.1);
        time += dt;

        // --- Entrada general -------------------------------------------------
        if rl.is_key_pressed(KeyboardKey::KEY_ESCAPE) || rl.is_key_pressed(KeyboardKey::KEY_TAB) {
            mouse_locked = !mouse_locked;
            if mouse_locked { rl.disable_cursor() } else { rl.enable_cursor() }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F1) {
            show_debug = !show_debug;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_C) {
            ship_cam.toggle();
        }
        if rl.is_key_pressed(KeyboardKey::KEY_R) {
            ship = spawn_ship(&scene);
            stick = (0.0, 0.0);
        }
        // F3 / F4: apagar culling o LOD para comparar los contadores.
        if rl.is_key_pressed(KeyboardKey::KEY_F3) {
            scene.culling_enabled = !scene.culling_enabled;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F4) {
            scene.lod_enabled = !scene.lod_enabled;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F2) {
            // Cámara libre de depuración: arranca donde está la nave.
            free_cam = match free_cam {
                Some(_) => None,
                None => {
                    let s = ship.render_state();
                    let f = s.forward;
                    Some(FreeFly { pos: s.pos - f * 3.0, yaw: f.x.atan2(-f.z), pitch: f.y.asin() })
                }
            };
        }

        let key = |k| rl.is_key_down(k);
        let axis = |pos: bool, neg: bool| (pos as i32 - neg as i32) as f32;
        let md = if mouse_locked { rl.get_mouse_delta() } else { Vector2::zero() };

        let ship_input = if let Some(fc) = free_cam.as_mut() {
            // --- Cámara libre (la nave queda flotando) ------------------------
            fc.yaw += md.x * MOUSE_SENS;
            fc.pitch = (fc.pitch - md.y * MOUSE_SENS).clamp(-1.5, 1.5);
            let speed = FREEFLY_SPEED * if key(KeyboardKey::KEY_LEFT_SHIFT) { 6.0 } else { 1.0 };
            let (f, r) = (fc.forward(), fc.right());
            let mv = f * axis(key(KeyboardKey::KEY_W), key(KeyboardKey::KEY_S))
                + r * axis(key(KeyboardKey::KEY_D), key(KeyboardKey::KEY_A))
                + Vec3::Y * axis(key(KeyboardKey::KEY_SPACE), key(KeyboardKey::KEY_LEFT_CONTROL));
            fc.pos += mv.normalize() * speed * dt;
            ShipInput::default()
        } else {
            // --- Controles de la nave ----------------------------------------
            // El mouse mueve una palanca virtual que vuelve sola al centro.
            // Así la cantidad de giro depende de cuánto se mueve el mouse,
            // no de cuántos cuadros hay.
            stick.0 += md.x * MOUSE_STICK_SENS;
            stick.1 += md.y * MOUSE_STICK_SENS;
            let len = (stick.0 * stick.0 + stick.1 * stick.1).sqrt();
            if len > 1.0 {
                stick = (stick.0 / len, stick.1 / len);
            }
            let decay = (-MOUSE_STICK_RECENTER * dt).exp();
            stick = (stick.0 * decay, stick.1 * decay);

            ShipInput {
                thrust: axis(key(KeyboardKey::KEY_W), key(KeyboardKey::KEY_S)),
                lift: axis(key(KeyboardKey::KEY_SPACE), key(KeyboardKey::KEY_LEFT_CONTROL)),
                boost: key(KeyboardKey::KEY_LEFT_SHIFT),
                pitch: (-stick.1 + axis(key(KeyboardKey::KEY_UP), key(KeyboardKey::KEY_DOWN))).clamp(-1.0, 1.0),
                yaw: (stick.0 + axis(key(KeyboardKey::KEY_RIGHT), key(KeyboardKey::KEY_LEFT))).clamp(-1.0, 1.0),
                roll: axis(key(KeyboardKey::KEY_D), key(KeyboardKey::KEY_A)),
            }
        };

        // --- Simulación ------------------------------------------------------
        let colliders = scene.colliders(time);
        ship.update(&ship_input, dt, &colliders);
        let ship_state = ship.render_state();
        let mut camera = ship_cam.update(&ship_state, ship.speed(), dt, &colliders);
        if let Some(fc) = &free_cam {
            camera = fc.camera(FOV_Y_DEG.to_radians(), NEAR, FAR);
        }

        // --- Render ----------------------------------------------------------
        let ship_view = ShipView {
            state: ship_state,
            throttle: ship.throttle,
            boosting: ship.boosting,
            visible: free_cam.is_some() || ship_cam.mode == CameraMode::ThirdPerson,
        };
        let bodies = scene.render(&mut renderer, &camera, time, Some(&ship_view));

        // --- Presentar -------------------------------------------------------
        screen_tex.update_texture(&renderer.fb.color).expect("update_texture");

        let (sw, sh) = (rl.get_screen_width() as f32, rl.get_screen_height() as f32);
        let fps = rl.get_fps();
        let stats = renderer.stats;
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        // Escalar manteniendo la proporción 16:9.
        let scale = (sw / RENDER_W as f32).min(sh / RENDER_H as f32);
        let (dw, dh) = (RENDER_W as f32 * scale, RENDER_H as f32 * scale);
        let (ox, oy) = ((sw - dw) * 0.5, (sh - dh) * 0.5);
        d.draw_texture_pro(
            &screen_tex,
            Rectangle::new(0.0, 0.0, RENDER_W as f32, RENDER_H as f32),
            Rectangle::new(ox, oy, dw, dh),
            Vector2::zero(),
            0.0,
            Color::WHITE,
        );

        // Mira y palanca virtual del mouse.
        let (cx, cy) = (sw * 0.5, sh * 0.5);
        if free_cam.is_none() {
            let r = 60.0;
            d.draw_circle_lines(cx as i32, cy as i32, r, Color::new(255, 255, 255, 50));
            d.draw_circle_v(Vector2::new(cx + stick.0 * r, cy + stick.1 * r), 4.0, Color::new(255, 220, 120, 200));
            if ship_cam.mode == CameraMode::Cockpit {
                d.draw_line(cx as i32 - 12, cy as i32, cx as i32 + 12, cy as i32, Color::new(120, 255, 220, 160));
                d.draw_line(cx as i32, cy as i32 - 12, cx as i32, cy as i32 + 12, Color::new(120, 255, 220, 160));
            }
        }

        // Panel de vuelo.
        let speed = ship.speed();
        let cam_name = if free_cam.is_some() {
            "Libre (F2)"
        } else if ship_cam.mode == CameraMode::Cockpit {
            "Cabina"
        } else {
            "Tercera persona"
        };
        // Cuerpo más cercano y altura sobre su superficie.
        let nearest = colliders
            .iter()
            .map(|c| (c, c.signed_distance(ship_state.pos)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let flight = [
            format!("Velocidad: {speed:5.1}{}", if ship.boosting { "  TURBO" } else { "" }),
            format!("Camara: {cam_name}"),
            match nearest {
                Some((c, alt)) => format!("{}: altura {:.1}", c.name, (alt - SHIP_COLLISION_RADIUS).max(0.0)),
                None => String::new(),
            },
        ];
        for (i, l) in flight.iter().enumerate() {
            d.draw_text(l, sw as i32 - 300, 18 + 24 * i as i32, 20, Color::new(255, 230, 170, 230));
        }
        if ship.contact_timer > 0.0 {
            let msg = format!("CONTACTO CON {}", ship.contact_with.to_uppercase());
            let w = d.measure_text(&msg, 28);
            let a = (255.0 * ship.contact_timer.min(1.0)) as u8;
            d.draw_text(&msg, (sw as i32 - w) / 2, (sh * 0.28) as i32, 28, Color::new(255, 110, 80, a));
        }

        if show_debug {
            let onoff = |b: bool| if b { "ON" } else { "OFF" };
            let mut lines = vec![
                format!("FPS: {fps}"),
                format!("Culling (F3): {}   LOD (F4): {}", onoff(scene.culling_enabled), onoff(scene.lod_enabled)),
                format!("Objetos: {} dibujados / {} descartados", stats.objects_drawn, stats.objects_culled),
                format!("Triangulos entrada: {}", stats.tris_in),
                format!("  fuera del frustum: {}", stats.tris_frustum),
                format!("  caras traseras:    {}", stats.tris_backface),
                format!("  rasterizados:      {}", stats.tris_raster),
                format!("Pixeles sombreados: {}", stats.pixels_shaded),
            ];
            for b in &bodies {
                lines.push(match b.lod {
                    Some(l) => format!("  {}: LOD {} ({} tris, {:.0} px)", b.name, l, b.triangles, b.pixels.min(9999.0)),
                    None => format!("  {}: fuera de vista", b.name),
                });
            }
            d.draw_rectangle(8, 8, 400, 24 + 22 * lines.len() as i32, Color::new(0, 0, 0, 150));
            for (i, l) in lines.iter().enumerate() {
                d.draw_text(l, 18, 18 + 22 * i as i32, 20, Color::RAYWHITE);
            }
        }
        d.draw_text(
            "W/S acelerar/frenar | Mouse/flechas girar | A/D rodar | Espacio/Ctrl subir/bajar | Shift turbo | C camara | R reiniciar | Tab mouse | F1 debug | F2 libre | F3 culling | F4 LOD",
            12,
            sh as i32 - 26,
            16,
            Color::new(200, 200, 220, 200),
        );
    }
}

/// Renderiza un cuadro sin abrir ventana y lo guarda como PPM. Sirve para
/// capturas del README y para probar el pipeline sin interfaz.
///
/// Uso: `cargo run --release -- --snapshot <segundos> <archivo.ppm> [lejos|cerca|rasante|cabina|nave|choque] [--sin-culling] [--sin-lod] [--planeta N]`
fn snapshot(args: &[String]) {
    let time: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10.0);
    let out = args.get(3).cloned().unwrap_or_else(|| "snapshot.ppm".into());
    let mode = args.get(4).map(String::as_str).unwrap_or("lejos");
    let mut scene = Scene::new().expect("escena");
    // Para comparar el trabajo del renderer: --sin-culling / --sin-lod.
    scene.culling_enabled = !args.iter().any(|a| a == "--sin-culling");
    scene.lod_enabled = !args.iter().any(|a| a == "--sin-lod");
    let mut renderer = Renderer::new(RENDER_W, RENDER_H);

    // Se simula la nave volando unos segundos hacia el objetivo elegido.
    // --planeta N elige a qué planeta acercarse (0 = el primero).
    let pi: usize = args
        .iter()
        .position(|a| a == "--planeta")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
        .min(scene.planets.len() - 1);
    let p0 = &scene.planets[pi];
    let planet = p0.position(time);
    let pr = p0.def.radius;
    let (start, target) = match mode {
        "cerca" | "cabina" => {
            let side = Vec3::new(0.3, 0.25, 1.0).normalize();
            (planet + side * (pr + 4.0), planet + Vec3::new(-6.0, 0.0, 0.0))
        }
        "nave" => (Vec3::new(0.0, 4.0, 80.0), Vec3::new(-20.0, 0.0, 0.0)),
        // Pegado al terreno, mirando al horizonte: se ve el relieve.
        "rasante" => {
            let up = Vec3::new(0.35, 0.6, 1.0).normalize();
            let tangent = up.cross(Vec3::Y).normalize();
            (planet + up * (pr * 1.12 + 1.0), planet + up * (pr * 1.12) + tangent * 20.0)
        }
        // Directo contra el planeta, con turbo: debe quedar rozando la superficie.
        "choque" => {
            let side = Vec3::new(0.2, 0.5, 1.0).normalize();
            (planet + side * (pr + 12.0), planet)
        }
        _ => (Vec3::new(0.0, 10.0, p0.def.orbit.radius + 25.0), Vec3::ZERO),
    };
    let mut ship = Ship::new(start, target - start);
    let input = ShipInput { thrust: 1.0, boost: mode == "choque", ..Default::default() };
    let colliders = scene.colliders(time);
    let mut cam = ShipCamera::new();
    if mode == "cabina" {
        cam.toggle();
    }
    let frames = if mode == "choque" { 150 } else { 30 };
    let mut camera = cam.update(&ship.render_state(), 0.0, 1.0 / 60.0, &colliders);
    for _ in 0..frames {
        ship.update(&input, 1.0 / 60.0, &colliders);
        camera = cam.update(&ship.render_state(), ship.speed(), 1.0 / 60.0, &colliders);
    }
    let state = ship.render_state();
    if mode == "nave" {
        // Vista lateral de cerca para ver el modelo.
        let eye = state.pos + state.right() * 1.6 + state.up * 0.5 + state.forward * 0.6;
        camera.eye = eye;
        camera.forward = (state.pos - eye).normalize();
        camera.up = state.up;
    }
    let view = ShipView {
        state,
        throttle: ship.throttle.max(0.6),
        boosting: false,
        visible: mode != "cabina",
    };

    let t0 = std::time::Instant::now();
    let bodies = scene.render(&mut renderer, &camera, time, Some(&view));
    let ms = t0.elapsed().as_secs_f32() * 1000.0;

    let mut ppm = format!("P6\n{} {}\n255\n", RENDER_W, RENDER_H).into_bytes();
    for px in renderer.fb.color.chunks_exact(4) {
        ppm.extend_from_slice(&px[..3]);
    }
    std::fs::write(&out, ppm).expect("no se pudo escribir la captura");
    let alt = colliders.iter().map(|c| c.signed_distance(state.pos)).fold(f32::INFINITY, f32::min);
    for b in &bodies {
        println!("  {}: lod {:?}, {} tris, {:.0} px", b.name, b.lod, b.triangles, b.pixels);
    }
    println!(
        "{out}: {ms:.1} ms, nave {} tris, distancia a superficie {alt:.2}, contacto '{}', stats = {:?}",
        scene.ship_triangles(),
        ship.contact_with,
        renderer.stats
    );
}
