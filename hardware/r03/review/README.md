# R0.3 review evidence

- `erc.json`: unmodified KiCad9 report, zero electrical errors, one antenna footprint link warning (J4), still a release blocker.
- `netlist-check.json`:37 safety-related checks against the exported XML, independent of generator in-memory state. It is not an analog/performance/security certificate.
- `connections.json`: human-reviewable intended connections; XML remains the checked export.
- `datasheets.json`: downloaded primary PDFs with hashes/URLs; PDFs themselves are not redistributed.
- `../../../docs/SECURITY_REVIEW_R03.md`: discovered issues, fixes, unresolved attack surfaces and physical test requirements.

There is no PCB DRC/manufacturing release or whole-device security evidence. The floorplan has no electrical nets and no tracks. No warning suppression or fake test artifact is used to authorize fabrication.

Input guard calculations are in protection-calculations.json. The NFC candidate lands now have a Fudan-authored package source and the eFuse uses TI RPW0010A compound lands; neither is production-qualified.

footprint-geometry.json records pin numbering, IN/OUT versus EP dimensions and minimum candidate copper separation. These checks do not cover solder process yield, supplier revision, stencil or physical qualification.
