use crate::request::Dialect;
use crate::{Error, Prepared, Review, prepare_ctap2};
use alloc::vec::Vec;
use device_policy::Operation;
use device_policy::{
    Authorization,
    execution::{Execution, State as ExecutionState},
};
use opensk::{api::customization::Customization, ctap::CtapState, env::Env};
use zeroize::{Zeroize, Zeroizing};

/// Implement only inside the verified root. Counter, key, persistence and
/// fingerprint providers must be real hardware adapters. No permissive default
/// or public simulation backend is supplied. End must revoke ALL scope state.
pub trait Platform: Env {
    fn check_boot_and_channel(&mut self, binding: &[u8; 32]) -> Result<(), Error>;
    fn authorization_generation(&self) -> u64;
    fn begin_authorized(&mut self, execution: Execution) -> Result<(), Error>;
    fn end_authorized(&mut self);
    /// Root-owned crash-safe FIDO reset. Do not invoke OpenSK's generic reset:
    /// it can delete shared sensor templates before the vault/SE transaction.
    /// Device-level notes/fingerprint lifecycle is separate from CTAP reset.
    fn reset_fido_authorized(&mut self) -> Result<Vec<u8>, Error>;
}
struct Activation<'a, E> {
    env: &'a mut E,
    binding: [u8; 32],
}
impl<E: Platform> ExecutionState for Activation<'_, E> {
    fn generation(&self) -> u64 {
        self.env.authorization_generation()
    }
    fn check_security_state(&mut self) -> Result<(), device_policy::Error> {
        self.env
            .check_boot_and_channel(&self.binding)
            .map_err(|_| device_policy::Error::HardwareFault)
    }
}
pub struct Processor<E: Platform> {
    env: E,
    state: CtapState<E>,
    continuation: Option<Continuation>,
}
struct Continuation {
    binding: [u8; 32],
    channel: opensk::ctap::Channel,
    rp: alloc::string::String,
    seed: [u8; 32],
    sequence: u32,
    remaining: u64,
    issued: u64,
    expires: u64,
    operation: Operation,
}
impl<E: Platform> Processor<E> {
    /// The root validates commissioned namespace/boot before construction.
    /// Persist::init must not recreate keys on a corrupt provisioned device.
    pub fn new(mut env: E) -> Self {
        let state = CtapState::new(&mut env);
        Self {
            env,
            state,
            continuation: None,
        }
    }
    /// Continuations retain their original RP/channel/transaction digest and
    /// deadline, then require a NEW device authorization for each next result.
    pub fn prepare(
        &mut self,
        raw: &[u8],
        binding: [u8; 32],
        channel: opensk::ctap::Channel,
        now: u64,
    ) -> Result<Prepared, Error> {
        if raw != [8] {
            return prepare_ctap2(raw, binding, channel);
        }
        let c = self.continuation.as_ref().ok_or(Error::Authorization)?;
        if c.binding != binding || c.channel != channel || now < c.issued || now >= c.expires {
            return Err(Error::Authorization);
        }
        Ok(crate::request::next_request(
            binding,
            channel,
            c.rp.clone(),
            c.seed,
            c.sequence,
            c.operation,
        ))
    }
    pub fn public(&mut self, request: Prepared) -> Result<Zeroizing<Vec<u8>>, Error> {
        if request.request.is_some() {
            return Err(Error::Authorization);
        }
        // Public shape is narrowed by prepare_ctap2; callers cannot construct
        // Prepared or mutate its byte buffer outside this crate.
        self.env.end_authorized();
        self.continuation = None;
        self.env.check_boot_and_channel(&request.binding)?;
        let mut response = Zeroizing::new(self.dispatch(&request)?);
        self.env.check_boot_and_channel(&request.binding)?;
        if response.len() > 7609 {
            return Err(Error::Limit);
        }
        if request.dialect == Dialect::Ctap2
            && request.bytes[0] == 4
            && response.first() == Some(&0)
        {
            response = sanitize_info(&response)?;
        }
        Ok(response)
    }
    pub fn authorized(
        &mut self,
        request: Prepared,
        authorization: Authorization,
        mut clock: impl FnMut() -> u64,
    ) -> Result<Zeroizing<Vec<u8>>, Error> {
        let expected = request.request.as_ref().ok_or(Error::Authorization)?;
        if authorization.request() != expected {
            return Err(Error::Authorization);
        }
        if let Some((seed, sequence)) = request.continuation {
            let c = self.continuation.as_ref().ok_or(Error::Authorization)?;
            if c.seed != seed
                || c.sequence != sequence
                || c.binding != request.binding
                || c.channel != request.channel
            {
                return Err(Error::Authorization);
            }
        }
        let start = clock();
        if start < authorization.issued_ms() || start >= authorization.expires_ms() {
            return Err(Error::Time);
        }
        let continuation_expires = if request.continuation.is_some() {
            self.continuation.as_ref().map(|c| c.expires)
        } else {
            None
        };
        if request.continuation.is_some()
            && self
                .continuation
                .as_ref()
                .is_none_or(|c| start < c.issued || start >= c.expires)
        {
            return Err(Error::Time);
        }
        self.env.check_boot_and_channel(&expected.channel)?;
        let execution = authorization.activate(
            expected,
            &mut Activation {
                env: &mut self.env,
                binding: expected.channel,
            },
            &mut clock,
        );
        let execution = match execution {
            Ok(execution) => execution,
            Err(error) => {
                self.env.end_authorized();
                self.continuation = None;
                return Err(match error {
                    device_policy::Error::Expired | device_policy::Error::TimeWentBackwards => {
                        Error::Time
                    }
                    device_policy::Error::HardwareFault => Error::State,
                    _ => Error::Authorization,
                });
            }
        };
        let execution = if let Some(until) = continuation_expires {
            match execution.restrict_deadline(until) {
                Ok(execution) => execution,
                Err(_) => {
                    self.env.end_authorized();
                    self.continuation = None;
                    return Err(Error::Time);
                }
            }
        } else {
            execution
        };
        let issued = execution.started_ms();
        let activated_at = execution.checked_at_ms();
        let expires = execution.expires_ms();
        let generation = execution.authorization().generation();
        if let Err(error) = self.env.begin_authorized(execution) {
            self.env.end_authorized();
            return Err(error);
        }
        let before = clock();
        let same_generation = self.env.authorization_generation() == generation;
        if !same_generation
            || before < activated_at
            || before >= expires
            || continuation_expires.is_some_and(|until| before >= until)
        {
            self.env.end_authorized();
            self.continuation = None;
            return Err(if same_generation {
                Error::Time
            } else {
                Error::Authorization
            });
        }
        let previous = self.continuation.take();
        let mut dispatched = self.dispatch(&request);
        let checked = self.env.check_boot_and_channel(&expected.channel);
        let same_generation = self.env.authorization_generation() == generation;
        let after = clock();
        self.env.end_authorized();
        if checked.is_err()
            || !same_generation
            || after < before
            || after >= expires
            || continuation_expires.is_some_and(|until| after >= until)
        {
            if let Ok(response) = &mut dispatched {
                response.zeroize();
            }
            return Err(if !same_generation {
                Error::Authorization
            } else if checked.is_err() {
                Error::State
            } else {
                Error::Time
            });
        }
        let mut response = Zeroizing::new(dispatched?);
        if response.len() > 7609 {
            response.zeroize();
            return Err(Error::Limit);
        }
        if request.bytes[0] == 2 && response.first() == Some(&0) {
            let map = opensk::ctap::cbor_read(&response[1..])
                .map_err(|_| Error::State)?
                .extract_map()
                .ok_or(Error::State)?;
            let remaining = map
                .iter()
                .find(|(k, _)| *k == sk_cbor::Value::unsigned(5))
                .and_then(|(_, v)| v.clone().extract_unsigned())
                .unwrap_or(1);
            if remaining > self.env.customization().max_supported_resident_keys() as u64
                || remaining > device_records::MAX_RECORDS as u64
            {
                response.zeroize();
                return Err(Error::Limit);
            }
            if remaining > 1
                && let Review::Website(rp) = request.review()
            {
                self.continuation = Some(Continuation {
                    binding: request.binding,
                    channel: request.channel,
                    rp: rp.clone(),
                    seed: expected.digest,
                    sequence: 1,
                    remaining: remaining - 1,
                    issued,
                    expires,
                    operation: expected.operation,
                });
            }
        } else if request.bytes[0] == 8
            && response.first() == Some(&0)
            && let Some(mut c) = previous
        {
            c.remaining = c.remaining.checked_sub(1).ok_or(Error::State)?;
            c.sequence = c.sequence.checked_add(1).ok_or(Error::Limit)?;
            if c.remaining > 0 {
                self.continuation = Some(c);
            }
        }
        Ok(response)
    }
    fn dispatch(&mut self, request: &Prepared) -> Result<Vec<u8>, Error> {
        if request.dialect == Dialect::Ctap2 && request.bytes[0] == 7 {
            return self.env.reset_fido_authorized();
        }
        Ok(match request.dialect {
            Dialect::Ctap2 => {
                self.state
                    .process_command(&mut self.env, &request.bytes, request.channel)
            }
            Dialect::U2f => self
                .state
                .process_legacy_protected(&mut self.env, &request.bytes),
        })
    }
}

fn sanitize_info(response: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
    use sk_cbor::Value;
    let mut map = opensk::ctap::cbor_read(&response[1..])
        .map_err(|_| Error::State)?
        .extract_map()
        .ok_or(Error::State)?;
    // Remaining credential capacity reveals usage metadata before fingerprint.
    map.retain(|(k, _)| *k != Value::unsigned(20));
    // Do not advertise optional large-blob/PIN configuration APIs that the
    // root gateway intentionally does not expose.
    map.retain(|(k, _)| *k != Value::unsigned(11) && *k != Value::unsigned(13));
    if let Some((_, extensions)) = map.iter_mut().find(|(k, _)| *k == Value::unsigned(2)) {
        let mut list = extensions.clone().extract_array().ok_or(Error::State)?;
        list.retain(|v| {
            *v != Value::text_string("largeBlobKey".into())
                && *v != Value::text_string("minPinLength".into())
        });
        *extensions = Value::array(list);
    }
    if let Some((_, options)) = map.iter_mut().find(|(k, _)| *k == Value::unsigned(4)) {
        let mut pairs = options.clone().extract_map().ok_or(Error::State)?;
        pairs.retain(|(k, _)| {
            *k != Value::text_string("clientPin".into())
                && *k != Value::text_string("alwaysUv".into())
                && *k != Value::text_string("largeBlobs".into())
                && *k != Value::text_string("authnrCfg".into())
                && *k != Value::text_string("setMinPINLength".into())
        });
        pairs.push((
            Value::text_string("alwaysUv".into()),
            Value::bool_value(true),
        ));
        *options = Value::map(pairs);
    }
    let mut result = Zeroizing::new(alloc::vec![0]);
    opensk::ctap::cbor_write(Value::map(map), &mut result).map_err(|_| Error::State)?;
    Ok(result)
}
