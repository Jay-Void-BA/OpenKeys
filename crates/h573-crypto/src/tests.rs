extern crate std;
use super::*;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes_gcm::aead::AeadInPlace;
use cmac::{Cmac, Mac};
use std::{cell::RefCell, rc::Rc, vec::Vec};

#[derive(Default)]
struct Model {
    cr: u32,
    sr: u32,
    flags: u32,
    key: [u32; 8],
    iv: [u32; 4],
    words: Vec<u32>,
    output: Vec<u32>,
    aad: Vec<u8>,
    plain: Vec<u8>,
    cipher: Vec<u8>,
    previous: [u8; 16],
    counter: u32,
    clear_values: Vec<u32>,
    key_writes: Vec<u8>,
    loaded_iv: Option<[u32; 4]>,
    loaded_control: u32,
    reset_count: usize,
    inject_read: bool,
    inject_key: bool,
    inject_clear: bool,
    fail_key_ready: bool,
    device_context: Option<device_key::Snapshot>,
    context_checks: usize,
    context_fault_at: Option<usize>,
    context_change_at: Option<usize>,
    dhuk_controls: Vec<u32>,
}
#[derive(Clone, Default)]
pub(crate) struct Port(Rc<RefCell<Model>>);

impl Model {
    fn keys(&self) -> Vec<u8> {
        if self.cr & KEYSEL == 1 << 28 {
            // Independent synthetic test key, NOT the H573 hardware KDF.
            assert_eq!(self.cr & !EN, DHUK_CONTROL);
            return std::vec![0x57; 32];
        }
        let count = if self.cr & (1 << 18) != 0 { 8 } else { 4 };
        (0..count)
            .rev()
            .flat_map(|i| self.key[i].to_be_bytes())
            .collect()
    }
    fn iv_bytes(&self) -> [u8; 16] {
        core::array::from_fn(|i| self.iv[3 - i / 4].to_be_bytes()[i % 4])
    }
    fn cipher_block(&self, input: &[u8; 16], decrypt: bool) -> [u8; 16] {
        let key = self.keys();
        let mut block = (*input).into();
        if key.len() == 16 {
            let aes = aes::Aes128::new_from_slice(&key).unwrap();
            if decrypt {
                aes.decrypt_block(&mut block)
            } else {
                aes.encrypt_block(&mut block)
            }
        } else {
            let aes = aes::Aes256::new_from_slice(&key).unwrap();
            if decrypt {
                aes.decrypt_block(&mut block)
            } else {
                aes.encrypt_block(&mut block)
            }
        }
        block.into()
    }
    fn produce(&mut self, input: [u8; 16]) {
        let mode = (self.cr >> 5) & 3;
        let direction = (self.cr >> 3) & 3;
        let output = if mode == 0 {
            self.cipher_block(&input, direction == 2)
        } else if mode == 1 {
            if direction == 2 {
                let decoded = self.cipher_block(&input, true);
                let output = core::array::from_fn(|i| decoded[i] ^ self.previous[i]);
                self.previous = input;
                output
            } else {
                let mixed = core::array::from_fn(|i| input[i] ^ self.previous[i]);
                let output = self.cipher_block(&mixed, false);
                self.previous = output;
                output
            }
        } else if mode == 3 {
            match (self.cr >> 13) & 3 {
                1 => {
                    self.aad.extend_from_slice(&input);
                    [0; 16]
                }
                2 => {
                    let mut counter = self.iv_bytes();
                    counter[12..].copy_from_slice(&self.counter.to_be_bytes());
                    self.counter += 1;
                    let mask = self.cipher_block(&counter, false);
                    let output = core::array::from_fn(|i| input[i] ^ mask[i]);
                    if direction == 2 {
                        self.cipher.extend_from_slice(&input);
                        self.plain.extend_from_slice(&output);
                    } else {
                        self.plain.extend_from_slice(&input);
                        let mut authenticated = output;
                        let padding = ((self.cr >> 20) & 15) as usize;
                        authenticated[16 - padding..].fill(0);
                        self.cipher.extend_from_slice(&authenticated);
                    }
                    output
                }
                3 => {
                    // Independent AES-GCM oracle sees the lengths encoded by
                    // the driver; assertions also catch wrong phase contents.
                    let a = u64::from_be_bytes(input[..8].try_into().unwrap()) as usize / 8;
                    let p = u64::from_be_bytes(input[8..].try_into().unwrap()) as usize / 8;
                    assert!(a <= self.aad.len() && p <= self.plain.len());
                    assert!(self.aad[a..].iter().all(|b| *b == 0));
                    assert!(
                        self.cipher[p..].iter().all(|b| *b == 0),
                        "incorrect GCM last-block padding"
                    );
                    let iv = self.iv_bytes();
                    assert_eq!(&iv[12..], &[0, 0, 0, 2]);
                    let mut plain = self.plain[..p].to_vec();
                    let key = self.keys();
                    let tag = if key.len() == 16 {
                        aes_gcm::Aes128Gcm::new_from_slice(&key)
                            .unwrap()
                            .encrypt_in_place_detached(
                                (&iv[..12]).into(),
                                &self.aad[..a],
                                &mut plain,
                            )
                            .unwrap()
                    } else {
                        aes_gcm::Aes256Gcm::new_from_slice(&key)
                            .unwrap()
                            .encrypt_in_place_detached(
                                (&iv[..12]).into(),
                                &self.aad[..a],
                                &mut plain,
                            )
                            .unwrap()
                    };
                    assert_eq!(plain, &self.cipher[..p]);
                    tag.into()
                }
                _ => panic!("unexpected GCM block before init completed"),
            }
        } else {
            panic!("unsupported model mode")
        };
        self.output = output
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_be_bytes(*b))
            .collect();
        self.flags |= CCF;
    }
}
impl Registers for Port {
    fn read(&mut self, r: Register) -> u32 {
        let mut m = self.0.borrow_mut();
        match r {
            Register::Cr => m.cr,
            Register::Sr => m.sr,
            Register::Isr => m.flags,
            Register::Dout => {
                assert_ne!(m.flags & CCF, 0);
                if m.inject_read {
                    m.flags |= 8;
                }
                m.output.remove(0)
            }
            _ => panic!("invalid register read"),
        }
    }
    fn write(&mut self, r: Register, value: u32) {
        let mut m = self.0.borrow_mut();
        match r {
            Register::Cr => {
                assert_eq!(m.sr & BUSY, 0, "control write while busy");
                let previous = m.cr;
                m.cr = value;
                if value & KEYSEL != previous & KEYSEL {
                    assert_eq!(previous & EN, 0, "KEYSEL change while enabled");
                    m.sr &= !KEYVALID;
                    m.key.fill(0);
                    if value & KEYSEL == 1 << 28 {
                        m.dhuk_controls.push(value);
                        if !m.fail_key_ready {
                            m.sr |= KEYVALID;
                        }
                        if m.inject_key {
                            m.flags |= 4;
                        }
                    }
                }
                if value & IPRST != 0 {
                    assert_eq!(value & KEYSEL, 0, "hardware key reload during cleanup");
                    m.sr = 0;
                    m.flags = 0;
                    m.key.fill(0);
                    m.iv.fill(0);
                    m.words.clear();
                    m.output.clear();
                    m.aad.clear();
                    m.plain.clear();
                    m.cipher.clear();
                    m.reset_count += 1;
                } else if value & EN != 0 {
                    assert_ne!(m.sr & KEYVALID, 0, "enable before key valid");
                    if ((value >> 3) & 3) == 1
                        || ((value >> 5) & 3) == 3 && ((value >> 13) & 3) == 0
                    {
                        m.cr &= !EN;
                        m.flags |= CCF;
                    }
                    if value & EN != 0 && previous & EN == 0 {
                        m.previous = m.iv_bytes();
                        m.counter = m.iv[0];
                    }
                }
            }
            Register::Ier => assert_eq!(value, 0),
            Register::Icr => {
                m.clear_values.push(value);
                m.flags &= !value;
                if m.inject_clear {
                    m.flags |= 8;
                }
            }
            Register::Key(index) => {
                assert_eq!(m.cr & EN, 0);
                assert_ne!(m.cr & KEYPROT, 0);
                m.loaded_control = m.cr;
                m.key[index as usize] = value;
                m.key_writes.push(index);
                if index == 0 && !m.fail_key_ready {
                    m.sr |= KEYVALID;
                }
                if m.inject_key {
                    m.flags |= 4;
                }
            }
            Register::Iv(index) => {
                m.iv[index as usize] = value;
                m.loaded_iv = Some(m.iv);
            }
            Register::Din => {
                assert_ne!(m.cr & EN, 0);
                m.words.push(value);
                if m.words.len() == 4 {
                    let block = core::array::from_fn(|i| m.words[i / 4].to_be_bytes()[i % 4]);
                    m.words.clear();
                    m.produce(block);
                }
            }
            _ => panic!("invalid register write"),
        }
    }
    fn device_key_context(&mut self) -> Result<DeviceKeyContext, Error> {
        let mut m = self.0.borrow_mut();
        m.context_checks += 1;
        if m.context_fault_at == Some(m.context_checks) {
            return Err(Error::Hardware);
        }
        if m.context_change_at == Some(m.context_checks)
            && let Some(context) = m.device_context.as_mut()
        {
            context.flash_secure_epoch += 1;
        }
        m.device_context.ok_or(Error::Hardware)?.qualify()
    }
    fn emergency_reset(&mut self) -> ! {
        panic!("emergency whole-device reset required")
    }
}

pub(crate) fn engine() -> (MaskedAes<Port>, Port) {
    let port = Port::default();
    let engine = MaskedAes::new(port.clone()).unwrap();
    (engine, port)
}
fn assert_erased(port: &Port) {
    let m = port.0.borrow();
    assert_eq!(m.sr & KEYVALID, 0);
    assert_eq!(m.cr & EN, 0);
    assert_eq!(m.cr & KEYSEL, 0);
    assert!(m.key.iter().all(|v| *v == 0));
    assert!(m.clear_values.iter().all(|v| *v == CCF));
}

fn dhuk_engine() -> (MaskedAes<Port>, Port) {
    let (aes, port) = engine();
    port.0.borrow_mut().device_context = Some(device_key::fixture());
    (aes, port)
}

#[test]
fn dhuk_cmac_matches_independent_oracle_without_key_register_io_or_mode_switch() {
    let (mut aes, port) = dhuk_engine();
    for length in [0, 1, 15, 16, 17, 31, 32, 33, 224, 256, 4080, 4096] {
        let message: Vec<_> = (0..length).map(|i| (i * 37) as u8).collect();
        let mut oracle = <Cmac<aes::Aes256> as Mac>::new_from_slice(&[0x57; 32]).unwrap();
        oracle.update(&message);
        let expected = oracle.finalize().into_bytes();
        let mut out = [0x55; 16];
        aes.dhuk_cmac(&message, &mut out).unwrap();
        assert_eq!(out.as_slice(), expected.as_slice());
        assert_erased(&port);
        let m = port.0.borrow();
        assert!(m.key_writes.is_empty());
        assert!(m.dhuk_controls.iter().all(|&c| c == DHUK_CONTROL));
    }
    assert_eq!(port.0.borrow().dhuk_controls.len(), 24);
}

#[test]
fn every_dhuk_context_fault_or_change_including_after_cleanup_rejects_output() {
    let (mut baseline, port) = dhuk_engine();
    baseline.dhuk_cmac(&[0x39; 33], &mut [0; 16]).unwrap();
    let checks = port.0.borrow().context_checks;
    assert!(checks > 10);
    for change in [false, true] {
        for at in 1..=checks {
            let (mut aes, port) = dhuk_engine();
            if change {
                port.0.borrow_mut().context_change_at = Some(at);
            } else {
                port.0.borrow_mut().context_fault_at = Some(at);
            }
            let mut out = [0x55; 16];
            let result = aes.dhuk_cmac(&[0x39; 33], &mut out);
            // The very first valid context establishes the lifetime identity;
            // there is no earlier epoch for this component to compare with.
            // Root's journal/SE owner binding must reject stale persistent data.
            if change && at == 1 {
                assert!(result.is_ok());
            } else {
                assert!(result.is_err(), "context change={change}, checkpoint={at}");
                assert_eq!(out, [0; 16]);
                assert_eq!(
                    aes.ecb(&[0; 16], Direction::Encrypt, &[0; 16], &mut out),
                    Err(Error::Faulted)
                );
                assert_eq!(out, [0; 16]);
            }
            assert_erased(&port);
        }
    }
}

#[test]
fn dhuk_absent_unqualified_and_changed_valid_lifecycle_never_falls_back() {
    let (mut absent, port) = engine();
    let mut out = [0x55; 16];
    assert_eq!(absent.dhuk_cmac(b"journal", &mut out), Err(Error::Hardware));
    assert_eq!(out, [0; 16]);
    assert!(port.0.borrow().key_writes.is_empty());
    assert!(port.0.borrow().dhuk_controls.is_empty());
    assert_erased(&port);
    for mutation in [0, 1, 2] {
        let (mut aes, port) = dhuk_engine();
        aes.dhuk_cmac(b"journal", &mut out).unwrap();
        let mut m = port.0.borrow_mut();
        let context = m.device_context.as_mut().unwrap();
        match mutation {
            0 => context.flash_secure_epoch += 1,
            1 => context.flash_options ^= (0x72 ^ 0x5c) << 8,
            _ => context.tamper_status = 1,
        }
        drop(m);
        assert_eq!(aes.dhuk_cmac(b"journal", &mut out), Err(Error::Hardware));
        assert_eq!(out, [0; 16]);
        assert_erased(&port);
    }
}

#[test]
fn dhuk_load_output_clear_faults_and_key_timeout_poison_and_erase() {
    for fault in 0..4 {
        let (mut aes, port) = dhuk_engine();
        let mut m = port.0.borrow_mut();
        match fault {
            0 => m.inject_key = true,
            1 => m.inject_read = true,
            2 => m.inject_clear = true,
            _ => m.fail_key_ready = true,
        }
        drop(m);
        let mut out = [0x55; 16];
        assert!(aes.dhuk_cmac(b"journal", &mut out).is_err());
        assert_eq!(out, [0; 16]);
        assert_eq!(aes.dhuk_cmac(b"journal", &mut out), Err(Error::Faulted));
        assert_erased(&port);
    }
    let (mut aes, port) = dhuk_engine();
    let mut out = [0x55; 16];
    assert_eq!(aes.dhuk_cmac(&[0; 4097], &mut out), Err(Error::Input));
    assert_eq!(out, [0; 16]);
    assert!(port.0.borrow().dhuk_controls.is_empty());
    assert_erased(&port);
}

#[test]
fn ecb_key_order_and_nist_known_answer() {
    let (mut aes, port) = engine();
    let mut output = [0; 16];
    let key = core::array::from_fn::<_, 16, _>(|i| i as u8);
    let input = [
        0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
    ];
    aes.ecb(&key, Direction::Encrypt, &input, &mut output)
        .unwrap();
    assert_eq!(
        output,
        [
            0x69, 0xc4, 0xe0, 0xd8, 0x6a, 0x7b, 0x04, 0x30, 0xd8, 0xcd, 0xb7, 0x80, 0x70, 0xb4,
            0xc5, 0x5a
        ]
    );
    assert_eq!(port.0.borrow().key_writes, &[3, 2, 1, 0]);
    assert_erased(&port);
    let ciphertext = output;
    aes.ecb(&key, Direction::Decrypt, &ciphertext, &mut output)
        .unwrap();
    assert_eq!(output, input);
    assert_erased(&port);
}

#[test]
fn cbc_and_cmac_match_independent_rustcrypto_for_boundaries() {
    let key = core::array::from_fn::<_, 32, _>(|i| i as u8);
    let iv = core::array::from_fn::<_, 16, _>(|i| i as u8 + 64);
    let (mut aes, port) = engine();
    for length in [0, 1, 15, 16, 17, 31, 32, 48, 287, 4096] {
        let message: Vec<u8> = (0..length).map(|i| i as u8).collect();
        let mut actual = [0; 16];
        aes.cmac(&key, &message, &mut actual).unwrap();
        let mut mac = <Cmac<aes::Aes256> as Mac>::new_from_slice(&key).unwrap();
        mac.update(&message);
        assert_eq!(actual, &mac.finalize().into_bytes()[..]);
        assert_erased(&port);
    }
    let data: Vec<u8> = (0..240).map(|i| i as u8).collect();
    let mut cipher = data.clone();
    aes.cbc(&key, Direction::Encrypt, &iv, &mut cipher).unwrap();
    assert_ne!(cipher, data);
    assert_eq!(
        port.0.borrow().loaded_iv,
        Some([0x4c4d4e4f, 0x48494a4b, 0x44454647, 0x40414243])
    );
    aes.cbc(&key, Direction::Decrypt, &iv, &mut cipher).unwrap();
    assert_eq!(cipher, data);
    assert_erased(&port);
}

#[test]
fn gcm_iv_registers_and_nist_vector_are_exact() {
    let (mut aes, port) = engine();
    let mut bytes = [0; 16];
    let tag = aes
        .gcm(&[0; 16], Direction::Encrypt, &[0; 12], &[], &mut bytes)
        .unwrap();
    assert_eq!(
        bytes,
        [
            0x03, 0x88, 0xda, 0xce, 0x60, 0xb6, 0xa3, 0x92, 0xf3, 0x28, 0xc2, 0xb9, 0x71, 0xb2,
            0xfe, 0x78
        ]
    );
    assert_eq!(
        tag,
        [
            0xab, 0x6e, 0x47, 0xd4, 0x2c, 0xec, 0x13, 0xbd, 0xf5, 0x3a, 0x67, 0xb2, 0x12, 0x57,
            0xbd, 0xdf
        ]
    );
    assert_eq!(port.0.borrow().loaded_iv, Some([2, 0, 0, 0]));
    assert_erased(&port);
    let key = core::array::from_fn::<_, 32, _>(|i| i as u8);
    let nonce = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let mut bytes = [0x5a; 12];
    let before = bytes;
    let tag = aes
        .gcm(&key, Direction::Encrypt, &nonce, b"eightAAD", &mut bytes)
        .unwrap();
    assert_eq!(
        port.0.borrow().loaded_iv,
        Some([2, 0x090a0b0c, 0x05060708, 0x01020304])
    );
    let decrypt_tag = aes
        .gcm(&key, Direction::Decrypt, &nonce, b"eightAAD", &mut bytes)
        .unwrap();
    assert_eq!(tag, decrypt_tag);
    assert_eq!(bytes, before);
    assert_erased(&port);
}

#[test]
fn aligned_vault_gcm_and_partial_lengths_match_independent_oracle() {
    let key = [0x81; 32];
    let nonce = [0x42; 12];
    let (mut aes, port) = engine();
    for (a, p) in [(0, 0), (8, 12), (16, 16), (48, 4080), (17, 33)] {
        let aad = std::vec![0x30;a];
        let mut plain = std::vec![0x55;p];
        let mut expected = plain.clone();
        let tag = aes_gcm::Aes256Gcm::new_from_slice(&key)
            .unwrap()
            .encrypt_in_place_detached((&nonce).into(), &aad, &mut expected)
            .unwrap();
        let actual = aes
            .gcm(&key, Direction::Encrypt, &nonce, &aad, &mut plain)
            .unwrap();
        assert_eq!(plain, expected);
        assert_eq!(actual, &tag[..]);
        let returned = aes
            .gcm(&key, Direction::Decrypt, &nonce, &aad, &mut plain)
            .unwrap();
        assert_eq!(returned, actual);
        assert_eq!(plain, std::vec![0x55;p]);
        assert_erased(&port);
    }
}

#[test]
fn rng_key_read_faults_and_timeouts_never_disappear_under_ccf_clear() {
    for fault in 0..3 {
        let (mut aes, port) = engine();
        if fault == 0 {
            port.0.borrow_mut().inject_key = true;
        } else if fault == 1 {
            port.0.borrow_mut().inject_read = true;
        } else {
            port.0.borrow_mut().inject_clear = true;
        }
        let mut output = [0x55; 16];
        assert_eq!(
            aes.ecb(&[1; 16], Direction::Encrypt, &[2; 16], &mut output),
            Err(Error::Hardware)
        );
        assert_eq!(output, [0; 16]);
        assert_erased(&port);
        assert_eq!(
            aes.ecb(&[1; 16], Direction::Encrypt, &[2; 16], &mut output),
            Err(Error::Faulted)
        );
    }
    let (mut aes, port) = engine();
    port.0.borrow_mut().fail_key_ready = true;
    let mut output = [1; 16];
    assert_eq!(
        aes.ecb(&[1; 16], Direction::Encrypt, &[2; 16], &mut output),
        Err(Error::Timeout)
    );
    assert_eq!(output, [0; 16]);
    assert_erased(&port);
}
