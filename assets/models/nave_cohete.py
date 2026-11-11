"""
Nave del proyecto 3: cohete caricaturesco (diseño propio).

Se construye por código con bmesh para que el modelo sea reproducible:
  - Cuerpo por revolución (lathe) de un perfil 2D alrededor del eje de vuelo.
  - Cabina: domo (esfera achatada) + aro (toro).
  - Dos alas cortas y una aleta dorsal (polígono extruido con grosor).
  - Antena con foco en la punta.
  - Tobera del motor en la parte trasera.

Convención: en Blender la nariz apunta a +Y y el techo a +Z. Al exportar con
forward=-Z / up=Y, en el .obj la nariz queda hacia -Z (el "adelante" del
renderer) y el techo hacia +Y. Largo total ~2 unidades.

Ejecutar desde la carpeta del proyecto:
  /Applications/Blender.app/Contents/MacOS/Blender -b -P assets/models/nave_cohete.py
Genera: assets/models/nave.obj, nave.mtl, nave.blend
"""
import math
import os

import bmesh
import bpy
from mathutils import Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))

bpy.ops.wm.read_factory_settings(use_empty=True)
mesh = bpy.data.meshes.new("Nave")
nave = bpy.data.objects.new("Nave", mesh)
bpy.context.scene.collection.objects.link(nave)


def material(nombre, color, emisivo=False):
    m = bpy.data.materials.new(nombre)
    m.diffuse_color = (*color, 1.0)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*color, 1.0)
    if emisivo:
        # Se exporta como Ke en el .mtl: el renderer no lo ilumina.
        bsdf.inputs["Emission Color"].default_value = (*color, 1.0)
        bsdf.inputs["Emission Strength"].default_value = 1.0
    nave.data.materials.append(m)
    return len(nave.data.materials) - 1


CREMA = material("Crema", (0.96, 0.92, 0.82))
ROJO = material("Rojo", (0.90, 0.22, 0.16))
VIDRIO = material("Vidrio", (0.30, 0.85, 0.88))
METAL = material("Metal", (0.36, 0.38, 0.44))
FOCO = material("Foco", (1.00, 0.86, 0.25), emisivo=True)
FUEGO = material("Fuego", (1.00, 0.55, 0.15), emisivo=True)

bm = bmesh.new()


def suavizar(caras, smooth=True):
    for f in caras:
        f.smooth = smooth


# ---------------------------------------------------------------- cuerpo
def lathe(perfil, segmentos, mat_de):
    """Gira un perfil [(y, radio)] alrededor del eje Y.
    mat_de(i) da el material del anillo i (entre perfil[i] y perfil[i+1])."""
    anillos = []
    for (y, r) in perfil:
        anillo = []
        for s in range(segmentos):
            a = 2 * math.pi * s / segmentos
            # x a la derecha, z arriba: el anillo rodea el eje Y.
            anillo.append(bm.verts.new((r * math.cos(a), y, r * math.sin(a))))
        anillos.append(anillo)
    caras = []
    for i in range(len(anillos) - 1):
        a0, a1 = anillos[i], anillos[i + 1]
        for s in range(segmentos):
            s2 = (s + 1) % segmentos
            if perfil[i][1] < 1e-4:  # punta: triángulo
                f = bm.faces.new((a0[s], a1[s2], a1[s]))
            elif perfil[i + 1][1] < 1e-4:
                f = bm.faces.new((a0[s], a0[s2], a1[s]))
            else:
                f = bm.faces.new((a0[s], a0[s2], a1[s2], a1[s]))
            f.material_index = mat_de(i)
            caras.append(f)
    suavizar(caras)
    return caras


# Perfil (y, radio) de cola a nariz: trasero redondeado, panza ancha, nariz.
perfil = [
    (-0.95, 0.30),
    (-0.92, 0.38),
    (-0.80, 0.46),
    (-0.60, 0.52),
    (-0.40, 0.54),
    (-0.20, 0.53),
    (0.00, 0.50),
    (0.20, 0.46),
    (0.38, 0.40),
    (0.55, 0.32),
    (0.70, 0.23),
    (0.82, 0.14),
    (0.92, 0.06),
    (0.97, 0.0),
]


def mat_cuerpo(i):
    y0 = perfil[i][0]
    if y0 >= 0.38:
        return ROJO  # nariz
    if -0.62 <= y0 < -0.45:
        return ROJO  # franja
    return CREMA


lathe(perfil, 24, mat_cuerpo)

# Tapa trasera (aro metálico, deja ver la tobera).
lathe([(-0.95, 0.30), (-0.95, 0.0)], 24, lambda i: METAL)


# ---------------------------------------------------------------- cabina
def esfera(centro, radios, seg, anillos, mat):
    m = Matrix.Translation(Vector(centro)) @ Matrix.Diagonal((*radios, 1.0))
    r = bmesh.ops.create_uvsphere(bm, u_segments=seg, v_segments=anillos, radius=1.0, matrix=m)
    caras = list({f for v in r["verts"] for f in v.link_faces})
    for f in caras:
        f.material_index = mat
    suavizar(caras)
    return caras


def toro(centro, R, r, seg_mayor, seg_menor, mat, normal_z=True):
    vs = []
    for i in range(seg_mayor):
        a = 2 * math.pi * i / seg_mayor
        fila = []
        for j in range(seg_menor):
            b = 2 * math.pi * j / seg_menor
            x = (R + r * math.cos(b)) * math.cos(a)
            y = (R + r * math.cos(b)) * math.sin(a)
            z = r * math.sin(b)
            fila.append(bm.verts.new(Vector(centro) + Vector((x, y, z))))
        vs.append(fila)
    caras = []
    for i in range(seg_mayor):
        for j in range(seg_menor):
            i2, j2 = (i + 1) % seg_mayor, (j + 1) % seg_menor
            f = bm.faces.new((vs[i][j], vs[i2][j], vs[i2][j2], vs[i][j2]))
            f.material_index = mat
            caras.append(f)
    suavizar(caras)
    return caras


# Domo de vidrio sobre el lomo, hacia el frente.
esfera((0.0, 0.10, 0.40), (0.27, 0.30, 0.20), 16, 8, VIDRIO)
toro((0.0, 0.10, 0.42), 0.27, 0.035, 20, 6, CREMA)


# ---------------------------------------------------------------- alas y aleta
def placa(puntos_2d, grosor, transformar, mat):
    """Polígono 2D (u, v) extruido con grosor, colocado con `transformar`."""
    abajo = [bm.verts.new(transformar(u, v, -grosor / 2)) for (u, v) in puntos_2d]
    arriba = [bm.verts.new(transformar(u, v, grosor / 2)) for (u, v) in puntos_2d]
    caras = [bm.faces.new(list(reversed(abajo))), bm.faces.new(arriba)]
    n = len(puntos_2d)
    for i in range(n):
        j = (i + 1) % n
        caras.append(bm.faces.new((abajo[i], abajo[j], arriba[j], arriba[i])))
    for f in caras:
        f.material_index = mat
    suavizar(caras, False)
    return caras


# Ala: forma de aleta redondeada (u = hacia fuera, v = a lo largo de Y).
ala = [(0.0, -0.75), (0.0, 0.05), (0.25, -0.15), (0.50, -0.55), (0.55, -0.85), (0.40, -0.95)]
for lado in (1, -1):
    def colocar(u, v, w, lado=lado):
        # Sale del costado (x), con un poco de caída hacia abajo.
        x = lado * (0.42 + u)
        z = -0.05 - 0.25 * u + w
        return (x, v, z)
    placa(ala if lado == 1 else list(reversed(ala)), 0.06, colocar, ROJO)

# Aleta dorsal trasera (plano YZ).
aleta = [(0.0, -0.90), (0.0, -0.35), (0.20, -0.62), (0.45, -0.95), (0.40, -1.08)]
placa(list(reversed(aleta)), 0.06, lambda u, v, w: (w, v, 0.40 + u), ROJO)

# ---------------------------------------------------------------- antena
cil = bmesh.ops.create_cone(
    bm, cap_ends=True, segments=6, radius1=0.018, radius2=0.018, depth=0.32,
    matrix=Matrix.Translation((0.0, 0.42, 0.52)) @ Matrix.Rotation(math.radians(-25), 4, "X"),
)
for f in {f for v in cil["verts"] for f in v.link_faces}:
    f.material_index = METAL
esfera((0.0, 0.49, 0.67), (0.055, 0.055, 0.055), 8, 6, FOCO)

# ---------------------------------------------------------------- tobera
tobera = bmesh.ops.create_cone(
    bm, cap_ends=False, segments=16, radius1=0.20, radius2=0.30, depth=0.22,
    matrix=Matrix.Translation((0.0, -1.03, 0.0)) @ Matrix.Rotation(math.radians(90), 4, "X"),
)
for f in {f for v in tobera["verts"] for f in v.link_faces}:
    f.material_index = METAL
    f.smooth = True
# Fondo incandescente dentro de la tobera.
lathe([(-0.99, 0.20), (-0.99, 0.0)], 16, lambda i: FUEGO)

# ---------------------------------------------------------------- terminar
bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
bm.to_mesh(mesh)
bm.free()
mesh.update()

bpy.ops.wm.save_as_mainfile(filepath=os.path.join(HERE, "nave.blend"))

bpy.context.view_layer.objects.active = nave
nave.select_set(True)
bpy.ops.wm.obj_export(
    filepath=os.path.join(HERE, "nave.obj"),
    export_selected_objects=True,
    export_uv=False,
    export_normals=True,
    export_colors=False,
    export_triangulated_mesh=True,
    export_materials=True,
    forward_axis="NEGATIVE_Z",
    up_axis="Y",
)
print("Triangulos:", sum(len(p.vertices) - 2 for p in mesh.polygons))
