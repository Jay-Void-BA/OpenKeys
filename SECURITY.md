# Security status

OpenKeys is an unaudited development project, not a production authenticator. Device firmware, trusted biometric integration, persistent encrypted storage and the authenticated management transport are incomplete. The desktop application refuses hardware authorization until a real verifier is implemented.

Do not use real credentials or recovery codes with this revision. Do not post keys, fingerprint data, OTP seeds or recovery codes in issues. Use GitHub's private vulnerability reporting if the repository maintainer enables it; otherwise request a private reporting channel without disclosing exploit details publicly.

Security claims must identify exact firmware, hardware, chip revision, lifecycle configuration and evaluation scope. Passing unit tests, ERC, a dependency scan or a component certificate is not a whole-device security evaluation. See hardware/RELEASE_GATES.md and docs/ARCHITECTURE.md.
