//! Strict normal-profile ATR parsing. ATR is untrusted capability data, not
//! device identity. A malformed/unknown profile never becomes default timing.
use crate::{Error, MAX_INF};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Atr {
    pub block_wait_us: u32,
    pub ifsc: usize,
    pub max_clock_khz: u16,
    pub poll_us: u32,
    pub guard_us: u32,
    pub wake_us: u32,
}

impl Atr {
    /// expected_version is part of the qualified firmware/chip profile, never
    /// selected by a host packet. The published SE050 example uses version0.
    pub fn parse(bytes: &[u8], expected_version: u8) -> Result<Self, Error> {
        if bytes.len() < 7 || bytes[0] != expected_version || bytes[1..6] != [0xa0, 0, 0, 3, 0x96] {
            return Err(Error::Control);
        }
        let link_len = usize::from(bytes[6]);
        if link_len != 4 {
            return Err(Error::Length);
        }
        let link = bytes.get(7..11).ok_or(Error::Length)?;
        let rest = bytes.get(11..).ok_or(Error::Length)?;
        if rest.len() < 2 || rest[0] != 2 {
            return Err(Error::Control);
        }
        let physical_len = usize::from(rest[1]);
        if physical_len != 11 {
            return Err(Error::Length);
        }
        let physical = rest.get(2..13).ok_or(Error::Length)?;
        let remaining = rest.get(13..).ok_or(Error::Length)?;
        if remaining.is_empty() || remaining.len() != 1 + usize::from(remaining[0]) {
            return Err(Error::Length);
        }
        let bwt = u16::from_be_bytes([link[0], link[1]]);
        let ifsc = u16::from_be_bytes([link[2], link[3]]);
        let clock = u16::from_be_bytes([physical[0], physical[1]]);
        let poll = physical[3];
        let guard = u16::from_be_bytes([physical[7], physical[8]]);
        let wake = u16::from_be_bytes([physical[9], physical[10]]);
        // Support only the qualified normal-length100kHz profile. Unknown
        // extensions, zero divisors or attacker-sized delays are rejected.
        if !(1..=5000).contains(&bwt)
            || !(16..=MAX_INF as u16).contains(&ifsc)
            || clock < 100
            || !(1..=100).contains(&poll)
            || guard > 10_000
            || wake > 10_000
            || physical[4..7] != [0, 0, 0]
        {
            return Err(Error::Control);
        }
        Ok(Self {
            block_wait_us: u32::from(bwt) * 1000,
            ifsc: usize::from(ifsc),
            max_clock_khz: clock,
            poll_us: u32::from(poll) * 1000,
            guard_us: u32::from(guard).max(10),
            wake_us: u32::from(wake),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const EXAMPLE: &[u8] = &[
        0, 0xa0, 0, 0, 3, 0x96, 4, 3, 0xe8, 0, 0xfe, 2, 11, 3, 0xe8, 8, 1, 0, 0, 0, 0, 100, 0, 0,
        10, b'J', b'C', b'O', b'P', b'4', b' ', b'A', b'T', b'P', b'O',
    ];
    #[test]
    fn exact_vendor_example_has_bounded_timing() {
        let a = Atr::parse(EXAMPLE, 0).unwrap();
        assert_eq!(a.ifsc, 254);
        assert_eq!(a.block_wait_us, 1_000_000);
        assert_eq!(a.poll_us, 1000);
        assert_eq!(a.guard_us, 100);
        assert!(Atr::parse(EXAMPLE, 1).is_err());
    }
    #[test]
    fn truncation_bad_lengths_zero_poll_and_unknown_profile_never_fall_back() {
        for len in 0..EXAMPLE.len() {
            assert!(Atr::parse(&EXAMPLE[..len], 0).is_err());
        }
        for (index, value) in [
            (0, 2),
            (6, 0),
            (6, 255),
            (12, 0),
            (12, 255),
            (16, 0),
            (20, 255),
            (24, 255),
            (1, 0xff),
        ] {
            let mut changed = [0; 35];
            changed.copy_from_slice(EXAMPLE);
            changed[index] = value;
            assert!(Atr::parse(&changed, 0).is_err());
        }
    }
}
