#![forbid(unsafe_code)]
//! GPUI manager model. The production device channel is not integrated.
//! No host Boolean/PIN or GUI button can create a device authorization.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Connect,
    ImportOtp,
    ShowOtpOnKey,
    DeleteOtp,
    OpenPgp,
    Piv,
    Ssh,
    ImportNotes,
    ViewPrivateRecords,
    ChangeHardwarePin,
}
impl Action {
    pub const ALL: [Self; 10] = [
        Self::Connect,
        Self::ImportOtp,
        Self::ShowOtpOnKey,
        Self::DeleteOtp,
        Self::OpenPgp,
        Self::Piv,
        Self::Ssh,
        Self::ImportNotes,
        Self::ViewPrivateRecords,
        Self::ChangeHardwarePin,
    ];
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    SecureChannelUnavailable,
}

pub struct Backend;
impl Backend {
    /// Deliberate fail-closed development boundary. Production requires an
    /// authenticated device, channel binding and an authenticated local-PIN session and exact local approval.
    /// Discovery, NFC UID, USB descriptors and an OS unlock cannot satisfy it.
    pub fn request(&mut self, _action: Action) -> Result<(), Error> {
        Err(Error::SecureChannelUnavailable)
    }
}

/// No OTP/seed response type exists: managers request display on the device.
/// Import secrets must use a separate bounded encrypted upload, never labels,
/// logs, UI previews, clipboard or a calculate-all OATH response.
pub const OTP_POLICY: &str =
    "Seeds cannot be exported. Six-digit codes appear only on the key LCD.";
pub const PIN_POLICY: &str = "Set a 6-digit hardware PIN on first boot. Protocol PINs remain separate. Both use paired-session 30-second idle leases.";

#[cfg(feature = "gui")]
pub mod ui;
