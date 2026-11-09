//! Actual two-bank W25Q64 mapping for device-vault. This adapter handles only
//! ciphertext. The SE-backed ledger and AEAD authority are separate and must
//! never be replaced by an ordinary Flash or host-supplied counter.
use crate::flash::{CAPACITY, Flash, PAGE, SECTOR};
use device_vault::{BANK_BYTES, CiphertextStorage, Error, SECTOR_BYTES};
use embedded_hal::{delay::DelayNs, spi::SpiDevice};

pub struct Storage<S, D> {
    flash: Flash<S>,
    delay: D,
    faulted: bool,
}

fn address(bank: u8, index: u32) -> Result<u32, Error> {
    if bank >= 2 || index >= BANK_BYTES / SECTOR {
        return Err(Error::Invalid);
    }
    let location = u32::from(bank) * BANK_BYTES + index * SECTOR;
    if location
        .checked_add(SECTOR)
        .is_none_or(|end| end > CAPACITY)
    {
        return Err(Error::Invalid);
    }
    Ok(location)
}

impl<S: SpiDevice, D: DelayNs> Storage<S, D> {
    pub fn new(mut flash: Flash<S>, mut delay: D) -> Result<Self, Error> {
        flash.identify(&mut delay).map_err(|_| Error::Storage)?;
        Ok(Self {
            flash,
            delay,
            faulted: false,
        })
    }
}

impl<S: SpiDevice, D: DelayNs> CiphertextStorage for Storage<S, D> {
    fn read(&mut self, bank: u8, index: u32, sector: &mut [u8; SECTOR_BYTES]) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::Storage);
        }
        let start = address(bank, index)?;
        for (page, output) in sector.as_chunks_mut::<PAGE>().0.iter_mut().enumerate() {
            if self
                .flash
                .read(start + (page * PAGE) as u32, output, &mut self.delay)
                .is_err()
            {
                self.faulted = true;
                return Err(Error::Storage);
            }
        }
        Ok(())
    }
    fn write(&mut self, bank: u8, index: u32, sector: &[u8; SECTOR_BYTES]) -> Result<(), Error> {
        if self.faulted {
            return Err(Error::Storage);
        }
        let start = address(bank, index)?;
        if self.flash.erase_sector(start, &mut self.delay).is_err() {
            self.faulted = true;
            return Err(Error::Storage);
        }
        for (page, input) in sector.as_chunks::<PAGE>().0.iter().enumerate() {
            if self
                .flash
                .program_page(start + (page * PAGE) as u32, input, &mut self.delay)
                .is_err()
            {
                self.faulted = true;
                return Err(Error::Storage);
            }
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), Error> {
        // Every Flash erase/program already waits for BUSY=0 and verifies
        // physical readback. There is no unflushed volatile write cache here.
        if self.faulted {
            Err(Error::Storage)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bank_boundaries_never_alias_or_wrap_into_the_other_bank() {
        assert_eq!(address(0, 0), Ok(0));
        assert_eq!(address(0, 1023), Ok(BANK_BYTES - SECTOR));
        assert_eq!(address(1, 0), Ok(BANK_BYTES));
        assert_eq!(address(1, 1023), Ok(CAPACITY - SECTOR));
        for bank in [0, 1, 2, u8::MAX] {
            for index in [1024, u32::MAX] {
                assert_eq!(address(bank, index), Err(Error::Invalid));
            }
        }
        assert_eq!(address(2, 0), Err(Error::Invalid));
    }
}
