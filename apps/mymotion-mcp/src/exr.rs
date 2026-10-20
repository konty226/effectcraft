//! Just enough of OpenEXR's header to list a render's passes: single- or multi-part files,
//! each part's name and channels (`ViewLayer.Combined.R`, `ViewLayer.Shadow Catcher.G`…), and
//! Cryptomatte layer names. Pixel data is never read.

use std::path::Path;

/// One pass: its layer name (`ViewLayer.Shadow Catcher`) and channel suffixes (`R`, `G`, `B`, `A`, `Z`).
#[derive(Debug, Clone, PartialEq)]
pub struct Pass {
    pub layer: String,
    pub channels: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Header {
    pub multipart: bool,
    pub width: u32,
    pub height: u32,
    pub passes: Vec<Pass>,
    pub cryptomatte: Vec<String>,
}

const MAGIC: [u8; 4] = [0x76, 0x2f, 0x31, 0x01];
const MULTIPART: u32 = 0x1000;

fn cstr(d: &[u8], i: &mut usize) -> Option<String> {
    let rest = d.get(*i..)?;
    let n = rest.iter().position(|&b| b == 0)?;
    let s = String::from_utf8_lossy(rest.get(..n)?).into_owned();
    *i += n + 1;
    Some(s)
}

fn i32_at(d: &[u8], i: usize) -> Option<i32> {
    Some(i32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?))
}

/// Channel names of a `chlist` attribute value.
fn chlist(v: &[u8]) -> Vec<String> {
    let mut out = vec![];
    let mut i = 0;
    while v.get(i).is_some_and(|&b| b != 0) {
        let Some(name) = cstr(v, &mut i) else { break };
        out.push(name);
        i += 16; // pixel type, pLinear, reserved, xSampling, ySampling
    }
    out
}

/// Parse the header(s) at the start of an EXR file (`bytes` may be just its first few hundred KB).
pub fn parse(bytes: &[u8]) -> Result<Header, String> {
    if bytes.get(..4) != Some(&MAGIC[..]) {
        return Err("not an OpenEXR file".into());
    }
    let flags = u32::from_le_bytes(bytes.get(4..8).and_then(|s| s.try_into().ok()).ok_or("truncated EXR")?);
    let mut h = Header { multipart: flags & MULTIPART != 0, ..Header::default() };
    let mut channels: Vec<String> = vec![];
    let mut i = 8;
    for _part in 0..256 {
        let mut part_channels = vec![];
        loop {
            match bytes.get(i) {
                None => return Err("truncated EXR header".into()),
                Some(0) => {
                    i += 1;
                    break;
                }
                Some(_) => {}
            }
            let name = cstr(bytes, &mut i).ok_or("bad attribute name")?;
            let _ty = cstr(bytes, &mut i).ok_or("bad attribute type")?;
            let size = usize::try_from(i32_at(bytes, i).ok_or("truncated attribute")?).map_err(|_| "negative attribute size")?;
            i += 4;
            let val = bytes.get(i..i + size).ok_or("truncated attribute value")?;
            i += size;
            match name.as_str() {
                "channels" => part_channels = chlist(val),
                "dataWindow" if h.width == 0 && val.len() >= 16 => {
                    let n = |k: usize| i32_at(val, k).unwrap_or(0);
                    h.width = u32::try_from(n(8) - n(0) + 1).unwrap_or(0);
                    h.height = u32::try_from(n(12) - n(4) + 1).unwrap_or(0);
                }
                n if n.starts_with("cryptomatte/") && n.ends_with("/name") => {
                    h.cryptomatte.push(String::from_utf8_lossy(val).trim_end_matches('\0').to_string());
                }
                _ => {}
            }
        }
        channels.extend(part_channels);
        if !h.multipart || bytes.get(i) == Some(&0) {
            break;
        }
    }
    // Group channels by layer: "ViewLayer.Combined.R" → layer "ViewLayer.Combined", channel "R".
    for c in channels {
        let (layer, ch) = c.rsplit_once('.').map(|(l, c)| (l.to_string(), c.to_string())).unwrap_or((String::new(), c));
        match h.passes.iter_mut().find(|p| p.layer == layer) {
            Some(p) => p.channels.push(ch),
            None => h.passes.push(Pass { layer, channels: vec![ch] }),
        }
    }
    for p in &mut h.passes {
        p.channels.sort_by_key(|c| ["R", "G", "B", "A", "X", "Y", "Z", "r", "g", "b", "a"].iter().position(|o| o == c).unwrap_or(99));
    }
    Ok(h)
}

pub fn read(path: &Path) -> Result<Header, String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut buf = vec![0u8; 1 << 20];
    let n = f.read(&mut buf).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    buf.truncate(n);
    parse(&buf)
}

/// The pass whose layer ends with `suffix` (case-insensitive, spaces ignored): "Shadow Catcher",
/// "Combined", "Ambient Occlusion", "Depth", "Mist".
pub fn find<'a>(h: &'a Header, suffix: &str) -> Option<&'a Pass> {
    let norm = |s: &str| s.to_lowercase().replace(' ', "");
    let want = norm(suffix);
    h.passes.iter().find(|p| {
        let l = norm(&p.layer);
        (l == want || l.ends_with(&format!(".{want}"))) && !l.contains("noisy")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attr(out: &mut Vec<u8>, name: &str, ty: &str, val: &[u8]) {
        out.extend_from_slice(name.as_bytes());
        out.push(0);
        out.extend_from_slice(ty.as_bytes());
        out.push(0);
        out.extend_from_slice(&(val.len() as i32).to_le_bytes());
        out.extend_from_slice(val);
    }

    fn chans(names: &[&str]) -> Vec<u8> {
        let mut v = vec![];
        for n in names {
            v.extend_from_slice(n.as_bytes());
            v.push(0);
            v.extend_from_slice(&[0u8; 16]);
        }
        v.push(0);
        v
    }

    #[test]
    fn multipart_passes() {
        let mut d = MAGIC.to_vec();
        d.extend_from_slice(&(2u32 | MULTIPART).to_le_bytes());
        let mut dw = vec![];
        for n in [0i32, 0, 1079, 1919] {
            dw.extend_from_slice(&n.to_le_bytes());
        }
        attr(&mut d, "channels", "chlist", &chans(&["ViewLayer.Combined.A", "ViewLayer.Combined.B", "ViewLayer.Combined.G", "ViewLayer.Combined.R"]));
        attr(&mut d, "dataWindow", "box2i", &dw);
        attr(&mut d, "name", "string", b"ViewLayer.Combined");
        attr(&mut d, "cryptomatte/abc/name", "string", b"ViewLayer.CryptoObject");
        d.push(0);
        attr(&mut d, "channels", "chlist", &chans(&["ViewLayer.Shadow Catcher.B", "ViewLayer.Shadow Catcher.G", "ViewLayer.Shadow Catcher.R"]));
        attr(&mut d, "name", "string", b"ViewLayer.Shadow Catcher");
        d.push(0);
        attr(&mut d, "channels", "chlist", &chans(&["ViewLayer.Noisy Shadow Catcher.R"]));
        d.push(0);
        d.push(0);
        d.extend_from_slice(&[1, 2, 3]);
        let h = parse(&d).unwrap_or_default();
        assert!(h.multipart);
        assert_eq!((h.width, h.height), (1080, 1920));
        assert_eq!(h.cryptomatte, vec!["ViewLayer.CryptoObject".to_string()]);
        assert_eq!(find(&h, "combined").map(|p| p.channels.clone()), Some(vec!["R".into(), "G".into(), "B".into(), "A".into()]));
        assert_eq!(find(&h, "Shadow Catcher").map(|p| p.layer.as_str()), Some("ViewLayer.Shadow Catcher"));
        assert!(find(&h, "mist").is_none());
        assert!(parse(b"nope").is_err());
        assert!(parse(&d[..30]).is_err());
    }
}
