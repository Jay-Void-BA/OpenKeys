//! Allocation-free resource gate before the upstream canonical CBOR decoder.
use crate::Error;

pub(crate) const MAX_BYTES: usize = 7609;
const MAX_NODES: usize = 512;
const MAX_DEPTH: usize = 16;

pub(crate) fn check(data: &[u8]) -> Result<(), Error> {
    if data.is_empty() || data.len() > MAX_BYTES {
        return Err(Error::Limit);
    }
    let mut offset = 0usize;
    let mut stack = [0usize; MAX_DEPTH + 1];
    let mut level = 0usize;
    stack[0] = 1;
    let mut nodes = 0;
    loop {
        if stack[level] == 0 {
            if level == 0 {
                break;
            }
            level -= 1;
            continue;
        }
        nodes += 1;
        if nodes > MAX_NODES {
            return Err(Error::Limit);
        }
        stack[level] -= 1;
        let first = *data.get(offset).ok_or(Error::Input)?;
        offset += 1;
        let major = first >> 5;
        let ai = first & 31;
        let size = match ai {
            0..=23 => u64::from(ai),
            24..=27 => {
                let count = 1usize << (ai - 24);
                let end = offset.checked_add(count).ok_or(Error::Limit)?;
                let bytes = data.get(offset..end).ok_or(Error::Input)?;
                let mut value = 0u64;
                for byte in bytes {
                    value = (value << 8) | u64::from(*byte);
                }
                offset = end;
                let minimum = match count {
                    1 => 24,
                    2 => 256,
                    4 => 65536,
                    _ => 1u64 << 32,
                };
                if value < minimum {
                    return Err(Error::Input);
                }
                value
            }
            _ => return Err(Error::Input),
        };
        let children = match major {
            0 | 1 => 0,
            2 | 3 => {
                let len = usize::try_from(size).map_err(|_| Error::Limit)?;
                let end = offset.checked_add(len).ok_or(Error::Limit)?;
                data.get(offset..end).ok_or(Error::Input)?;
                offset = end;
                0
            }
            4 => usize::try_from(size).map_err(|_| Error::Limit)?,
            5 => usize::try_from(size)
                .map_err(|_| Error::Limit)?
                .checked_mul(2)
                .ok_or(Error::Limit)?,
            7 if (20..=23).contains(&ai) => 0,
            _ => return Err(Error::Input),
        };
        if children > MAX_NODES - nodes {
            return Err(Error::Limit);
        }
        if children != 0 {
            if level == MAX_DEPTH {
                return Err(Error::Limit);
            }
            level += 1;
            stack[level] = children;
        }
    }
    if offset != data.len() {
        return Err(Error::Input);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    #[test]
    fn rejects_huge_node_counts_depth_and_noncanonical_or_trailing_data_before_allocating() {
        for input in [
            &[0x9b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff][..],
            &[0x18, 0],
            &[0, 0],
            &[0x9f, 0xff],
            &[0xc0, 0],
        ] {
            assert!(check(input).is_err());
        }
        let mut nodes = vec![0x99, 0x02, 0x00];
        nodes.extend([0; 512]);
        assert!(check(&nodes).is_err());
        let mut deep = vec![0x81; MAX_DEPTH + 1];
        deep.push(0);
        assert!(check(&deep).is_err());
        assert!(check(&[0xa1, 1, 0x43, 1, 2, 3]).is_ok());
    }
}
