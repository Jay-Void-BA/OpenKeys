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

U13 MCP1700 input and C40 now use the charger's **SYS**, rather than BAT.
SYS can be supplied by USB without a battery and by the protected cell when USB
is absent. U13 output has local C41; MCU VBAT has separate C56. C40/C41 are
4.7 µF X7R parts and must retain at least 1 µF effective capacitance across
DC bias, tolerance and temperature. All three capacitors are on the same side
as their consumers and within 3 mm, independently checked from the saved PCB.

This closes the previous schematic-only BAT dependency of the tamper/RTC rail.
It does not prove startup, input-current margin or correct authentication with
an empty/missing cell. The separate [LCD correction](CLASSICAL_LCD_POWER_R05.md)
now adds switched SYS, level buffering, a voltage window and hardware reset hold;
its DC and discharge bounds still require bench qualification. The diagnostic
continues to disable charging and all private services.

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
- [Microchip MCP1700 data sheet](https://ww1.microchip.com/downloads/en/DeviceDoc/20001826F.pdf).
- [TI BQ25185 data sheet](https://www.ti.com/lit/ds/symlink/bq25185.pdf).

For E01, use synthetic data: battery absent, protection-cutoff cell, low cell,
USB insertion/removal, SW1 off/on and slow supply ramps. Observe SYS, TAMP_3V0,
MCU VBAT, NRST, case fault and SE enable with a scope. No private operation may
survive a reset/fault or regain a previous PIN session. Measure overshoot,
undershoot, regulator stability, temperature, RTC continuity and worst load;
these tests do not replace the separate fault-injection/side-channel plan.
