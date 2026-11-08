# SPDX-License-Identifier: CERN-OHL-S-2.0
# blender -b -t 2 -P hardware/r03/tools/render.py
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
 if name=='lid':o.rotation_euler=(math.pi,0,0);o.location=(0,98,20)
cube('PCB placement study',(3,3,11),(68,92,1),mat('PCB',(.04,.24,.14)))
cube('Protected500mAh pack envelope',(18,36,1.8),(32,37,6),mat('Cell',(.55,.61,.65),.3))
cube('16x2 white LCD',(13.65,11.5,18.2),(46.7,15.3,.5),white)
cube('FP module envelope - fit pending',(44,43,15),(16.6,16.6,4.6),sensor)
cube('USB-C receptacle',(32.5,-.2,11.5),(9,8,3),metal);cube('USB-C opening',(33,-.3,12),(8,.2,2),ink)
font=bpy.data.fonts.load('/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf')
def txt(body,x,y,z,size,material):
 c=bpy.data.curves.new(body,'FONT');c.body=body;c.font=font;c.size=size;c.align_x='CENTER';c.extrude=.005;o=bpy.data.objects.new(body,c);bpy.context.collection.objects.link(o);o.location=(x,y,z);o.rotation_euler=(0,0,math.pi);o.data.materials.append(material)
txt('OpenKeys',37,17,18.75,3.4,ink);txt('Use fingerprint',37,23,18.75,3.4,ink)
for i,(x,y) in enumerate([(14,46),(26,46),(14,60),(26,60)]):
 bpy.ops.mesh.primitive_cylinder_add(vertices=40,radius=2.8,depth=1.8,location=(x,y,19.7));bpy.context.object.data.materials.append(teal);txt(str(i+1),x,y+.8,20.65,2.6,white)
for x,y in [(6,6),(68,6),(6,70),(68,70)]:
 bpy.ops.mesh.primitive_cylinder_add(vertices=32,radius=1.6,depth=.3,location=(x,y,19.85));bpy.context.object.data.materials.append(metal);cube('Screw slot',(x-1,y-.12,20.03),(2,.24,.04),ink)
txt('OPENKEYS',50,66,20.05,2.6,white);txt('NFC',37,87,20.05,3.5,white)
for i in range(5):
 curve=bpy.data.curves.new('Sensor illustration','CURVE');curve.dimensions='3D';curve.bevel_depth=.07;sp=curve.splines.new('POLY');sp.points.add(39)
 for j,p in enumerate(sp.points):a=math.radians(-145+j*290/39);p.co=(52.3+(2+i*.7)*math.cos(a),51.3+(3+i*.7)*math.sin(a),19.7,1)
 o=bpy.data.objects.new('Sensor illustration',curve);bpy.context.collection.objects.link(o);o.data.materials.append(metal)
cube('Backdrop',(-100,-100,-2),(300,350,1.9),mat('Backdrop',(.82,.87,.9)))
for loc,power,size in [((80,0,190),190000,150),((-80,90,120),130000,130)]:
 bpy.ops.object.light_add(type='AREA',location=loc);o=bpy.context.object;o.data.energy=power;o.data.shape='DISK';o.data.size=size;o.rotation_euler=(Vector((37,45,5))-o.location).to_track_quat('-Z','Y').to_euler()
bpy.ops.object.camera_add(location=(155,175,240));o=bpy.context.object;o.rotation_euler=(Vector((37,46,8))-o.location).to_track_quat('-Z','Y').to_euler();o.data.type='ORTHO';o.data.ortho_scale=150;bpy.context.scene.camera=o
s=bpy.context.scene;s.render.engine='CYCLES';s.cycles.samples=24;s.cycles.use_denoising=False;s.render.resolution_x=1400;s.render.resolution_y=1400;s.render.resolution_percentage=100;s.view_settings.view_transform='AgX';s.render.image_settings.file_format='PNG';s.render.filepath=str(R/'exports/OpenKeys-assembly.png')
bpy.ops.wm.save_as_mainfile(filepath=str(R/'mechanical/OpenKeys-assembly.blend'));bpy.ops.render.render(write_still=True)
