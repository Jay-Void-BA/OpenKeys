# Classical R05 LCD power and reset correction

[Status](DEVELOPMENT_STATUS.md) · [Circuit and PCB](../hardware/r05/README.md) · [Bench instructions](../manufacturing/BUILD_R05_ZH.md)

This corrects two electrical design gaps found before hardware qualification:
the DFR0555 minimum supply was not covered by a nominal3.3V rail, and a new
arm edge immediately after reset could restore retained display contents.
These corrections are engineering candidates, not proof of secure clearing.

## Supply and level isolation

TPS63031 PS/SYNC now follows VIN/SYS for forced PWM. Its guaranteed static
main-rail range is3.267..3.333V; even that cannot supply a module requiring
at least3.3V once regulator and harness errors are included. U18 therefore
switches SYS to the LCD rather than main3.3V. U21 TCA9517A separates the
main3.3V A-side from the switched-SYS B-side. No5V LCD signal directly drives
an H573 pin. Both B-side pullups use switched power.

U22 TPS3700 is powered by main3.3V and independently monitors SYS with two
0.1percent divider ladders. Its open-drain outputs join NRST. U10 remains
powered by SYS and monitors main3.3V. Static worst corners include reference
limits, resistor errors and +/-25nA input bias:

| Bound | Voltage |
|---|---|
| SYS UV assertion, falling |3.4246..3.5563V|
| SYS UV recovery, rising |3.5043..3.5918V|
| SYS OV assertion, rising |4.7406..4.8597V|
| Qualified LCD lower bound, **if** switch plus harness drop<=50mV |3.3746V|

Low-battery cutoff deliberately occurs early. A500mAh label does not establish
usable runtime above that cutoff. Main PWM ripple, transitions, current margin,
reset propagation and the complete battery/charger load are not covered by
these DC calculations. Comparator typical delay is not nanosecond fault protection.

TCA9517A EN high threshold follows VCCB. U23 is an open-drain level converter;
R53/R54 pull EN from switched power and low when off. A direct3.3V high would
not meet the EN threshold at the highest SYS. Saved-PCB checks bind the pinout,
both supplies, pullups and same-side local decoupling.

## Clearing and restart

NRST or open-drain PC3 pulls U17 MR low through U15. U17 TPS3808G33 holds
the latch asynchronous clear low for180..420ms after release. CT connects
to VDD **through100k**, never directly; its fixed-delay mode requires40..200k.
R57 pulls up the open-drain reset output. A new PC4 edge is required after
the hold; a clock level held high during clear does not rearm the latch.

R47 is330ohm1percent,0.25W1206; it discharges the switched rail without
relying on powered firmware. R59 keeps switch ON low when the main-powered
latch is unpowered, including backcharge through the bus-delay network.
R47 adds up to about73mW at the stable DC upper corner; input-current and
thermal qualification must include it.

The executable [model](../hardware/r05/tools/display_model.py) assumes total
LCD/module/harness capacitance<=100uF, aggregate off injection<=20uA and
aggregate turn-off delay<=5ms. Under those **unmeasured** assumptions, the
180ms minimum hold leaves at least175ms off and a calculated residual of
about0.0355V. A500ms software off hold gives about0.0067V. These are electrical
model results, not measurements of visible glass clearing or active-probe resistance.

Firmware must keep PC4 low and PC3 low for500ms; release PC3, wait500ms
(above the420ms maximum fixed delay), then raise PC4 and wait200ms before
full initialization. Any GPIO/I2C failure latches off until restart. It never
automatically resumes an old frame. The protected Root must own all pins and
buses; this circuit cannot constrain compromised Root software or a soldered bypass.

## Required bench checks

Use synthetic display contents and no owner secrets. Record LCD_VCC_SW,
NRST, MR, CLEAR_N, POWER_LATCH, EN and SCL/SDA across reset, manual lock,
slow ramps, low battery, USB insertion/removal and complete power loss.
Exercise early/repeated PC4 edges during the hardware hold. Verify the
minimum hold, maximum recovery, no automatic repower, off leakage and actual
visible clearing across temperature and received modules. Measure drop and
capacitance rather than assuming the bounds above. Injected fast faults,
backfeed and side channels remain separate lab tests.

The current diagnostic cold start includes two500ms off holds,500ms release
wait and200ms POR wait before initialization traffic. Measure total startup
and the watchdog's earliest timeout across actual clocks and temperature;
repeated watchdog resets are a failed bring-up, not a successful display test.

Primary references: [TI TPS63031](https://www.ti.com/lit/ds/symlink/tps63031.pdf),
[TPS3700](https://www.ti.com/lit/ds/symlink/tps3700.pdf),
[TPS3808](https://www.ti.com/lit/ds/symlink/tps3808.pdf),
[TCA9517A](https://www.ti.com/lit/ds/symlink/tca9517a.pdf),
[TPS22919](https://www.ti.com/lit/ds/symlink/tps22919.pdf),
[SN74LVC1G07](https://www.ti.com/lit/ds/symlink/sn74lvc1g07.pdf),
[DFRobot module](https://wiki.dfrobot.com/dfr0555).
