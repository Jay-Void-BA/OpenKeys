# R0.3 review evidence

- `erc.json`: unmodified KiCad9 report, zero electrical errors, two RF footprint link warnings (U7/J4). Both remain release blockers.
- `netlist-check.json`:31 safety-related checks against the exported XML, independent of generator in-memory state. It is not an analog/performance/security certificate.
- `connections.json`: human-reviewable intended connections; XML remains the checked export.
- `datasheets.json`: downloaded primary PDFs with hashes/URLs; PDFs themselves are not redistributed.
- `../../../docs/SECURITY_REVIEW_R03.md`: discovered issues, fixes, unresolved attack surfaces and physical test requirements.

There is no PCB DRC/manufacturing release or whole-device security evidence. The floorplan has no electrical nets and no tracks. No warning suppression or fake test artifact is used to authorize fabrication.
