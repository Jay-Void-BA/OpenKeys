// SPDX-License-Identifier: CERN-OHL-S-2.0
// R05 fit CANDIDATE. PCB-local mm + [3,3,11]; inspect actual LCD first.
// This is not a qualified anti-tamper enclosure or waterproof design.
part="assembly"; $fn=48;
W=102; H=118; D=30; wall=1.8; floor_t=1.6; lid_t=1.6; board_z=11;
holes=[[6,6],[96,6],[30,111],[96,111]];
keys=[[39,53],[57,53],[75,53],[39,67],[57,67],[75,67],
      [39,81],[57,81],[75,81],[57,95]];
labels=[1,2,3,4,5,6,7,8,9,0];
lid_contacts=[[89,105.5],[84.25,52],[36,103],[80,108]];
tip=14.64; // nominal TL3342 top=14.54; jig-adjust real travel/stop
module rr(w,h,r,z){hull(){for(x=[r,w-r])for(y=[r,h-r])translate([x,y,0])cylinder(r=r,h=z);}}
module board(){linear_extrude(1)polygon([[0,0],[42,0],[42,-2.5],[54,-2.5],[54,0],[96,0],[96,112],[0,112]]);}
module base(){difference(){union(){
 difference(){rr(W,H,5,D-lid_t);translate([wall,wall,floor_t])rr(W-2*wall,H-2*wall,3.2,D);}
 for(p=holes)translate([p[0],p[1],floor_t])cylinder(d=5.2,h=board_z-floor_t);
 // Protected pouch pocket; no compression or screws in cell/RF keepout.
 for(x=[36,70])translate([x,50,floor_t])cube([.8,43,4]);
 for(y=[49.2,93])translate([36,y,floor_t])cube([34.8,.8,3]);
 // Socket rear baffle: no second service aperture.
 translate([45,11.1,12.8])cube([12,1.2,5]);
 for(x=[45,56])translate([x,1.7,12.8])cube([1,10.6,5]);
 }
 for(p=holes)translate([p[0],p[1],floor_t+1])cylinder(d=1.6,h=board_z);
 translate([46.1,-1,11.8])cube([9.8,4,3.7]);
 translate([99,64,11.8])cube([4,8,3.2]);
}}
module lid(){difference(){union(){
 translate([0,0,D-lid_t])rr(W,H,5,lid_t);
 translate([2.05,2.05,D-lid_t-1.2])difference(){rr(W-4.1,H-4.1,3,1.4);translate([1,1,-.1])rr(W-6.1,H-6.1,2,1.6);}
 for(p=holes)translate([p[0],p[1],board_z+1])cylinder(d=4.6,h=D-lid_t-board_z-1);
 for(p=keys)translate([p[0],p[1],15.1])difference(){cylinder(d=4.4,h=13.5);translate([0,0,-.1])cylinder(d=2.7,h=14);}
 // Four blind adjustable insulating M2 actuators: separate reset + active
 // loop contacts. No additional external button or access hole.
 for(p=lid_contacts)translate([p[0],p[1],15.1])cylinder(d=5.2,h=13.3);
 // Insulating rails support the module's edge, not assumed screw holes.
 for(x=[4.4,94.7])translate([x,11,15])cube([2.8,32,13.4]);
 }
 translate([8,11.5,27.8])cube([86,31,4]);
 translate([6.5,10,29.2])cube([89,34,1]);
 for(p=keys){
  translate([p[0],p[1],28.6])cylinder(d=6.2,h=2);
  translate([p[0],p[1],28.6])cylinder(d=7.2,h=1);
 }
 for(p=holes){translate([p[0],p[1],11.9])cylinder(d=2.4,h=19);translate([p[0],p[1],28.8])cylinder(d=4.3,h=2);}
 for(p=lid_contacts)translate([p[0],p[1],14.9])cylinder(d=1.6,h=13.3);
 for(x=[6,96])for(y=[16,38])translate([x,y,14.9])cylinder(d=1.2,h=8);
}}
module lcd_retainer(){difference(){
 // Support only the provisional module edge. Inspect the real underside and
 // insulate it; change this frame if it touches any component or solder joint.
 translate([4.4,9,14])cube([93.1,36,1]);
 translate([9,12.5,13.9])cube([84,29,1.2]);
 for(x=[6,96])for(y=[16,38])translate([x,y,13.9])cylinder(d=1.8,h=1.2);
}}
module plunger(label){difference(){union(){
 translate([0,0,29.5])cylinder(d=5.6,h=1.2);
 translate([0,0,28.9])cylinder(d=6.8,h=.6);
 translate([0,0,tip])cylinder(d=2.2,h=29.6-tip);
 }
 translate([0,0,30.4])linear_extrude(.5)text(str(label),size=3.2,halign="center",valign="center");
}}
module components(){
 color("seagreen")translate([3,3,board_z])board();
 color("silver")translate([39,53,1.8])cube([29,36,4.75]);
 // Pre-tabbed primary backup, separate from the protected rechargeable pack.
 // Envelope only: sample tab height, insulation and retention need checking.
 color("silver")translate([85,103,7.2])cylinder(d=20,h=3.2);
 color("navy")translate([7.5,11,15])cube([87,32,13]);
 color([.8,.9,.95,.45])translate([6.5,10,29.2])cube([89,34,.8]);
 color("silver")translate([46.53,-.15,12])cube([8.94,7.35,3.16]);
}
module selected(){
 if(part=="base")base();
 else if(part=="lid")lid();
 else if(part=="retainer")translate([0,0,-14])lcd_retainer();
 else if(part=="plungers")for(i=[0:9])translate([(i%5)*10,floor(i/5)*10,-tip])plunger(labels[i]);
 else {color([.12,.18,.24])base();components();color([.12,.18,.24])lid();color("gray")lcd_retainer();for(i=[0:9])color("teal")translate([keys[i][0],keys[i][1],0])plunger(labels[i]);}
}
translate([0,H,0])mirror([0,1,0])selected();
