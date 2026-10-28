//! Minimal ZIP writer (STORE only) — enough for EPUB packaging, where the
//! `mimetype` entry must be first and uncompressed.

use std::io::{self, Write};

fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *e = c;
        }
        t
    });
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c = t[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Write `entries` (name, bytes) as a stored ZIP archive.
pub fn write_zip<W: Write>(mut w: W, entries: &[(&str, &[u8])]) -> io::Result<()> {
    let mut central = Vec::new();
    let mut offset: u32 = 0;
    let too_big = || io::Error::other("entry too large for the minimal zip writer");
    for (name, data) in entries {
        let crc = crc32(data);
        let size = u32::try_from(data.len()).map_err(|_| too_big())?;
        let n = name.as_bytes();
        let mut local = Vec::with_capacity(30 + n.len());
        local.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        local.extend_from_slice(&20u16.to_le_bytes()); // version needed
        local.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 names
        local.extend_from_slice(&0u16.to_le_bytes()); // STORE
        local.extend_from_slice(&0u16.to_le_bytes()); // time
        local.extend_from_slice(&0x21u16.to_le_bytes()); // date 1980-01-01
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&(n.len() as u16).to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(n);
        w.write_all(&local)?;
        w.write_all(data)?;

        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // made by
        central.extend_from_slice(&local[4..30]);
        central.extend_from_slice(&0u16.to_le_bytes()); // comment len
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // int attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // ext attrs
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(n);
        offset = offset
            .checked_add(local.len() as u32)
            .and_then(|o| o.checked_add(size))
            .ok_or_else(too_big)?;
    }
    w.write_all(&central)?;
    let count = entries.len() as u16;
    let mut end = Vec::new();
    end.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    end.extend_from_slice(&[0, 0, 0, 0]);
    end.extend_from_slice(&count.to_le_bytes());
    end.extend_from_slice(&count.to_le_bytes());
    end.extend_from_slice(&(central.len() as u32).to_le_bytes());
    end.extend_from_slice(&offset.to_le_bytes());
    end.extend_from_slice(&0u16.to_le_bytes());
    w.write_all(&end)?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn writes_valid_header() {
        let mut v = Vec::new();
        write_zip(&mut v, &[("mimetype", b"application/epub+zip")]).unwrap();
        assert!(v.starts_with(b"PK\x03\x04"));
        assert_eq!(&v[30..38], b"mimetype");
        assert_eq!(&v[38..58], b"application/epub+zip");
    }
}
