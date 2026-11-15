# Classical R05: NFC power-domain isolation

[Hardware](../hardware/r05/README.md) · [Power](CLASSICAL_SE_POWER_R05.md) · [Bench tests](../manufacturing/NFC_ISOLATION_TESTS_R05_ZH.md)

The H573 / SE051C2 architecture is retained. This correction addresses an
unqualified electrical boundary, not a demonstrated exploit or a claim that
NFC is now immune to physical attacks. Production Root and protocol integration
are still incomplete. No physical test has been run.

## Boundary and circuit

FM11NC08S can operate from an RF field. Leaving its VOUT disconnected from the
main supply does not, by itself, prove that its other pins cannot back-power
the main MCU. The available Fudan v1.0 brief identifies the SPI and RF pins but
does not provide a complete powered-off I/O leakage table. Isolation must not
be inferred from that omission.

| Boundary | Implemented circuit | Required condition |
|---|---|---|
| CS, SCK, MOSI, MISO | U29 TMUX1511RWBR, all four bidirectional channels | Main-powered VDD; both shared enables hard-wired to NRST |
| IRQ | One U30 TMUX1511RWBR channel; unused signal pins grounded | Same physical reset enable; no host/software bypass |
| Each RF-side signal | R70–R74, 330Ω, 1% | Limits contention/edge current; does not clamp overvoltage |
| NFC VCC | U31 LM66100DCKR from main 3.3V to NFC_VCC | CE=VOUT; unused ST=GND; NC left unconnected |
| RF-side defaults | R75: CS to NFC_VCC; R76/R77: SCK/MOSI to GND, each 100k | Host IRQ/CS pullups R21/R22 remain on the main side |
| Main discharge | R69 9.1k, 1%, main 3.3V to GND | Finite leakage must not accumulate on a floating rail |

C26/C27 decouple **NFC_VCC**; C28 decouples the separate **NFC_VOUT**.
C69/C70 decouple the main-side switches; C71 is local to U31 VIN. VOUT is not
connected to SYS, BAT, backup VBAT or main 3.3V. The five `NFC_RF_*` nets cannot
share copper with their host counterparts. The exported XML and saved PCB
are checked independently for these connections and the exact packages.

When NRST is low the switches disconnect before software runs. Reset release
does not authorise a private service or renew a PIN lease. Root must still
enforce local PIN, independent protocol roles, paired sessions, trusted time,
fresh request confirmation and physical link revocation. USB/NFC interrupt
or field presence is never an authentication result.

## Electrical limits and allocations

TI SCDS390B specifies TMUX1511 signal terminals **0–3.6V when VDD<1.5V**.
Its **±2µA** powered-off leakage limit is specified at **VDD=0V** and source
0–3.6V / drain 0V. The recommended signal-voltage range is not a leakage bound
for every intermediate supply voltage. Likewise, a fail-safe control pin is
not powered-off protection of the signal terminals. Do not substitute
TMUX1101 or TS5A3166 on that basis.

The static **engineering allocation**, conditional on all terminal limits,
is 5×2µA for the NFC channels +3µA for reverse supply leakage +4µA for other
paths +3µA margin =20µA. The reverse-supply allocation uses LM66100's
−40–85°C bound (2.7µA maximum at up to 5.5V reverse difference); it does not
cover its higher-temperature limit or define the product operating range.
At R69's high tolerance corner:

`20µA × 9.1kΩ × 1.01 = 0.18382V`.

This is not a guaranteed main-rail voltage. The FM11 RF-side voltages, board
contamination, all reverse supply paths, switch leakage during supply ramps
and regulator behaviour must be measured. Static LM66100 leakage is not the
reverse turn-off current or pulse charge. The source/drain protection limit
does not establish survival against excessive RF, negative pulses or invasive
injection. If any corner exceeds the allocation, the design must change.

R69 draws about **0.363mA at 3.3V** while main power is on. That cost is explicit;
it is not included in a promised battery life. The primary backup-cell circuit
is unchanged and still needs capacity and reverse-current qualification.

Initial NFC SPI operation is limited to **2MHz until qualified**. This is a
hardware interface requirement, not a claim that the production SPI driver
already exists. Verify edge rate, total capacitance, MISO setup/hold, IRQ latency
and protocol timing on the assembled board over temperature and supply range.
NFC still requires main battery power for LCD, keypad and authentication.

## Primary sources

- [TI TMUX1511](https://www.ti.com/lit/ds/symlink/tmux1511.pdf), SCDS390B, March 2025: RWB0012A pin table, recommended operating conditions, IPOFF measurement conditions and powered-off signal protection.
- [TI LM66100](https://www.ti.com/lit/ds/symlink/lm66100.pdf), SLVSEZ8A: CE=VOUT, unused ST connection, reverse-current comparator and nonzero reverse leakage. Static limits do not bound transition pulse charge.
- [Fudan FM11NC08 brief](https://wiki.nfc.im/attachments/7), v1.0, 2014: RF-derived VOUT and pin roles. The public brief is insufficient to qualify powered-off I/O or strong-field operation; obtain the exact supplier revision and full electrical limits.

No FIDO, Common Criteria, EMC or side-channel certification is inferred.
