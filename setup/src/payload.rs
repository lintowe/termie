//! the embedded application payload: files packed by build.rs, DEFLATE per
//! entry. an empty payload (dev build without TERMIE_PAYLOAD_DIR) parses to
//! zero entries and install refuses with a clear message

pub struct Entry {
    /// path relative to the install root, forward slashes
    pub name: String,
    pub raw_len: u64,
    comp: &'static [u8],
}

impl Entry {
    pub fn decompress(&self) -> Result<Vec<u8>, String> {
        let limit = usize::try_from(self.raw_len)
            .map_err(|_| format!("payload {} is too large for this platform", self.name))?;
        let bytes = miniz_oxide::inflate::decompress_to_vec_with_limit(self.comp, limit)
            .map_err(|e| format!("decompress {}: {e:?}", self.name))?;
        (bytes.len() as u64 == self.raw_len)
            .then_some(bytes)
            .ok_or_else(|| format!("payload {} has an unexpected size", self.name))
    }
}

pub fn entries() -> Vec<Entry> {
    parse(include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin")))
}

pub const APP_VERSION: &str = env!("TERMIE_APP_VERSION");

fn parse(bytes: &'static [u8]) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut p = 0usize;
    let take = |p: &mut usize, n: usize| -> Option<&'static [u8]> {
        let s = bytes.get(*p..*p + n)?;
        *p += n;
        Some(s)
    };
    let Some(count) = take(&mut p, 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())) else {
        return out;
    };
    for _ in 0..count {
        let Some(nl) = take(&mut p, 2).map(|b| u16::from_le_bytes(b.try_into().unwrap())) else {
            break;
        };
        let Some(name) = take(&mut p, nl as usize).and_then(|b| std::str::from_utf8(b).ok()) else {
            break;
        };
        let Some(raw_len) = take(&mut p, 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()))
        else {
            break;
        };
        let Some(comp_len) = take(&mut p, 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()))
        else {
            break;
        };
        let Some(comp) = take(&mut p, comp_len as usize) else {
            break;
        };
        out.push(Entry { name: name.to_string(), raw_len, comp });
    }
    out
}

/// total bytes on disk once installed, for the ARP EstimatedSize entry
pub fn installed_bytes(entries: &[Entry]) -> u64 {
    entries.iter().map(|e| e.raw_len).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decompression_respects_the_recorded_size() {
        let compressed = miniz_oxide::deflate::compress_to_vec(b"termie", 8);
        let compressed = Box::leak(compressed.into_boxed_slice());
        let entry = Entry { name: "termie.exe".into(), raw_len: 6, comp: compressed };
        assert_eq!(entry.decompress().unwrap(), b"termie");

        let oversized = Entry { name: "termie.exe".into(), raw_len: 5, comp: compressed };
        assert!(oversized.decompress().is_err());
    }
}
