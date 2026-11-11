# Guía para crear los mundos (Esteban y Emily)

Cada uno hace **2 planetas**. Esta guía dice qué hacer, dónde, con qué reglas
técnicas y visuales, y cómo entregarlo sin romper nada. Léanla completa antes
de empezar, junto con `README.md` y `PENDIENTES.md`.

| Persona | Planeta 1 | Planeta 2 | Archivo de trabajo | Semillas |
|---|---|---|---|---|
| Esteban | **Lava** (órbita 30) | **Desierto con anillo de asteroides** (órbita 80) | `src/mundos/esteban.rs` | `WORLD_SEED + 20`, `+ 21` |
| Emily | **Ciudad** (órbita 45) | **Hielo / océano** (órbita 120) | `src/mundos/emily.rs` | `WORLD_SEED + 30`, `+ 31` |

Ya existen (no los toquen): **Jungla jurásica** (órbita 60) y **Embrujado**
(órbita 100), de Martin. Son la referencia de calidad y de estilo.

---

## 1. Antes de empezar

```bash
git clone https://github.com/Martinho777/Proyecto3-Graficas.git
cd Proyecto3-Graficas
brew install cmake            # solo la primera vez
cargo run --release           # el juego (siempre --release)
cargo test --release          # deben pasar todos
git checkout -b mundo-esteban # o mundo-emily
```

Vuelen hasta la jungla y el embrujado, acérquense y denles la vuelta.
Su planeta tiene que sentirse **del mismo juego**.

---

## 2. Qué archivos pueden tocar

**Sí (son de ustedes):**
- `src/mundos/<nombre>.rs` — sus planetas, sus shaders y sus reglas de objetos.
  Si crece mucho, pueden crear la carpeta `src/mundos/<nombre>/` con submódulos.
- `assets/models/modelos_<nombre>.py` — su script de Blender (nuevo).
- `assets/models/<modelo>.obj/.mtl` — los modelos que genere su script.
  Prefijen los nombres para no chocar: `lava_roca.obj`, `ciudad_torre.obj`…
- `capturas/` — sus capturas para el README.
- `README.md` y `PENDIENTES.md` — **solo** sus filas (contribución, puntos).

**No (avisar a Martin antes):** `renderer.rs`, `scene.rs`, `planet.rs`,
`props.rs`, `ship.rs`, `collision.rs`, `camera.rs`, `math.rs`, `noise.rs`,
`config.rs`, `planets.rs`, `src/shaders/*`, `main.rs`. Si de verdad
necesitan algo del motor, se habla primero; casi todo se puede resolver
desde su archivo.

Así nunca habrá conflictos de merge entre ustedes.

---

## 3. Paso a paso de un planeta

Todo en `src/mundos/<nombre>.rs`. Copien la estructura de `jungle()` y
`haunted()` en `src/planets.rs`:

```rust
use crate::config::WORLD_SEED;
use crate::math::{Mat4, Vec3, smoothstep};
use crate::noise::{Perlin, worley};
use crate::planet::{Orbit, Planet, PlanetDef, Terrain, TerrainKind};
use crate::planets::model;
use crate::props::{Placement, PropSpec};
use crate::shader::{Shader, Uniforms, Varyings, lambert, rim, toon_diffuse};

pub fn planetas() -> Result<Vec<Planet>, String> {
    Ok(vec![lava()?, desierto()?])
}

// 1) Shader propio
pub struct LavaShader { pub noise: Perlin, pub amplitude: f32 /* , … */ }
impl Shader for LavaShader {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 { /* … */ }
}

// 2) Planeta: terreno + órbita + shader + objetos
fn lava() -> Result<Planet, String> {
    let terrain = Terrain { kind: TerrainKind::Ridged, amplitude: 0.12, frequency: 2.0, octaves: 5, sea_level: None };
    let def = PlanetDef {
        name: "Lava",
        radius: 6.0,
        orbit: Orbit { radius: 30.0, speed: 0.04, phase: 2.5, inclination: 0.05 },
        spin_speed: 0.2,
        axial_tilt: 0.1,
        seed: WORLD_SEED + 20,
        terrain,
    };
    let shader = LavaShader { noise: Perlin::new(def.seed + 100), amplitude: terrain.amplitude };
    let props = vec![PropSpec { name: "Roca", mesh: model("lava_roca")?, /* … */ }];
    Ok(Planet::new(def, Box::new(shader)).with_props(props))
}
```

La órbita, el giro, la colisión, el LOD, el culling, el HUD y los objetos
funcionan solos. No hay que tocar nada más.

Para ver su planeta de cerca sin abrir la ventana (N = posición en la lista:
0 jungla, 1 embrujado, 2 y 3 los de Esteban, 4 y 5 los de Emily):

```bash
cargo run --release -- --snapshot 10 out.ppm cerca --planeta 2
cargo run --release -- --snapshot 30 out.ppm rasante --planeta 2 --bench
sips -s format png out.ppm --out out.png
```

---

## 4. Reglas técnicas (no negociables)

1. **Nada de 3D de raylib** (`draw_model`, `Camera3D`, shaders GPU…). Todo
   pasa por nuestro pipeline. Si lo rompen, el proyecto vale 0.
2. **Animación con `u.time`**, nunca contando cuadros.
3. **Shaders sin memoria dinámica** (`Vec`, `String`, `Box`) dentro de
   `fragment`: corre ~100 000 veces por cuadro en varios hilos.
4. **Reproducible:** todo lo aleatorio sale de `Rng::new(semilla)` o del
   ruido con semilla. Nada de `rand` ni tiempo del sistema.
5. **Presupuesto de rendimiento** (Mac M4, medir con `--bench`):
   - Vista `cerca` y `rasante` de su planeta: **≤ 10 ms** promedio.
   - Objetos por planeta: **≤ 60 000 triángulos** en total (el HUD con F1 y
     el snapshot dicen cuántos). Modelos de 50 a 400 triángulos.
   - En un shader: como máximo **una** llamada a `worley` y ~6 llamadas a
     `fbm` por pixel. El ruido es lo más caro.
6. `cargo test --release` pasa y `cargo build --release` no da errores.
7. Comentarios y nombres **en español**, explicando el *por qué* de la
   técnica (como en `jungle.rs`): en la demo tienen que explicarlo.

---

## 5. Dirección artística (para que se vea del mismo juego)

- **Mini-mundos estilo caricatura.** Colores saturados, nada realista. Un
  tema que se entienda **en 2 segundos**: de lejos por la paleta, de cerca
  por las siluetas de los objetos.
- **Iluminación toon:** usen `toon_diffuse(lambert(...), ambiente)` (dos
  escalones, no degradado continuo). Ambiente entre 0.12 y 0.25: el lado de
  noche no debe quedar negro puro.
- **Atmósfera:** borde con `rim(...)` del color del tema (la jungla es verde,
  el embrujado verde espectral). Una sola por planeta, sutil.
- **Algo emisivo** que se lea de noche (lava, ventanas, cristales, farolitos).
- **El color depende de la altura real** del terreno
  (`h = (v.local_pos.length() - 1.0) / amplitude`), así coincide con el
  relieve y con dónde caen los objetos.
- **Resolución 480×270:** detalles de al menos 3–4 pixeles. Ruido muy fino
  parpadea y se ve sucio; mejor manchas grandes y bordes limpios.
- **Objetos de superficie:** al menos 2 tipos por planeta, low-poly, colores
  planos por material (`Kd` en el `.mtl`), sin texturas. Se dibujan con el
  mismo toon shader de la nave, así que combinan solos. Uno puede ser un
  "monumento" único con `Placement::Highest` (como la casona).
- **Tamaños:** radio del planeta 5–9; la nave mide 1. Objetos de 0.5 a 3
  unidades (escala Mario Galaxy: grandes respecto al planeta).

### Ideas que no chocan con lo que ya existe

| Planeta | Terreno | Shader (técnica distinta) | Objetos |
|---|---|---|---|
| Lava | `Ridged`, amplitud ~0.12 | Placas de roca negra separadas por grietas brillantes (`worley`: borde `f2 - f1` = grieta), lava que pulsa | Rocas volcánicas, columnas de basalto, cristales de obsidiana |
| Desierto | `Smooth`, frecuencia alta, amplitud baja | Dunas con bandas (`sin` sobre una dirección + ruido), estratos rojizos | Cactus, arcos de piedra, huesos/ruinas; **anillo de asteroides** (ver abajo) |
| Ciudad | casi plano (amplitud ~0.03) | Cuadrícula de calles sobre la esfera (`fract` de coordenadas), ventanas que se encienden en el lado nocturno (`lambert < 0.1`) | Edificios de varias alturas, torres, antenas con luz |
| Hielo/océano | `Smooth`, `sea_level` alto | Océano con olas animadas y especular; casquetes por latitud (`local_pos.y`) | Cristales de hielo, icebergs, pingüino/iglú propio |

Ojo: la jungla ya tiene volcanes, así que la lava de Esteban tiene que
verse distinta (placas y grietas, no conos verdes con lava en la punta).

### Anillo de asteroides (Esteban)

El anillo puede ir dentro de la malla de objetos del planeta (está en
espacio local, radio 1 = radio del planeta), después de `with_props`:

```rust
use crate::props::append_transformed;
let mut p = Planet::new(def, Box::new(shader)).with_props(props);
let roca = model("desierto_asteroide")?;
let mut rng = crate::noise::Rng::new(def.seed + 7);
if let Some(b) = p.props.as_mut() {
    for _ in 0..120 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(1.6, 2.2);                     // en radios del planeta
        let pos = Vec3::new(a.cos() * r, rng.range(-0.03, 0.03), a.sin() * r);
        let m = Mat4::translation(pos) * Mat4::rotation_y(a) * Mat4::uniform_scale(rng.range(0.03, 0.08));
        append_transformed(&mut b.mesh, &roca, &m);
    }
    b.mesh.compute_bounding_radius();
    p.bounding_radius = p.bounding_radius.max(b.mesh.bounding_radius * p.def.radius);
}
```

---

## 6. Modelos (Blender por script)

Copien la estructura de `assets/models/modelos_superficie.py` (clase
`Modelo`, helpers `caja`, `esfera`, `cono`, `tubo`, `hoja`) a su propio
script `assets/models/modelos_<nombre>.py`. Reglas:

- Base del modelo en `z = 0`, arriba = +Z, frente = +Y (el exportador lo
  convierte al sistema del juego).
- Medidas en unidades del mundo (la nave mide 1).
- Materiales con color plano; `emisivo=True` para lo que brilla.
- Exportar con los mismos parámetros que el script de referencia.

```bash
/Applications/Blender.app/Contents/MacOS/Blender -b -P assets/models/modelos_esteban.py
```

---

## 7. Antes de abrir el Pull Request (checklist)

- [ ] `cargo test --release` pasa.
- [ ] Volaron en la ventana alrededor de sus 2 planetas: se acercan, dan
      la vuelta, chocan con el suelo, se alejan. Nada truena.
- [ ] `--bench` en `cerca` y `rasante` de cada planeta: ≤ 10 ms.
- [ ] El tema se entiende sin explicarlo (enséñenle una captura a alguien).
- [ ] Capturas en `capturas/` y su fila en el README (integrantes y
      contribución), y en la tabla de `PENDIENTES.md`.
- [ ] Commits con **su** nombre y correo, mensajes en español con el
      estilo del repo (título corto + qué y por qué).
- [ ] PR de `mundo-<nombre>` hacia `main`. Martin revisa y hace merge.

En la demo cada quien explica **sus** planetas: el terreno (parámetros de
`Terrain`), la técnica del shader y las reglas de distribución de objetos.
El profesor puede pedir cambiar el radio, la órbita, la semilla o la
intensidad de un shader en vivo: tengan esos valores a la mano en su archivo.

---

## 8. Prompt para su asistente de IA

Si usan IA, pásenle esto tal cual al empezar (y declaren su uso en el README):

> Estoy trabajando en un proyecto universitario de gráficas en Rust: un
> software renderer propio (raylib solo para ventana/entrada/framebuffer).
> Antes de escribir código, **lee y entiende el proyecto completo**:
> `README.md`, `PENDIENTES.md`, `GUIA_MUNDOS.md`, `src/planets.rs`,
> `src/planet.rs`, `src/props.rs`, `src/scene.rs`, `src/shader.rs`,
> `src/noise.rs`, `src/shaders/jungle.rs`, `src/shaders/haunted.rs` y
> `assets/models/modelos_superficie.py`. Mi tarea es crear 2 planetas en
> `src/mundos/<mi_nombre>.rs` (y mis modelos en
> `assets/models/modelos_<mi_nombre>.py`) siguiendo **exactamente** las
> reglas técnicas y la dirección artística de `GUIA_MUNDOS.md`: mismos
> patrones de código que la jungla y el embrujado, iluminación toon con
> `toon_diffuse`, color según la altura real del terreno, comentarios en
> español que expliquen cada técnica, todo reproducible con semilla y
> animado con `u.time`. **No modifiques** archivos fuera de los míos; si
> crees que hace falta cambiar el motor, dímelo y explícame por qué en vez
> de hacerlo. Después de cada cambio corre `cargo test --release`, genera
> capturas con `--snapshot … --planeta N`, míralas y mide con `--bench`
> (≤ 10 ms de cerca). Mi planeta debe verse del mismo juego que la jungla
> y el embrujado, con un tema que se entienda a simple vista. Explícame lo
> que hagas para que yo pueda defenderlo en la demo.
