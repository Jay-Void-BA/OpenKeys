# SPDX-License-Identifier: CERN-OHL-S-2.0
# blender -b -t 2 -P hardware/r04/tools/render.py
import bpy,math
from pathlib import Path
from mathutils import Vector
R=Path(__file__).resolve().parents[1]
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
def mat(name,c,metal=0):
 m=bpy.data.materials.new(name);m.diffuse_color=(*c,1);m.use_nodes=True;n=m.node_tree.nodes.get('Principled BSDF');n.inputs['Base Color'].default_value=(*c,1);n.inputs['Metallic'].default_value=metal;n.inputs['Roughness'].default_value=.4;return m
shell=mat('Graphite shell',(.06,.105,.15));teal=mat('Context keys',(.04,.48,.5));metal=mat('Metal',(.62,.7,.76),.8);ink=mat('LCD dark text',(.012,.025,.02));white=mat('White backlight',(.9,.95,.91));sensor=mat('FP envelope',(.12,.18,.23))
def cube(name,pos,size,material):
 bpy.ops.mesh.primitive_cube_add(size=1,location=Vector(pos)+Vector(size)/2);o=bpy.context.object;o.name=name;o.dimensions=size;bpy.ops.object.transform_apply(location=False,rotation=False,scale=True);o.data.materials.append(material);return o
for name in ['base','lid']:
 bpy.ops.wm.stl_import(filepath=str(R/'exports'/('OpenKeys-'+name+'.stl')));o=bpy.context.object;o.name=name;o.data.materials.append(shell)
cube('Electrical PCB body',(3,3,11),(68,92,1),mat('PCB',(.04,.24,.14)))
cube('Protected500mAh pack envelope',(19.5,37,1.8),(29,36,4.75),mat('Cell',(.55,.61,.65),.3))
cube('16x2 white LCD',(13.65,11.5,19.21),(46.7,15.3,.05),white)
bpy.ops.mesh.primitive_cylinder_add(vertices=64,radius=7,depth=.7,location=(52.3,51.3,18.95));bpy.context.object.data.materials.append(sensor)
bpy.ops.mesh.primitive_cylinder_add(vertices=64,radius=6.05,depth=.8,location=(52.3,51.3,19.7));bpy.context.object.data.materials.append(sensor)
cube('USB-C receptacle',(32.53,-.15,12),(8.94,7.35,3.16),metal);cube('USB-C opening',(32.9,-.17,12.35),(8.2,.15,2.45),ink)
font=bpy.data.fonts.load('/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf')
def txt(body,x,y,z,size,material):
 c=bpy.data.curves.new(body,'FONT');c.body=body;c.font=font;c.size=size;c.align_x='CENTER';c.extrude=.005;o=bpy.data.objects.new(body,c);bpy.context.collection.objects.link(o);o.location=(x,y,z);o.rotation_euler=(0,0,math.pi);o.data.materials.append(material)
txt('OpenKeys',37,17,19.28,3.2,ink);txt('Use fingerprint',37,23,19.28,3.2,ink)
for i,(x,y) in enumerate([(13,41.5),(26,41.5),(13,62),(26,62)]):
 bpy.ops.mesh.primitive_cylinder_add(vertices=40,radius=2.8,depth=1.2,location=(x,y,19.65));bpy.context.object.data.materials.append(teal);txt(str(i+1),x,y+.8,20.28,2.6,white)
for x,y in [(6,6),(68,6),(6,70),(68,70)]:
 bpy.ops.mesh.primitive_cylinder_add(vertices=32,radius=1.6,depth=.3,location=(x,y,19.85));bpy.context.object.data.materials.append(metal);cube('Screw slot',(x-1,y-.12,20.03),(2,.24,.04),ink)
txt('OPENKEYS',50,66,20.05,2.6,white);txt('NFC',37,87,20.05,3.5,white)
for i in range(5):
 curve=bpy.data.curves.new('Decorative fingerprint illustration','CURVE');curve.dimensions='3D';curve.bevel_depth=.07;sp=curve.splines.new('POLY');sp.points.add(39)
 for j,p in enumerate(sp.points):a=math.radians(-145+j*290/39);p.co=(52.3+(2+i*.7)*math.cos(a),51.3+(3+i*.7)*math.sin(a),19.7,1)
 o=bpy.data.objects.new('Decorative fingerprint illustration',curve);bpy.context.collection.objects.link(o);o.data.materials.append(metal)
# Match the SCAD right-handed manufacturing frame; text glyphs keep their
# orientation and only their baseline/position changes. STL parts are
# already in this frame and must not be mirrored again.
from mathutils import Matrix
cad_transform=Matrix.Translation((0,98,0)) @ Matrix.Diagonal((1,-1,1,1))
for obj in list(bpy.context.scene.objects):
 if obj.name in ('base','lid'):continue
 if obj.type=='FONT':obj.location.y=98-obj.location.y;obj.rotation_euler.z=0
 else:obj.matrix_world=cad_transform @ obj.matrix_world
cube('Backdrop',(-100,-100,-2),(300,350,1.9),mat('Backdrop',(.82,.87,.9)))
for loc,power,size in [((80,0,190),190000,150),((-80,90,120),130000,130)]:
 bpy.ops.object.light_add(type='AREA',location=loc);o=bpy.context.object;o.data.energy=power;o.data.shape='DISK';o.data.size=size;o.rotation_euler=(Vector((37,45,5))-o.location).to_track_quat('-Z','Y').to_euler()
bpy.ops.object.camera_add(location=(155,-135,240));o=bpy.context.object;o.rotation_euler=(Vector((37,46,8))-o.location).to_track_quat('-Z','Y').to_euler();o.data.type='ORTHO';o.data.ortho_scale=150;bpy.context.scene.camera=o
s=bpy.context.scene;s.render.engine='CYCLES';s.cycles.samples=24;s.cycles.use_denoising=False;s.render.resolution_x=1400;s.render.resolution_y=1400;s.render.resolution_percentage=100;s.view_settings.view_transform='AgX';s.render.image_settings.file_format='PNG';s.render.filepath=str(R/'exports/OpenKeys-assembly.png')
bpy.ops.wm.save_as_mainfile(filepath=str(R/'mechanical/OpenKeys-assembly.blend'));bpy.ops.render.render(write_still=True)
