// SPDX-License-Identifier: CERN-OHL-S-2.0
// R0.3 engineering enclosure, mm. No tooling / waterproof / fit release.
part="assembly"; $fn=48;
W=74; H=98; D=20; wall=1.8; floor_t=1.6; lid_t=1.6; board_z=11;
screws=[[6,6],[68,6],[6,70],[68,70]];
buttons=[[14,46],[26,46],[14,60],[26,60]];
module rr(w,h,r,z){hull(){for(x=[r,w-r])for(y=[r,h-r])translate([x,y,0])cylinder(r=r,h=z);}}
module base(){difference(){union(){difference(){rr(W,H,5,D-lid_t);translate([wall,wall,floor_t])rr(W-2*wall,H-2*wall,3.2,D);}for(p=screws)translate([p[0],p[1],floor_t])cylinder(d=5.2,h=board_z-floor_t);}for(p=screws)translate([p[0],p[1],floor_t-.1])cylinder(d=1.6,h=board_z+1);translate([31.9,-1,10.2])cube([10.2,5,4.8]);translate([-1,64,10])cube([4,8,4]);}}
module lid(){difference(){union(){translate([0,0,D-lid_t])rr(W,H,5,lid_t);translate([2.05,2.05,D-lid_t-1.2])difference(){rr(W-4.1,H-4.1,3,1.4);translate([1,1,-.1])rr(W-6.1,H-6.1,2,1.6);}for(p=screws)translate([p[0],p[1],board_z+1])cylinder(d=4.6,h=D-lid_t-board_z-1);translate([9,64,13.4])cylinder(d=2.5,h=5.1);}translate([13.65,11.5,17])cube([46.7,15.3,5]);translate([44,43,17])cube([16.6,16.6,5]);for(p=buttons)translate([p[0],p[1],17])cylinder(d=6.2,h=5);for(p=screws){translate([p[0],p[1],11])cylinder(d=2.4,h=10);translate([p[0],p[1],19])cylinder(d1=2.4,d2=4.3,h=1.05);}}}
module plunger(){union(){cylinder(d=5.6,h=3.2);cylinder(d=6.8,h=.7);translate([0,0,-3.5])cylinder(d=2.2,h=3.6);}}
module plungers(){for(p=buttons)translate([p[0],p[1],17.4])plunger();}
module components(){color("seagreen")translate([3,3,board_z])cube([68,92,1]);color("silver")translate([18,36,1.8])cube([32,37,6]);color("ivory")translate([12.15,10,12])cube([56.8,26.35,5.5]);color("white")translate([13.65,11.5,18.2])cube([46.7,15.3,.5]);color("black")translate([44,43,15])cube([16.6,16.6,4.5]);color("silver")translate([32.5,-.2,11.5])cube([9,8,3]);}
if(part=="base")base();else if(part=="lid")translate([0,H,D])rotate([180,0,0])lid();else if(part=="plungers")for(i=[0:3])translate([i*10,0,3.2])rotate([180,0,0])plunger();else{color([.12,.18,.24])base();components();color([.12,.18,.24])lid();color([.1,.55,.6])plungers();}
