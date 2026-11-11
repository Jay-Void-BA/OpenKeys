use crate::{
    CHANNEL_MS, Entropy, Error, HANDSHAKE_MS, Identity,
    crypto::{Dh, SecretHash},
    record::{Channel, Context, NativeLink},
};
use device_policy::{
    Authorization, Operation, Request,
    execution::{Completed, State},
};
use noise_protocol::{HandshakeState, U8Array, patterns::noise_xx};
use noise_rust_crypto::{ChaCha20Poly1305, sensitive::Sensitive};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

type Noise = HandshakeState<Dh, ChaCha20Poly1305, SecretHash>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Manager,
    Key,
}
impl Role {
    pub(crate) const fn sender(self) -> u8 {
        match self {
            Self::Manager => 1,
            Self::Key => 2,
        }
    }
}

/// Backed by protected, authenticated storage; absence is distinct from an I/O
/// error. Implementations must bind pins to this device's persistent identity.
pub trait PeerStore {
    fn contains(&mut self, peer: &[u8; 32]) -> Result<bool, Error>;
    /// Atomic store/readback. A failure must never be treated as paired.
    fn insert(&mut self, peer: &[u8; 32]) -> Result<(), Error>;
}

/// Native manager UI only. Show the full comparison and require the person to
/// compare it with the physical key. This cannot authorize anything on the key.
pub trait ManagerPairingReview {
    fn compare_with_key(&mut self, comparison: &[u8; 20], peer: &[u8; 32]) -> Result<(), Error>;
}

pub struct Handshake {
    noise: Option<Noise>,
    role: Role,
    link: NativeLink,
    deadline: u64,
    last: u64,
    step: u8,
    local: [u8; 32],
}
pub(crate) fn prologue(link: NativeLink) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"OpenKeys/R05/management/Noise/1");
    h.update([link.transport_byte(), 1]);
    h.finalize().into()
}
impl Handshake {
    pub fn begin(
        role: Role,
        identity: &Identity,
        entropy: &mut impl Entropy,
        link: NativeLink,
        now: u64,
    ) -> Result<Self, Error> {
        link.validate()?;
        let deadline = now.checked_add(HANDSHAKE_MS).ok_or(Error::Time)?;
        let mut ephemeral = Zeroizing::new([0; 32]);
        entropy.fill(&mut ephemeral)?;
        if *ephemeral == [0; 32]
            || bool::from(ephemeral[..].ct_eq(&identity.0[..]))
            || bool::from(crate::crypto::public(&ephemeral).ct_eq(&identity.public_key()))
        {
            return Err(Error::Entropy);
        }
        // Hedge a repeated RNG block across native attachments/reselections.
        // This does NOT replace fresh checked entropy or a nonrepeating native
        // epoch. Both sides still speak standard Noise XX, no custom DH suite.
        use sha2_secret::Digest as _;
        let mut h = sha2_secret::Sha256::new();
        h.update(b"OpenKeys/R05/Noise-ephemeral/v1");
        h.update(ephemeral.as_slice());
        h.update(identity.public_key());
        h.update([role.sender(), link.transport_byte()]);
        h.update(link.epoch_bytes());
        let mut mixed = h.finalize();
        ephemeral.copy_from_slice(&mixed);
        mixed.as_mut_slice().zeroize();
        let noise = Noise::new(
            noise_xx(),
            role == Role::Manager,
            prologue(link),
            Some(Sensitive::from_slice(&*identity.0)),
            Some(Sensitive::from_slice(&*ephemeral)),
            None,
            None,
        );
        Ok(Self {
            noise: Some(noise),
            role,
            link,
            deadline,
            last: now,
            step: 0,
            local: identity.public_key(),
        })
    }
    fn fail<T>(&mut self, error: Error) -> Result<T, Error> {
        self.noise = None;
        Err(error)
    }
    fn check(&mut self, link: NativeLink, now: u64) -> Result<(), Error> {
        if self.noise.is_none() {
            return Err(Error::Closed);
        }
        if link != self.link {
            return self.fail(Error::Link);
        }
        if now < self.last {
            return self.fail(Error::Time);
        }
        if now >= self.deadline {
            return self.fail(Error::Expired);
        }
        self.last = now;
        Ok(())
    }
    fn writing(&self) -> bool {
        self.step.is_multiple_of(2) == (self.role == Role::Manager)
    }
    fn length(&self) -> usize {
        [32, 96, 64][self.step as usize]
    }
    /// Exact XX messages and empty payloads only. Bounded caller-owned output;
    /// no application data, peer names or secrets can ride an unfinished XX.
    pub fn write(&mut self, link: NativeLink, now: u64, output: &mut [u8]) -> Result<usize, Error> {
        output.fill(0);
        self.check(link, now)?;
        if self.step >= 3 || !self.writing() {
            return self.fail(Error::State);
        }
        let n = self.length();
        if output.len() < n {
            return self.fail(Error::Bounds);
        }
        if self
            .noise
            .as_mut()
            .ok_or(Error::Closed)?
            .write_message(&[], &mut output[..n])
            .is_err()
        {
            output.fill(0);
            return self.fail(Error::Crypto);
        }
        self.step += 1;
        Ok(n)
    }
    pub fn read(&mut self, link: NativeLink, now: u64, message: &[u8]) -> Result<(), Error> {
        self.check(link, now)?;
        if self.step >= 3 || self.writing() {
            return self.fail(Error::State);
        }
        if message.len() != self.length() {
            return self.fail(Error::Bounds);
        }
        if self
            .noise
            .as_mut()
            .ok_or(Error::Closed)?
            .read_message(message, &mut [])
            .is_err()
        {
            return self.fail(Error::Crypto);
        }
        self.step += 1;
        Ok(())
    }
    /// Completion is NOT peer acceptance. An unknown peer gets no record API.
    pub fn finish(mut self, link: NativeLink, now: u64) -> Result<PairingCandidate, Error> {
        self.check(link, now)?;
        if self.step != 3 {
            return Err(Error::State);
        }
        let noise = self.noise.take().ok_or(Error::Closed)?;
        if !noise.completed() {
            return Err(Error::State);
        }
        let peer = noise.get_rs().ok_or(Error::Peer)?;
        if peer == [0; 32] || bool::from(peer.ct_eq(&self.local)) {
            return Err(Error::Peer);
        }
        let transcript = noise.get_hash().try_into().map_err(|_| Error::Crypto)?;
        let (i_to_r, r_to_i) = noise.get_ciphers();
        let (send, receive) = match self.role {
            Role::Manager => (i_to_r, r_to_i),
            Role::Key => (r_to_i, i_to_r),
        };
        let context = Context::new(transcript, peer, link)?;
        Ok(PairingCandidate {
            channel: Channel::new(send, receive, self.role, context, now)?,
            local: self.local,
            deadline: now.checked_add(CHANNEL_MS).ok_or(Error::Time)?,
        })
    }
}

/// The only pre-pairing output is a public peer key and a transcript-dependent
/// 80-bit comparison string, shown in full over five LCD pages. Not a six-digit
/// SAS: an attacker must not get a million online tries to forge a pairing.
pub struct PairingCandidate {
    channel: Channel,
    local: [u8; 32],
    deadline: u64,
}
impl PairingCandidate {
    pub fn context(&self) -> &Context {
        self.channel.context()
    }
    pub fn comparison(&self) -> [u8; 20] {
        let mut h = Sha256::new();
        h.update(b"OpenKeys/R05/pairing-comparison/v1");
        h.update(self.context().transcript());
        let hash = h.finalize();
        let mut out = [0; 20];
        for (i, b) in hash[..10].iter().enumerate() {
            out[2 * i] = b"0123456789ABCDEF"[(b >> 4) as usize];
            out[2 * i + 1] = b"0123456789ABCDEF"[(b & 15) as usize];
        }
        out
    }
    /// Exact local approval must include peer key, transcript, transport and
    /// pairing comparison. No host display alone authenticates a new key.
    pub fn pairing_request(&self) -> Request {
        let mut h = Sha256::new();
        h.update(b"OpenKeys/R05/pair-host/v1");
        let (manager, key) = match self.channel.role {
            Role::Manager => (self.local, self.context().peer()),
            Role::Key => (self.context().peer(), self.local),
        };
        h.update(manager);
        h.update(key);
        h.update(self.context().transcript());
        h.update([self.context().link().transport_byte()]);
        Request {
            operation: Operation::PairHost,
            digest: h.finalize().into(),
            channel: self.context().transcript(),
        }
    }
    pub fn accept_known(
        mut self,
        store: &mut impl PeerStore,
        link: NativeLink,
        mut clock: impl FnMut() -> u64,
    ) -> Result<Channel, Error> {
        self.channel.check(link, clock())?;
        if !store.contains(&self.context().peer())? {
            return Err(Error::Peer);
        }
        self.channel.check(link, clock())?;
        Ok(self.channel)
    }
    /// Root-only local pairing workflow. `Authorization` must originate from
    /// six local digits + full comparison review, never a manager Boolean.
    /// Caller must recheck native link/time around each blocking store write.
    pub fn accept_new(
        mut self,
        authorization: Authorization,
        state: &mut impl State,
        store: &mut impl PeerStore,
        link: NativeLink,
        mut clock: impl FnMut() -> u64,
    ) -> Result<(Channel, Completed), Error> {
        if self.channel.role != Role::Key || clock() >= self.deadline {
            return Err(Error::Pairing);
        }
        let request = self.pairing_request();
        let mut execution = authorization
            .activate(&request, state, &mut clock)
            .map_err(|_| Error::Authorization)?;
        self.channel.check(link, clock())?;
        execution
            .check(&request, state, &mut clock)
            .map_err(|_| Error::Authorization)?;
        store.insert(&self.context().peer())?;
        self.channel.check(link, clock())?;
        execution
            .check(&request, state, &mut clock)
            .map_err(|_| Error::Authorization)?;
        if !store.contains(&self.context().peer())? {
            return Err(Error::Persistence);
        }
        self.channel.check(link, clock())?;
        let completed = execution
            .finish(&request, state, clock)
            .map_err(|_| Error::Authorization)?;
        Ok((self.channel, completed))
    }
    pub fn accept_new_manager(
        mut self,
        store: &mut impl PeerStore,
        review: &mut impl ManagerPairingReview,
        link: NativeLink,
        mut clock: impl FnMut() -> u64,
    ) -> Result<Channel, Error> {
        if self.channel.role != Role::Manager {
            return Err(Error::Pairing);
        }
        self.channel.check(link, clock())?;
        review.compare_with_key(&self.comparison(), &self.context().peer())?;
        self.channel.check(link, clock())?;
        store.insert(&self.context().peer())?;
        self.channel.check(link, clock())?;
        if !store.contains(&self.context().peer())? {
            return Err(Error::Persistence);
        }
        self.channel.check(link, clock())?;
        Ok(self.channel)
    }
}
