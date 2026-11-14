# Classical R05: SE and USB backup supply

[Status](DEVELOPMENT_STATUS.md) · [Hardware](../hardware/r05/README.md) · [Security review](SECURITY_REVIEW_R05.md)

This is a schematic and routed-PCB correction, not a production qualification.
The original STM32H573, single secure-element architecture is retained.
SM2, ML-DSA and ML-KEM are excluded from the classical edition.

## Secure-element selection

U5 is now **NXP SE051C2HQ1/Z01XDZ**, 12NC **935414457472**,
SOT1969-1 / HX2QFN20. The procurement target is the full-feature **OEF A8FA,
Applet 7.2**, with 46 kB user memory. Older C2 configurations are not approved
substitutes. NXP AN12973 lists RSA up to 4096 bits including on-chip generation,
NIST ECDSA/ECDH, Ed25519/X25519, AES and HMAC for this C2 configuration.
This resolves the previous E2 silicon feature mismatch; firmware integration,
native PIN ACLs, exact algorithm commands and physical qualification remain open.
The RSA-generation restrictions for W2 or F2 must not be copied onto C2.

The pin table was checked against the primary SE051 data sheet: SDA 9, SCL 10,
ENA 11, VIN 12, VOUT 15 linked to VCC 18; unused RF 1/17, RST_N 14 and VSS 19
are grounded. The exposed pad is grounded; unused IO/CLK/NC pins stay unconnected.
The existing footprint retains its historical `SE050_SOT1969-1_HX2QFN20` name;
that name does not select E2 silicon. Netlist and board checks bind the C2 MPN,
package and electrical pins independently.

The existing SE05x Rust backend and native-PIN probe are candidates, not proof
that Applet 7.2 enforces the intended ACLs. Test UserID failure limits, object
immutability, authenticated-session closure, SCP03, PIN-bound key access,
power-loss recovery and RSA commands on the exact received device. Do not enable
MCU software private-key operations as a compatibility fallback.

## USB operation without a battery

The BQ25185 SYS path supplies the main regulator and switched LCD from USB,
including with the protected main pack absent. Charging stays disabled in the
public diagnostic. Battery-free USB operation is a bench acceptance condition,
not an observed result.

The independent **Panasonic CR-2032/HFN** pre-tabbed primary cell now feeds MCU
VBAT through **R61 10k** and **U13 LM66100DCKR**. This replaces the SYS-only
MCP1700. LM66100 CE is tied to its VOUT, as required for reverse blocking;
CE-to-ground would defeat that function. Unused ST is grounded per TI.
C40 is 100nF at VIN; C41 4.7uF and C56 100nF are local to MCU VBAT.
The main cell and primary backup are different batteries. Do not charge the
primary, solder an ordinary coin body, or enable the MCU internal VBAT charger.
Reverse leakage is nonzero and requires primary-cell supplier approval and
measurements over temperature and all supply transitions.

PA1 / **TAMP_OUT4** goes through R63 and three dispersed internal contacts
SW13/14/15 to PC13 / **TAMP_IN1**. SW12 separately drives a main-powered hard
reset; it no longer draws continuous current from backup. PC1 is unused because
it is not a VBAT-capable tamper pin; normal NRST is not treated as a lid event.
U27 (low-leakage dual Schmitt) conditions the two live nodes before the strapped
U24 XOR. R66/C62, U25 Schmitt and U26 open drain join the existing NRST fault
bus, independently clearing SE enable and the LCD latch. No software arm pin or
host-controlled bypass is provided. A bridge that preserves the waveform remains
a residual attack; contacts alone are not an active mesh.

The intended H573 active profile shares output4 with input1, using the LSE clock
and a seeded hardware engine while main power is absent. Backup SRAM is off;
the 32 backup registers retain a continuity anchor. Missing anchor, pending
flags or changed active-engine state must quarantine an already commissioned
key, never silently initialise it as blank. Firmware readback/profile and Root
continuity integration must match this board; old passive-IN1/IN3 binaries are
incompatible. No automatic normal-boot flag clearing or reseeding is permitted.

The main-cell ADC input is isolated by **TMUX1511RWBR** with SEL1=NRST. Its
330k/330k divider and 10nF reservoir are upstream; the ADC-side 100k bias gives
normal ratio **100/530**, rather than 1/2. Unused channels are grounded. A
control-pin fail-safe switch without analog powered-off protection is not a
substitute. R69 47k provides a passive main-rail discharge path against finite
off-state leakage; the 4uA aggregate allocation and partial-power ramps need
measurement, including NFC. Measured voltage is not an accurate state-of-charge percentage.

See the [netlist-bound allocations and passive models](../hardware/r05/review/backup-tamper-model.json)
and [bench procedure](../manufacturing/BACKUP_TAMPER_TESTS_R05_ZH.md).
The model keeps active-engine current, primary usable capacity, PCB leakage,
logic thresholds and skew allocations explicit. Its RC crossing is not a bound
on PRNG runs, sensor bypass, voltage/clock injection or complete attack latency.
No backup service life or physical resistance has been qualified. The separate
[LCD power correction](CLASSICAL_LCD_POWER_R05.md) also needs measurement.

## Mainland three-unit procurement

Checked 2026-11-11: [DigiKey China C2 listing](https://www.digikey.cn/zh/products/detail/nxp-usa-inc/SE051C2HQ1-Z01XDZ/18712817)
shows single-piece cut tape; its snapshot lists about RMB 53.01 including VAT.
This is not a reservation or a guarantee of a Mainland warehouse, personal
checkout eligibility, OEF or delivery date. The assembler listing is
[JLCPCB C5202100](https://jlcpcb.com/partdetail/NXPSemicon-SE051C2HQ1Z01XDZ/C5202100);
an assembly catalog entry does not establish stock or loose-chip sales.
Confirm exact MPN, OEF/Applet, date code, traceability and a three-piece quote
before ordering. Foreign chips remain preferred; no Chinese substitute has
been electrically or cryptographically qualified.

## Primary references and first-board tests

- [NXP SE051 data sheet](https://www.nxp.com/docs/en/data-sheet/SE051.pdf), Rev. 2.0, 2024-07-08.
- [NXP AN12973 configurations](https://www.nxp.com/docs/en/application-note/AN12973.pdf), C2 OEF/Applet and algorithm tables.
- [ST STM32H573 data sheet](https://www.st.com/resource/en/datasheet/stm32h573ri.pdf), DS14121 Rev5: VBAT1.2..3.6V, backup-current and VBAT-capable pin tables.
- [TI LM66100](https://www.ti.com/lit/ds/symlink/lm66100.pdf), SLVSEZ8A: CE=VOUT reverse blocking, pinout and leakage.
- [TI SN74AUP2G17](https://www.ti.com/lit/ds/symlink/sn74aup2g17.pdf), SCES750A: slow-input Schmitt buffers and powered-off leakage.
- [TI SN74AUP1G99](https://www.ti.com/lit/ds/symlink/sn74aup1g99.pdf), SCES594C: strapped XOR truth table; its inputs need fast edges.
- [TI TMUX1511](https://www.ti.com/lit/ds/symlink/tmux1511.pdf), SCDS390B: RWB0012A pinout, signal-path powered-off protection and leakage.
- Exact Panasonic HFN capacity, pulse/reverse-current acceptance, tab dimensions and lot traceability still require the supplier's current primary documents; nominal225mAh is provisional.
- [TI BQ25185 data sheet](https://www.ti.com/lit/ds/symlink/bq25185.pdf).

For E01, use synthetic data: battery absent, protection-cutoff cell, low cell,
USB insertion/removal, SW1 off/on and slow supply ramps. Observe SYS, VBAT_BACKUP,
MCU VBAT, NRST, case fault and SE enable with a scope. No private operation may
survive a reset/fault or regain a previous PIN session. Measure overshoot,
undershoot, regulator stability, temperature, RTC continuity and worst load;
these tests do not replace the separate fault-injection/side-channel plan.
