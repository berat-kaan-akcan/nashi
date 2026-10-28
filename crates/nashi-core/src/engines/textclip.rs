//! macOS `.textclipping` files: binary or XML property lists whose
//! `UTI-Data` dictionary holds the clipped text.

// `as_chunks` would need Rust 1.88; the workspace MSRV is 1.80.
#![allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]

#[derive(Debug, Clone)]
enum Value {
    Dict(Vec<(Value, Value)>),
    Array(Vec<Value>),
    Str(String),
    Data(Vec<u8>),
    Other,
}

const TEXT_KEYS: &[&str] = &[
    "public.utf8-plain-text",
    "public.plain-text",
    "public.utf16-plain-text",
    "com.apple.traditional-mac-plain-text",
];

/// Extract the clipped text from a `.textclipping` file's data fork.
pub fn extract_text(data: &[u8]) -> Option<String> {
    if data.starts_with(b"bplist00") {
        let root = parse_bplist(data)?;
        find_text(&root)
    } else if let Ok(s) = std::str::from_utf8(data) {
        if s.contains("<plist") {
            xml_text(s)
        } else {
            None
        }
    } else {
        None
    }
}

fn decode_text(key: &str, v: &Value) -> Option<String> {
    match v {
        Value::Str(s) => Some(s.clone()),
        Value::Data(d) if key == "public.utf16-plain-text" => {
            let (le, body) = match d.get(..2) {
                Some([0xFF, 0xFE]) => (true, &d[2..]),
                Some([0xFE, 0xFF]) => (false, &d[2..]),
                _ => (true, &d[..]),
            };
            let units: Vec<u16> = body
                .chunks_exact(2)
                .map(|c| {
                    if le {
                        u16::from_le_bytes([c[0], c[1]])
                    } else {
                        u16::from_be_bytes([c[0], c[1]])
                    }
                })
                .collect();
            Some(String::from_utf16_lossy(&units))
        }
        Value::Data(d) => Some(String::from_utf8_lossy(d).into_owned()),
        _ => None,
    }
}

fn find_text(v: &Value) -> Option<String> {
    match v {
        Value::Dict(entries) => {
            for want in TEXT_KEYS {
                for (k, val) in entries {
                    if let Value::Str(k) = k {
                        if k == want {
                            if let Some(t) = decode_text(k, val) {
                                return Some(t);
                            }
                        }
                    }
                }
            }
            entries.iter().find_map(|(_, val)| find_text(val))
        }
        Value::Array(a) => a.iter().find_map(find_text),
        _ => None,
    }
}

fn be(b: &[u8]) -> u64 {
    b.iter().fold(0u64, |a, x| (a << 8) | *x as u64)
}

fn parse_bplist(d: &[u8]) -> Option<Value> {
    if d.len() < 40 {
        return None;
    }
    let tr = &d[d.len() - 32..];
    let off_size = tr[6] as usize;
    let ref_size = tr[7] as usize;
    let num = be(&tr[8..16]) as usize;
    let top = be(&tr[16..24]) as usize;
    let table = be(&tr[24..32]) as usize;
    if off_size == 0
        || ref_size == 0
        || num > 1_000_000
        || table.checked_add(num.checked_mul(off_size)?)? > d.len()
    {
        return None;
    }
    let offsets: Vec<usize> = (0..num)
        .map(|i| be(&d[table + i * off_size..table + (i + 1) * off_size]) as usize)
        .collect();
    let p = Bp {
        d,
        offsets,
        ref_size,
    };
    p.obj(top, 0)
}

struct Bp<'a> {
    d: &'a [u8],
    offsets: Vec<usize>,
    ref_size: usize,
}

impl Bp<'_> {
    fn len_at(&self, pos: usize, low: u8) -> Option<(usize, usize)> {
        if low != 0x0F {
            return Some((low as usize, pos + 1));
        }
        let m = *self.d.get(pos + 1)?;
        if m >> 4 != 1 {
            return None;
        }
        let n = 1usize << (m & 0x0F);
        let v = be(self.d.get(pos + 2..pos + 2 + n)?) as usize;
        Some((v, pos + 2 + n))
    }

    fn obj(&self, idx: usize, depth: u32) -> Option<Value> {
        if depth > 32 {
            return None;
        }
        let pos = *self.offsets.get(idx)?;
        let marker = *self.d.get(pos)?;
        let (hi, low) = (marker >> 4, marker & 0x0F);
        Some(match hi {
            0x4 => {
                let (n, s) = self.len_at(pos, low)?;
                Value::Data(self.d.get(s..s + n)?.to_vec())
            }
            0x5 => {
                let (n, s) = self.len_at(pos, low)?;
                Value::Str(String::from_utf8_lossy(self.d.get(s..s + n)?).into_owned())
            }
            0x6 => {
                let (n, s) = self.len_at(pos, low)?;
                let units: Vec<u16> = self
                    .d
                    .get(s..s + 2 * n)?
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                Value::Str(String::from_utf16_lossy(&units))
            }
            0xA => {
                let (n, s) = self.len_at(pos, low)?;
                let mut v = Vec::new();
                for i in 0..n {
                    let r = be(self
                        .d
                        .get(s + i * self.ref_size..s + (i + 1) * self.ref_size)?)
                        as usize;
                    v.push(self.obj(r, depth + 1)?);
                }
                Value::Array(v)
            }
            0xD => {
                let (n, s) = self.len_at(pos, low)?;
                let rs = self.ref_size;
                let mut v = Vec::new();
                for i in 0..n {
                    let k = be(self.d.get(s + i * rs..s + (i + 1) * rs)?) as usize;
                    let o = be(self.d.get(s + (n + i) * rs..s + (n + i + 1) * rs)?) as usize;
                    v.push((self.obj(k, depth + 1)?, self.obj(o, depth + 1)?));
                }
                Value::Dict(v)
            }
            _ => Value::Other,
        })
    }
}

fn b64(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        } as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    out
}

fn unescape_xml(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn xml_text(s: &str) -> Option<String> {
    for key in TEXT_KEYS {
        let k = format!("<key>{key}</key>");
        if let Some(i) = s.find(&k) {
            let rest = s[i + k.len()..].trim_start();
            if let Some(r) = rest.strip_prefix("<data>") {
                let end = r.find("</data>")?;
                let bytes = b64(&r[..end]);
                return decode_text(key, &Value::Data(bytes));
            }
            if let Some(r) = rest.strip_prefix("<string>") {
                let end = r.find("</string>")?;
                return Some(unescape_xml(&r[..end]));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tiny bplist: {"UTI-Data": {"public.utf8-plain-text": data}}.
    fn sample_bplist(text: &str) -> Vec<u8> {
        let mut d = b"bplist00".to_vec();
        let mut offs = Vec::new();
        // 0: outer dict {1: 2}
        offs.push(d.len());
        d.extend_from_slice(&[0xD1, 1, 2]);
        // 1: "UTI-Data"
        offs.push(d.len());
        d.push(0x58);
        d.extend_from_slice(b"UTI-Data");
        // 2: inner dict {3: 4}
        offs.push(d.len());
        d.extend_from_slice(&[0xD1, 3, 4]);
        // 3: key
        offs.push(d.len());
        let k = b"public.utf8-plain-text";
        d.push(0x5F);
        d.push(0x10);
        d.push(k.len() as u8);
        d.extend_from_slice(k);
        // 4: data
        offs.push(d.len());
        let t = text.as_bytes();
        d.push(0x4F);
        d.push(0x10);
        d.push(t.len() as u8);
        d.extend_from_slice(t);
        let table = d.len();
        for o in &offs {
            d.push(*o as u8);
        }
        let mut trailer = vec![0u8; 6];
        trailer.push(1); // offset size
        trailer.push(1); // ref size
        trailer.extend_from_slice(&(offs.len() as u64).to_be_bytes());
        trailer.extend_from_slice(&0u64.to_be_bytes());
        trailer.extend_from_slice(&(table as u64).to_be_bytes());
        d.extend_from_slice(&trailer);
        d
    }

    #[test]
    fn bplist() {
        assert_eq!(
            extract_text(&sample_bplist("Hello clip")).as_deref(),
            Some("Hello clip")
        );
        assert!(extract_text(b"bplist00garbage").is_none());
    }

    #[test]
    fn xml() {
        let x = "<plist><dict><key>UTI-Data</key><dict><key>public.utf8-plain-text</key><data>SGVsbG8=</data></dict></dict></plist>";
        assert_eq!(extract_text(x.as_bytes()).as_deref(), Some("Hello"));
    }
}
