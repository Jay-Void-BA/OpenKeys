// SPDX-License-Identifier: CERN-OHL-S-2.0
// OpenKeys R0 dimensional prototype. NOT a tooling/manufacturing release.
// Units: mm. Front at +Z, USB-A at Y=0. Exact module/connector selection pending.
part = "assembly"; // [assembly,base,lid,plungers,exploded]
$fn=64;
W=44; H=88; D=16; wall=1.8; floor_t=1.6; lid_t=1.6;
board_z=6.4; board_t=1.0;
screws=[[6,6],[38,6],[6,60],[38,60]];
buttons=[[8,38],[16,38],[8,48],[16,48]];
module rr(w,h,r,z){hull(){for(x=[r,w-r])for(y=[r,h-r])translate([x,y,0])cylinder(r=r,h=z);}}
module base(){
 difference(){
  union(){
   difference(){rr(W,H,5,D-lid_t);translate([wall,wall,floor_t])rr(W-2*wall,H-2*wall,3.2,D);}
   for(p=screws)translate([p[0],p[1],floor_t])cylinder(d=5.2,h=board_z-floor_t);
  }
  for(p=screws)translate([p[0],p[1],floor_t-.1])cylinder(d=1.6,h=board_z+1);
  // Connector openings are provisional envelopes, not connector drawings.
  translate([15.5,-1,5.4])cube([13,5,6.2]);
  translate([W-wall-1,17.5,5.2])cube([5,10,5.6]);
  // Recess for OFF/ON side switch.
  translate([-1,54,6])cube([4,8,4]);
 }
}
module lid(){
 difference(){
  union(){
   translate([0,0,D-lid_t])rr(W,H,5,lid_t);
   translate([2.05,2.05,D-lid_t-1.2])difference(){rr(W-4.1,H-4.1,3,1.4);translate([1,1,-.1])rr(W-6.1,H-6.1,2,1.6);}
   for(p=screws)translate([p[0],p[1],board_z+board_t])cylinder(d=4.6,h=D-lid_t-board_z-board_t);
  }
  translate([7.5,8.5,13])cube([29,17,5]); // OLED viewing window
  translate([21.7,34.7,13])cube([16.6,16.6,5]); // FP envelope: exact module TBD
  for(p=buttons)translate([p[0],p[1],13])cylinder(d=5.8,h=5);
  for(p=screws){
   translate([p[0],p[1],7])cylinder(d=2.4,h=10);
   translate([p[0],p[1],15.0])cylinder(d1=2.4,d2=4.3,h=1.05);
  }
 }
}
module plunger(){union(){cylinder(d=5.2,h=3.4);cylinder(d=6.5,h=.7);translate([0,0,-4])cylinder(d=2.2,h=4.1);}}
module plungers(){for(p=buttons)translate([p[0],p[1],12.9])plunger();}
module components(){
 color("seagreen")translate([3,3,board_z])cube([38,82,board_t]);
 color([.65,.68,.72])translate([12,31,1.8])cube([20,30,4]); // protected battery envelope
 color([.03,.04,.05])translate([6,7,10.8])cube([32,25,2.4]);
 color([.02,.18,.23])translate([7.5,8.5,13.2])cube([29,17,.5]);
 color([.12,.15,.18])translate([22,35,11.8])cube([16,16,4]);
 color([.7,.73,.78])translate([16,-12,6])cube([12,17,4.5]);
 color([.6,.64,.68])translate([37,18,5.5])cube([7,9,4.5]);
}
if(part=="base")base();
else if(part=="lid")translate([0,H,D])rotate([180,0,0])lid();
else if(part=="plungers")for(i=[0:3])translate([i*9,0,3.4])rotate([180,0,0])plunger();
else if(part=="exploded"){
 color([.22,.27,.32])base();components();
 color([.24,.3,.36])translate([0,0,18])lid();
 color([.1,.65,.72])translate([0,0,18])plungers();
}else{
 color([.22,.27,.32])base();components();
 color([.24,.3,.36])lid();color([.1,.65,.72])plungers();
}
