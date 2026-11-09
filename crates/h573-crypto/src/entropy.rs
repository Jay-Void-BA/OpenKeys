//! Checked blocking entropy: faults never trigger reset/recovery/retry as a
//! successful draw. Hardware noise/clock/seed flags and repetition are fatal
//! for this owner. This is not a substitute for the silicon health tests or
//! measured entropy/fault qualification of the real clock configuration.
use crate::{Error, MAX_POLLS};

pub trait Source {
    /// Check clock/seed/current/latched flags BEFORE and AFTER reading DR.
    /// None means healthy but not ready. Error must not clear a fault flag.
    fn poll(&mut self) -> Result<Option<u32>, Error>;
}
pub struct CheckedRng<S: Source> {
    source: S,
    previous: u32,
    faulted: bool,
}
impl<S: Source> CheckedRng<S> {
    pub fn new(mut source: S) -> Result<Self, Error> {
        // Establish/discard a comparison baseline before returning any bits.
        let previous = draw(&mut source)?;
        Ok(Self {
            source,
            previous,
            faulted: false,
        })
    }
    pub fn fill(&mut self, output: &mut [u8]) -> Result<(), Error> {
        output.fill(0);
        if self.faulted {
            return Err(Error::Faulted);
        }
        let result = (|| {
            for chunk in output.chunks_mut(4) {
                let word = draw(&mut self.source)?;
                if word == self.previous {
                    return Err(Error::Hardware);
                }
                self.previous = word;
                chunk.copy_from_slice(&word.to_le_bytes()[..chunk.len()]);
            }
            Ok(())
        })();
        if result.is_err() {
            self.faulted = true;
            output.fill(0);
        }
        result
    }
}
fn draw(source: &mut impl Source) -> Result<u32, Error> {
    for _ in 0..MAX_POLLS {
        if let Some(word) = source.poll()? {
            return Ok(word);
        }
    }
    Err(Error::Timeout)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Script {
        at: usize,
        error: bool,
        stuck: bool,
        timeout: bool,
    }
    impl Source for Script {
        fn poll(&mut self) -> Result<Option<u32>, Error> {
            if self.timeout {
                return Ok(None);
            }
            self.at += 1;
            if self.error && self.at == 3 {
                return Err(Error::Hardware);
            }
            Ok(Some(if self.stuck {
                0x12345678
            } else {
                self.at as u32
            }))
        }
    }
    #[test]
    fn baseline_is_discarded_and_partial_output_is_erased_on_any_fault() {
        let mut good = CheckedRng::new(Script {
            at: 0,
            error: false,
            stuck: false,
            timeout: false,
        })
        .unwrap();
        let mut output = [0; 8];
        good.fill(&mut output).unwrap();
        assert_eq!(output, [2, 0, 0, 0, 3, 0, 0, 0]);
        for stuck in [false, true] {
            let mut rng = CheckedRng::new(Script {
                at: 0,
                error: !stuck,
                stuck,
                timeout: false,
            })
            .unwrap();
            assert_eq!(rng.fill(&mut output), Err(Error::Hardware));
            assert_eq!(output, [0; 8]);
            assert_eq!(rng.fill(&mut output), Err(Error::Faulted));
        }
    }
    #[test]
    fn unavailable_entropy_is_bounded_without_a_fallback_seed() {
        assert!(matches!(
            CheckedRng::new(Script {
                at: 0,
                error: false,
                stuck: false,
                timeout: true
            }),
            Err(Error::Timeout)
        ));
    }
}
