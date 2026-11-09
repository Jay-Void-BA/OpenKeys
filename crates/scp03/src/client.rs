//! Protected-domain composition of bounded T=1 and authenticated SCP03. It is
//! not an exported USB/NFC raw-APDU API. Callers must bind each guard to the
//! internal boot/provisioning phase or the exact consumed user grant.
use crate::{Channel, Command, Crypto, Error, Handshake, SecretResponse, StaticKeys};
use openkeys_se_transport::transport::{Clock, Io, Transport};
use zeroize::Zeroizing;

/// Only the protected service implements/owns this link. Partial/ambiguous
/// exchange errors must never be retried by the implementation or its caller.
pub trait Link {
    fn exchange(&mut self, command: &[u8], output: &mut [u8]) -> Result<usize, Error>;
}
impl<I: Io, C: Clock> Link for Transport<I, C> {
    fn exchange(&mut self, command: &[u8], output: &mut [u8]) -> Result<usize, Error> {
        self.exchange_untrusted(command, output)
            .map(|reply| reply.wire_bytes().len())
            .map_err(|_| Error::Link)
    }
}

/// Live trusted clock, cancellation, hardware faults and the exact policy
/// context. A host Boolean or cached "unlocked" state is not an adapter.
/// Internal sensor verification/attempt precharge may precede a match; those
/// phases must not reveal secrets or act as a general private-command grant.
pub trait Guard {
    fn check(&mut self) -> Result<(), Error>;
}

/// Fixed by authenticated provisioning/qualified firmware, never from a host
/// request, ATR or unauthenticated GET VERSION. The SE050 command-counter
/// profile here requires applet4.3.0 or later (all APDUs increment).
pub struct Profile<'a> {
    pub key_version: u8,
    pub full_selected_aid: &'a [u8],
    pub qualified_applet_version: u32,
}

pub struct Client<L, const N: usize> {
    link: L,
    channel: Channel<N>,
    faulted: bool,
}

impl<L: Link, const N: usize> Client<L, N> {
    /// The link must already have synchronized and selected the qualified
    /// application. Selection/ATR is not authentication. Fresh mutual SCP03
    /// and the per-device provisioned keyset provide the channel protection.
    /// No fallback key version, security level, S16/SCP02 or retry is tried.
    pub fn establish(
        mut link: L,
        crypto: &mut impl Crypto<N>,
        keys: &StaticKeys<N>,
        profile: &Profile<'_>,
        guard: &mut impl Guard,
    ) -> Result<Self, Error> {
        if profile.qualified_applet_version < 0x0403_0000 {
            return Err(Error::Unsupported);
        }
        guard.check().map_err(|_| Error::Authorization)?;
        let handshake =
            Handshake::start(crypto, keys, profile.key_version, profile.full_selected_aid)?;
        let mut reply = Zeroizing::new([0; 256]);
        guard.check().map_err(|_| Error::Authorization)?;
        let length = link.exchange(&handshake.initialize_update(), &mut reply[..])?;
        guard.check().map_err(|_| Error::Authorization)?;
        let response = reply.get(..length).ok_or(Error::Link)?;
        let pending = handshake.accept(crypto, response)?;
        reply.fill(0);
        guard.check().map_err(|_| Error::Authorization)?;
        let length = link.exchange(pending.external_authenticate(), &mut reply[..])?;
        guard.check().map_err(|_| Error::Authorization)?;
        let response = reply.get(..length).ok_or(Error::Link)?;
        let channel = pending.finish(response)?;
        Ok(Self {
            link,
            channel,
            faulted: false,
        })
    }

    pub fn close(&mut self) {
        self.faulted = true;
        self.channel.close();
    }

    /// Exactly one attempt. Recheck after every potentially blocking phase
    /// and after authentication/decryption, before releasing any plaintext.
    /// On error the client is permanently closed; do not refund persistent
    /// budgets or automatically re-establish/replay the uncertain operation.
    pub fn exchange(
        &mut self,
        crypto: &mut impl Crypto<N>,
        command: &Command<'_>,
        guard: &mut impl Guard,
    ) -> Result<SecretResponse, Error> {
        if self.faulted {
            return Err(Error::State);
        }
        let result = self.exchange_inner(crypto, command, guard);
        if result.is_err() {
            self.close();
        }
        result
    }
    fn exchange_inner(
        &mut self,
        crypto: &mut impl Crypto<N>,
        command: &Command<'_>,
        guard: &mut impl Guard,
    ) -> Result<SecretResponse, Error> {
        guard.check().map_err(|_| Error::Authorization)?;
        let command = self.channel.wrap(crypto, command)?;
        guard.check().map_err(|_| Error::Authorization)?;
        let mut reply = Zeroizing::new([0; 256]);
        let length = self.link.exchange(command.wire_bytes(), &mut reply[..])?;
        guard.check().map_err(|_| Error::Authorization)?;
        let response = reply.get(..length).ok_or(Error::Link)?;
        let plaintext = self.channel.unwrap(crypto, response)?;
        guard.check().map_err(|_| Error::Authorization)?;
        Ok(plaintext)
    }
}
