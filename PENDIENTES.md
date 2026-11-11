# Pendientes del Proyecto 3 (para Esteban y Emily)

> **Para crear sus mundos lean primero [GUIA_MUNDOS.md](GUIA_MUNDOS.md)**:
> reparto, archivos propios, reglas técnicas y visuales, y el prompt para
> su IA.

Este archivo dice qué falta, cuántos puntos da y **cómo hacerlo con el código
que ya existe**. La rúbrica completa está en `PROYECTO_FINAL GRAFICAS 2026.pdf`.

> Regla de oro de la rúbrica: todo el 3D es nuestro. raylib solo abre la
> ventana, lee teclado/mouse, muestra el framebuffer y dibuja texto 2D.
> **No usen** `draw_model`, `Camera3D`, `draw_sphere`, shaders GPU, etc.

---

## 1. Cómo correr y probar

Requisitos: Rust (stable) y `cmake` (`brew install cmake` en macOS).

```bash
cargo run --release                      # el juego
cargo test --release                     # 16 tests (matemática, física, colisión, LOD…)
cargo run --release -- --snapshot 10 out.ppm cerca   # captura sin ventana
```

Modos de captura: `lejos`, `cerca`, `rasante`, `cabina`, `nave`, `choque`.
Opciones: `--sin-culling`, `--sin-lod` (para comparar contadores),
`--planeta N` (a qué planeta acercarse), `--bench` (repite el cuadro 60 veces
y muestra el tiempo promedio y el peor). **Midan su planeta con `--bench`**:
de cerca debería quedar por debajo de ~10 ms en una Mac M-series.
`sips -s format png out.ppm --out out.png` la convierte a PNG en macOS.

**Siempre en `--release`**: en debug el renderer es ~20x más lento.

---

## 2. Puntos: hecho vs. pendiente

| Característica | Pts | Estado | Dónde |
|---|---|---|---|
| Planeta 1: Jungla jurásica | 10 | ✅ | `planets.rs`, `shaders/jungle.rs` |
| Planeta 2: Embrujado | 10 | ✅ | `planets.rs`, `shaders/haunted.rs` |
| Planetas 3–4: Lava y Desierto (Esteban) | 20 | ✅ | `src/mundos/esteban.rs` |
| Planetas 5–6: Ciudad y Hielo (Emily) | 10 (máx. 5 planetas puntúan) | ✅ | `src/mundos/emily.rs`, `assets/models/modelos_emily.py` |
| 4 shaders distintos (ya hay 6: sol, jungla, embrujado, toon, flama, fantasma) | 24 | ✅ | `src/shaders/` |
| Shader animado (sol, flama, lava de la jungla, niebla del embrujado) | 8 | ✅ | `sun.rs`, `thruster.rs`, `jungle.rs`, `haunted.rs` |
| Deformación del planeta con ruido | 10 | ✅ | `planet.rs` → `build_terrain_mesh` |
| Generación/subdivisión de geometría (icosfera) | 10 | ✅ | `mesh.rs` → `icosphere` |
| Normal mapping | 8 | 🟡 normal map procedural en las olas del Hielo (campo `olas`, 0 = apagado); falta la tecla para alternarlo, que requiere tocar `main.rs` (Martin) | `src/mundos/emily.rs` → `HieloShader::mar`, ver §6 |
| Tipos de modelo sobre planetas (4 × 5) — ya hay 7: palmera, helecho, esqueleto de dino, árbol seco, lápida, casona, fantasma | 20 | ✅ | `assets/models/modelos_superficie.py` |
| Distribución procedural con semilla | 10 | ✅ | `src/props.rs` → `place` (tests `same_seed_same_positions…`) |
| Posición/orientación sobre superficie curva | 5 | ✅ | `src/props.rs` → `instance_matrix` |
| Aceleración/frenado/giro progresivos (FPS-independiente) | 8 | ✅ | `ship.rs` (test `same_result_at_30_and_144_fps`) |
| Colisión con el planeta | 4 | ✅ | `collision.rs` (usa la altura real del terreno) |
| Modo de control local para rodear un planeta | 8 | ⬜ | ver §5 |
| Gravedad que modifica la trayectoria | 10 | ⬜ | ver §5 |
| Culling de objetos fuera de la vista | 7 | ✅ | `scene.rs` → `draw_body`, tecla F3 |
| LOD por distancia | 8 | ✅ | `scene.rs` → `select_lod`, tecla F4 |
| Coherencia visual + claridad (presentación) | 10 | 🟡 | se cierra con el video y el README |

**Hecho: 134 puntos técnicos** (+ hasta 10 de presentación). Ya pasamos de
100: lo que falta es margen por si el profesor da fracciones de algún punto,
y para que cada quien tenga su contribución que explicar. Ojo: la rúbrica paga como máximo 5 planetas
(50 pts); el sexto no suma, pero tampoco resta.

Comparación ya medida (para la presentación):
- LOD: vista lejana = **2 988** triángulos de entrada; con `--sin-lod` = **26 988**.
- Culling: cerca de la jungla, con el sol y el embrujado fuera de vista = **21 868**; con `--sin-culling` = **27 308**.

---

## 3. Agregar un planeta (≈ 30 min cada uno)

1. Copiar `src/shaders/jungle.rs` (o `haunted.rs`) como
   `src/shaders/lava.rs` (por ejemplo) y cambiar el efecto. Registrarlo en
   `src/shaders/mod.rs`.
2. En `src/planets.rs` hay una plantilla comentada `fn lava()`. Llenarla y
   agregarla a `solar_system()`.

Eso es todo: órbita, rotación, colisión, LOD, culling y HUD funcionan solos.

**Importante para la rúbrica:** "cambiar solo el color no cuenta". Cada
planeta necesita identidad propia y cada shader una *implementación*
distinta. Ideas que ya combinan con lo que hay:

| Planeta | Terreno (`Terrain`) | Shader (idea) |
|---|---|---|
| Lava | `Ridged`, amplitud ~0.12, sin mar | grietas brillantes con `ridged` + pulso animado (`u.time`); roca oscura con toon |
| Hielo/océano | `Smooth`, mar alto (`sea_level: Some(0.1)`) | agua con olas animadas (ruido desplazado con el tiempo) + especular; casquetes por latitud (`local_pos.y`) |
| Ciudad | casi plano (amplitud 0.03) | cuadrícula de calles (`fract` sobre coordenadas esféricas), ventanas que se encienden en el lado nocturno (`lambert < 0.1`) |
| Desierto | `Smooth`, frecuencia alta | dunas con bandas (`sin` de una coordenada + ruido); + anillo de asteroides (mesh plano `double_sided`) |

En un shader tienen a mano (`Varyings`): `world_pos`, `normal`, `local_pos`
(posición en la esfera unitaria deformada → la altura es `local_pos.length() - 1`),
`uv`, `color`. Y en `Uniforms`: `time`, `camera_pos`, `sun_pos`.
Funciones útiles en `shader.rs`: `lambert`, `blinn_phong`, `rim`, `toon_diffuse`.
En `noise.rs`: `Perlin::fbm`, `Perlin::ridged`, `worley` (ruido celular:
copas de árboles, farolitos, piedras), `hash3`.

Los dos planetas hechos usan: Jungla → órbita 60, Embrujado → órbita 100.
Elijan órbitas que no choquen (por ejemplo 30, 45, 80, 120).

Para la demo, el profesor puede pedir cambiar **radio, velocidad orbital,
semilla o intensidad de un shader**: todo está en `PlanetDef` y en los campos
de cada shader en `planets.rs`.

---

## 4. Modelos sobre la superficie (ya funciona: úsenlo en sus planetas)

**Ya está implementado** en `src/props.rs` y lo usan la Jungla y el
Embrujado (ver `planets.rs`). Para su planeta solo tienen que:

1. Modelar sus objetos agregando una sección a
   `assets/models/modelos_superficie.py` (o un script propio) y correrlo.
2. Agregar `.with_props(vec![PropSpec { .. }])` a su planeta en `planets.rs`
   con las reglas: cuántos, escala, rango de altura, ruido de densidad,
   pendiente máxima. `Placement::Highest` pone UNO en la cima (como la casona).
3. Opcional: `.with_floaters(FloaterSpec { .. })` para cosas que vuelan
   (como los fantasmas).

Todo se hornea en una sola malla por planeta y solo se dibuja de cerca
(`PROPS_MIN_PX` en `config.rs`). Midan con `--bench` después de agregarlos.

Lo que sigue explica cómo funciona por dentro (para poder explicarlo en la
demo):

**Modelos (5 pts por tipo, máx. 4):** árbol, edificio, roca/arco, cristal…
Deben ser hechos por nosotros. Lo más rápido: un script de Blender como
`assets/models/nave_cohete.py` (se ejecuta con
`/Applications/Blender.app/Contents/MacOS/Blender -b -P assets/models/arbol.py`)
que exporte `.obj` + `.mtl`. Los colores salen del `Kd` de cada material y
`mesh::load_obj` ya los lee. Mantenerlos chicos (100–600 triángulos).
Se pueden dibujar con el `ToonShader` que ya existe.

**Distribución procedural con semilla (10 pts):** NO poner posiciones a mano.
Ejemplo:

```rust
let mut rng = Rng::new(planet.def.seed);          // misma semilla → mismas posiciones
for _ in 0..N {
    // dirección uniforme en la esfera
    let z = rng.range(-1.0, 1.0);
    let a = rng.range(0.0, std::f32::consts::TAU);
    let r = (1.0 - z * z).sqrt();
    let dir = Vec3::new(r * a.cos(), z, r * a.sin());
    // filtrar con ruido (ej.: bosques solo donde fbm > 0.1 y no en el agua)
    if planet.noise.fbm(dir * 3.0, 3) < 0.1 { continue; }
    instancias.push(dir);
}
```

**Posición y orientación sobre la superficie curva (5 pts):** guardar cada
instancia en espacio LOCAL del planeta y multiplicar por la matriz del planeta
en cada cuadro, así rotan con él:

```rust
let p = planet.surface_point_local(dir);            // punto sobre el terreno (radio ~1)
let up = dir.normalize();                           // "arriba" = normal de la esfera
let right = up.cross(Vec3::Y).normalize();          // cuidado si up ≈ Y: usar Vec3::X
let fwd = right.cross(up);
let local = Mat4::translation(p)
          * Mat4::from_basis(right, up, fwd)
          * Mat4::uniform_scale(escala / planet.def.radius); // el planeta ya escala por su radio
let model = planet.model(time) * local;
renderer.draw_mesh(&arbol, &model, &toon, &uniforms, DrawOptions::default());
```

Para rendimiento: aplicar `renderer.sphere_visible(...)` a cada instancia (o al
planeta entero) antes de dibujarla, igual que `Scene::draw_body`.

---

## 5. Navegación que falta (18 pts)

Todo va en `src/ship.rs` → `Ship::step` (corre a 120 Hz fijos; usar `dt` de
ahí para que siga siendo independiente de los FPS).

**Gravedad (10 pts):** `step` ya recibe `colliders` (posición y radio de cada
cuerpo). Agregar antes de integrar la velocidad:

```rust
for c in colliders {
    let d = c.center - s.pos;
    let r2 = d.length_sq().max(1.0);
    accel += d.normalize() * (G * masa(c) / r2);   // masa ∝ radio³, G en config.rs
}
```

Que se note pero no impida volar: probar G pequeño y mostrarlo en la demo
soltando la nave cerca del planeta (cae y choca suavemente).

**Modo órbita (8 pts), tecla `O`:** al activarlo cerca de un planeta, la
nave mantiene una altura constante y A/D (o el mouse) la hacen dar la vuelta
alrededor sin cortar el vuelo. Idea: guardar el planeta objetivo y la altura;
en cada paso, proyectar la velocidad sobre el plano tangente
(`vel -= n * vel.dot(n)` con `n` = dirección desde el centro), corregir la
altura con un resorte suave, y re-orientar `up` hacia `n`. Mostrar en el HUD
"MODO ÓRBITA: Jungla". Al presionar `O` otra vez vuelve el vuelo libre.

---

## 6. Normal mapping (8 pts)

Tiene que estar en nuestro renderer. Opción sencilla y válida: mapa de
normales **procedural** en un shader (ej. dunas del desierto o roca de lava):

1. Construir la base tangente en el fragmento: `n` (normal), `t = n.cross(Y)`
   normalizado, `b = n.cross(t)`.
2. Leer la "textura" de normales: derivadas del ruido por diferencias finitas,
   `dx = h(p + e*t) - h(p - e*t)`, `dy = h(p + e*b) - h(p - e*b)`.
3. Normal perturbada: `n' = normalize(n - t*dx*k - b*dy*k)` y usarla en la
   iluminación. `k` = intensidad (parámetro para la demo).

Mostrar con una tecla que lo apague/encienda para que se vea la diferencia.

---

## 7. Entrega

- **README.md**: completar integrantes, contribuciones de cada quien, lista de
  puntos solicitados y dónde se ven (la tabla de §2 sirve), uso de IA.
- **Video**: vuelo espacial → acercamiento → vuelta alrededor de un planeta →
  alejamiento. Activar F1 un momento y mostrar F3/F4 cambiando los contadores.
- Cada integrante debe poder explicar su parte en vivo.
