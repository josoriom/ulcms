use miniz_oxide::inflate::decompress_to_vec_zlib;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::str;

#[derive(Debug, Clone)]
pub enum Node {
    Element(Element),
    Text(String),
    Comment(String),
}

#[derive(Debug, Clone)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone)]
pub struct SpectrumSummary {
    pub index: usize,
    pub id: String,
    pub array_length: usize,
    pub ms_level: Option<u32>,
    pub scan_type: Option<String>,
    pub polarity: Option<String>,
    pub spectrum_type: Option<String>,
    pub retention_time: Option<f64>,
    pub scan_window_lower_limit: Option<f64>,
    pub scan_window_upper_limit: Option<f64>,
    pub total_ion_current: Option<f64>,
    pub base_peak_intensity: Option<f64>,
    pub base_peak_mz: Option<f64>,
    pub mz_array: Option<Vec<f64>>,
    pub intensity_array: Option<Vec<f64>>,
}

struct Scratch {
    b64_buf: Vec<u8>,
    zlib_buf: Vec<u8>,
}

pub fn parse_mzml(bytes: &[u8]) -> Result<Vec<SpectrumSummary>, String> {
    let file_len = bytes.len() as u64;
    let mut cursor = Cursor::new(bytes);
    let mut scratch = Scratch {
        b64_buf: Vec::with_capacity(256),
        zlib_buf: Vec::with_capacity(256),
    };

    if let Some(offsets) = read_spectrum_offsets(&mut cursor)? {
        if file_len <= 1_073_741_824 {
            let all = bytes;
            let mut out = Vec::with_capacity(offsets.len());
            for i in 0..offsets.len() {
                let start = offsets[i] as usize;
                let end = if i + 1 < offsets.len() {
                    offsets[i + 1] as usize
                } else {
                    find_spectrum_end_in(all, start)
                        .ok_or_else(|| "no </spectrum> after last offset".to_string())?
                };
                if let Some(sum) = parse_spectrum_block(&all[start..end], &mut scratch) {
                    out.push(sum);
                }
            }
            return Ok(out);
        } else {
            let mut out = Vec::with_capacity(offsets.len());
            for i in 0..offsets.len() {
                let start = offsets[i];
                let next = if i + 1 < offsets.len() {
                    Some(offsets[i + 1])
                } else {
                    None
                };
                if let Some(sum) = read_one_spectrum_span(&mut cursor, start, next, &mut scratch)? {
                    out.push(sum);
                }
            }
            return Ok(out);
        }
    }

    cursor.set_position(0);
    linear_scan_spectra(&mut cursor, &mut scratch)
}

// <indexListOffset>, <index name="spectrum">
fn read_spectrum_offsets<R: Read + Seek>(r: &mut R) -> Result<Option<Vec<u64>>, String> {
    const TAIL: u64 = 64 * 1024;
    let end = r
        .seek(SeekFrom::End(0))
        .map_err(|e| format!("seek end: {e}"))?;
    let start = end.saturating_sub(TAIL);
    r.seek(SeekFrom::Start(start))
        .map_err(|e| format!("seek tail: {e}"))?;
    let mut tail = Vec::with_capacity((end - start) as usize);
    r.take(end - start)
        .read_to_end(&mut tail)
        .map_err(|e| format!("read tail: {e}"))?;
    if let Some(off) = extract_index_list_offset(&tail) {
        r.seek(SeekFrom::Start(off))
            .map_err(|e| format!("seek indexList: {e}"))?;
        let mut buf = vec![0u8; (end - off).min(1_000_000).max(4096) as usize];
        let n = r
            .read(&mut buf)
            .map_err(|e| format!("read indexList: {e}"))?;
        buf.truncate(n);
        let offs = parse_spectrum_offsets_from_index(&buf);
        return Ok(Some(offs));
    }
    Ok(None)
}

// <indexListOffset>
fn extract_index_list_offset(tail: &[u8]) -> Option<u64> {
    let tag = b"<indexListOffset>";
    let endtag = b"</indexListOffset>";
    let pos = memmem(tail, tag)?;
    let pos2 = memmem(&tail[pos + tag.len()..], endtag)?;
    let num = &tail[pos + tag.len()..pos + tag.len() + pos2];
    parse_u64_ascii(strip_ws(num))
}

// <index name="spectrum">, <offset>
fn parse_spectrum_offsets_from_index(buf: &[u8]) -> Vec<u64> {
    let mut out = Vec::new();
    if let Some(ix_start) = memmem(buf, br#"<index name="spectrum">"#) {
        if let Some(ix_end_rel) = memmem(&buf[ix_start..], b"</index>") {
            let block = &buf[ix_start..ix_start + ix_end_rel];
            let mut cursor = 0usize;
            let tag = b"<offset";
            let endtag = b"</offset>";
            while let Some(p) = memmem(&block[cursor..], tag) {
                let from = cursor + p;
                let g = match memchr(&block[from..], b'>') {
                    Some(rel) => from + rel + 1,
                    None => break,
                };
                let endp_rel = match memmem(&block[g..], endtag) {
                    Some(v) => v,
                    None => break,
                };
                let num = &block[g..g + endp_rel];
                if let Some(v) = parse_u64_ascii(strip_ws(num)) {
                    out.push(v);
                }
                cursor = g + endp_rel + endtag.len();
            }
        }
    }
    out
}

// <spectrum>
fn read_one_spectrum_span<R: Read + Seek>(
    r: &mut R,
    start: u64,
    next: Option<u64>,
    _scratch: &mut Scratch,
) -> Result<Option<SpectrumSummary>, String> {
    r.seek(SeekFrom::Start(start))
        .map_err(|e| format!("seek: {e}"))?;
    if let Some(end) = next {
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];
        r.read_exact(&mut buf)
            .map_err(|e| format!("read span: {e}"))?;
        if let Some(pos) = memmem(&buf, b"</spectrum>") {
            buf.truncate(pos + b"</spectrum>".len());
        }
        Ok(parse_spectrum_block(&buf, _scratch))
    } else {
        let mut buf = Vec::with_capacity(128 * 1024);
        let mut tmp = [0u8; 128 * 1024];
        let close = b"</spectrum>";
        let mut search_from = 0usize;
        loop {
            let n = r
                .read(&mut tmp)
                .map_err(|e| format!("read tail spectrum: {e}"))?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            let window_start = search_from.saturating_sub(close.len().saturating_sub(1));
            if let Some(rel) = memmem(&buf[window_start..], close) {
                let end = window_start + rel + close.len();
                buf.truncate(end);
                break;
            }
            search_from = buf.len();
            if buf.len() > 32 * 1024 * 1024 {
                return Err("spectrum block too large?".into());
            }
        }
        Ok(parse_spectrum_block(&buf, _scratch))
    }
}

// </spectrum>
fn find_spectrum_end_in(hay: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    loop {
        let rel = memchr(&hay[i..], b'<')?;
        let p = i + rel;
        if hay.get(p + 1..)?.starts_with(b"/spectrum>") {
            return Some(p + 1 + b"/spectrum>".len());
        }
        i = p + 1;
    }
}

// <spectrum>
fn linear_scan_spectra<R: Read + Seek>(
    r: &mut R,
    scratch: &mut Scratch,
) -> Result<Vec<SpectrumSummary>, String> {
    r.seek(SeekFrom::Start(0))
        .map_err(|e| format!("seek: {e}"))?;
    let mut file = Vec::new();
    r.read_to_end(&mut file)
        .map_err(|e| format!("read all: {e}"))?;
    let mut out = Vec::new();
    let mut cur = 0usize;
    let open_tag = b"<spectrum ";
    let close_tag = b"</spectrum>";
    while let Some(p) = memmem(&file[cur..], open_tag) {
        let start = cur + p;
        let end_rel = memmem(&file[start..], close_tag)
            .ok_or_else(|| "unterminated <spectrum>".to_string())?;
        let end = start + end_rel + close_tag.len();
        if let Some(sum) = parse_spectrum_block(&file[start..end], scratch) {
            out.push(sum);
        }
        cur = end;
    }
    Ok(out)
}

// <spectrum>, <cvParam>, <binaryDataArray>, <binary>
fn parse_spectrum_block(block: &[u8], scratch: &mut Scratch) -> Option<SpectrumSummary> {
    let index = find_attr_usize(block, b"spectrum", b"index").unwrap_or(0);
    let id = find_attr_string(block, b"spectrum", b"id").unwrap_or_default();
    let array_len = find_attr_usize(block, b"spectrum", b"defaultArrayLength").unwrap_or(0);

    let header_end = memmem(block, b"<binaryDataArrayList").unwrap_or(block.len());
    let header = &block[..header_end];

    let ms_level = find_cv_value_u32(header, b"ms level");
    let scan_type = if has_cv_name(header, b"MS1 spectrum") {
        Some("MS1".to_string())
    } else if has_cv_name(header, b"MSn spectrum") {
        Some("MSn".to_string())
    } else {
        None
    };
    let polarity = if has_cv_name(header, b"positive scan") {
        Some("positive".to_string())
    } else if has_cv_name(header, b"negative scan") {
        Some("negative".to_string())
    } else {
        None
    };
    let spectrum_type = if has_cv_name(header, b"profile spectrum") {
        Some("profile".to_string())
    } else if has_cv_name(header, b"centroid spectrum") {
        Some("centroid".to_string())
    } else {
        None
    };
    let total_ion_current = find_cv_value_f64(header, b"total ion current");
    let base_peak_intensity = find_cv_value_f64(header, b"base peak intensity");
    let base_peak_mz = find_cv_value_f64(header, b"base peak m/z");
    let retention_time = find_scan_start_time_min(header);
    let scan_window_lower_limit = find_cv_value_f64(header, b"scan window lower limit");
    let scan_window_upper_limit = find_cv_value_f64(header, b"scan window upper limit");

    let (mz_array, intensity_array) = decode_binary_arrays(block, array_len, scratch);

    Some(SpectrumSummary {
        index,
        id,
        array_length: array_len,
        ms_level,
        scan_type,
        polarity,
        spectrum_type,
        retention_time,
        scan_window_lower_limit,
        scan_window_upper_limit,
        total_ion_current,
        base_peak_intensity,
        base_peak_mz,
        mz_array,
        intensity_array,
    })
}

fn find_attr_usize(buf: &[u8], tag: &[u8], attr: &[u8]) -> Option<usize> {
    find_attr_ascii(buf, tag, attr).and_then(|s| str::from_utf8(s).ok()?.parse().ok())
}

fn find_attr_string(buf: &[u8], tag: &[u8], attr: &[u8]) -> Option<String> {
    let b = find_attr_ascii(buf, tag, attr)?;
    String::from_utf8(b.to_vec()).ok()
}

fn find_attr_ascii<'a>(buf: &'a [u8], tag: &[u8], attr: &[u8]) -> Option<&'a [u8]> {
    let mut pat = Vec::with_capacity(1 + tag.len());
    pat.push(b'<');
    pat.extend_from_slice(tag);
    let start = memmem(buf, &pat)?;
    let gt = memchr(&buf[start..], b'>').map(|x| start + x)?;
    let head = &buf[start..gt];
    find_attr_value_in_tag(head, attr)
}

fn find_attr_value_in_tag<'a>(head: &'a [u8], attr: &[u8]) -> Option<&'a [u8]> {
    let mut pat = Vec::with_capacity(attr.len() + 1);
    pat.extend_from_slice(attr);
    pat.push(b'=');
    let p = memmem(head, &pat)?;
    let q = p + pat.len();
    let quote = *head.get(q)?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let rest = &head[q + 1..];
    let end = memchr(rest, quote)?;
    Some(&rest[..end])
}

// <cvParam>
fn has_cv_name(buf: &[u8], name: &[u8]) -> bool {
    let mut cur = 0usize;
    const TAG: &[u8] = b"<cvParam";
    while let Some(p) = memmem(&buf[cur..], TAG) {
        let from = cur + p;
        if let Some(gt_rel) = memchr(&buf[from..], b'>') {
            let gt = from + gt_rel;
            let head = &buf[from..gt];
            if let Some(v) = find_attr_value_in_tag(head, b"name") {
                if v == name {
                    return true;
                }
            }
            cur = gt + 1;
            continue;
        } else {
            return false;
        }
    }
    false
}

// <cvParam>
fn find_cv_value_f64(buf: &[u8], name: &[u8]) -> Option<f64> {
    find_cv_value(buf, name).and_then(|s| str::from_utf8(s).ok()?.parse().ok())
}

// <cvParam>
fn find_cv_value_u32(buf: &[u8], name: &[u8]) -> Option<u32> {
    find_cv_value(buf, name).and_then(|s| str::from_utf8(s).ok()?.parse().ok())
}

// <cvParam>
fn find_cv_value<'a>(buf: &'a [u8], name: &[u8]) -> Option<&'a [u8]> {
    let mut cur = 0usize;
    const TAG: &[u8] = b"<cvParam";
    while let Some(p) = memmem(&buf[cur..], TAG) {
        let from = cur + p;
        let gt = memchr(&buf[from..], b'>').map(|x| from + x)?;
        let head = &buf[from..gt];
        if let Some(nm) = find_attr_value_in_tag(head, b"name") {
            if nm == name {
                return find_attr_value_in_tag(head, b"value");
            }
        }
        cur = gt + 1;
    }
    None
}

// <cvParam name="scan start time">
fn find_scan_start_time_min(buf: &[u8]) -> Option<f64> {
    let mut cur = 0usize;
    const SCAN_TAG: &[u8] = b"<cvParam";
    while let Some(p) = memmem(&buf[cur..], SCAN_TAG) {
        let from = cur + p;
        let gt = memchr(&buf[from..], b'>').map(|x| from + x)?;
        let head = &buf[from..gt];
        if let Some(nm) = find_attr_value_in_tag(head, b"name") {
            if nm == b"scan start time" {
                let val = find_attr_value_in_tag(head, b"value")?;
                let mut v: f64 = str::from_utf8(val).ok()?.parse().ok()?;
                if let Some(unit) = find_attr_value_in_tag(head, b"unitName") {
                    if unit == b"second" {
                        v /= 60.0;
                    }
                }
                return Some(v);
            }
        }
        cur = gt + 1;
    }
    None
}

// <binaryDataArray>
fn bda_flags(b: &[u8]) -> (bool, bool, bool, bool, bool, bool) {
    let stop = memmem(b, b"<binary>").unwrap_or(b.len());
    let head = &b[..stop];
    let mut kind_mz = false;
    let mut kind_int = false;
    let mut is_zlib = false;
    let mut is_f64 = false;
    let mut is_f32 = false;
    let mut little = true;
    let mut cur = 0usize;
    const TAG: &[u8] = b"<cvParam";
    while let Some(p) = memmem(&head[cur..], TAG) {
        let from = cur + p;
        if let Some(gt_rel) = memchr(&head[from..], b'>') {
            let gt = from + gt_rel;
            let tag_head = &head[from..gt];
            if let Some(nm) = find_attr_value_in_tag(tag_head, b"name") {
                match nm {
                    b"m/z array" => kind_mz = true,
                    b"intensity array" => kind_int = true,
                    b"zlib compression" => is_zlib = true,
                    b"64-bit float" => is_f64 = true,
                    b"32-bit float" => is_f32 = true,
                    b"little endian" => little = true,
                    b"big endian" => little = false,
                    _ => {}
                }
            }
            cur = gt + 1;
        } else {
            break;
        }
    }
    (kind_mz, kind_int, is_zlib, is_f64, is_f32, little)
}

// <binaryDataArray>, <binary>
fn decode_binary_arrays(
    block: &[u8],
    expected_len: usize,
    scratch: &mut Scratch,
) -> (Option<Vec<f64>>, Option<Vec<f64>>) {
    let mut mz: Option<Vec<f64>> = None;
    let mut inten: Option<Vec<f64>> = None;
    let mut cur = 0usize;
    const BDA: &[u8] = b"<binaryDataArray";
    while let Some(p) = memmem(&block[cur..], BDA) {
        let start = cur + p;
        let end_rel = match memmem(&block[start..], b"</binaryDataArray>") {
            Some(v) => v,
            None => break,
        };
        let b = &block[start..start + end_rel];

        let (kind_mz, kind_int, is_zlib, is_f64, is_f32, little) = bda_flags(b);

        if let Some((bs, be)) = tag_body(b, b"<binary>", b"</binary>") {
            scratch.b64_buf.clear();
            if !decode_base64_ws_into(&b[bs..be], &mut scratch.b64_buf) {
                cur = start + end_rel + b"</binaryDataArray>".len();
                continue;
            }

            let bytes: &[u8] = if is_zlib {
                scratch.zlib_buf.clear();
                match decompress_to_vec_zlib(&scratch.b64_buf) {
                    Ok(v) => {
                        scratch.zlib_buf = v;
                        &scratch.zlib_buf
                    }
                    Err(_) => {
                        cur = start + end_rel + b"</binaryDataArray>".len();
                        continue;
                    }
                }
            } else {
                &scratch.b64_buf
            };

            let want = if expected_len > 0 {
                expected_len
            } else if is_f64 {
                bytes.len() / 8
            } else {
                bytes.len() / 4
            };

            let vals = if is_f64 {
                bytes_to_f64_exact_into(bytes, little, want)
            } else if is_f32 {
                bytes_to_f32_as_f64_exact_into(bytes, little, want)
            } else {
                Vec::new()
            };

            match (kind_mz, kind_int) {
                (true, false) => mz = Some(vals),
                (false, true) => inten = Some(vals),
                (true, true) => {}
                (false, false) => {}
            }
        }

        cur = start + end_rel + b"</binaryDataArray>".len();
    }
    (mz, inten)
}

// <binary>
fn tag_body(hay: &[u8], open: &[u8], close: &[u8]) -> Option<(usize, usize)> {
    let s = memmem(hay, open)?;
    let e_rel = memmem(&hay[s + open.len()..], close)?;
    Some((s + open.len(), s + open.len() + e_rel))
}

fn decode_base64_ws_into(s: &[u8], out: &mut Vec<u8>) -> bool {
    let mut useful = 0usize;
    let mut pads = 0usize;
    for &b in s.iter().rev() {
        if is_ws(b) {
            continue;
        }
        if b == b'=' {
            pads += 1;
        } else {
            break;
        }
    }
    for &b in s {
        if !is_ws(b) {
            useful += 1;
        }
    }
    if useful == 0 {
        return true;
    }
    if useful % 4 != 0 {
        return false;
    }
    let estimated = useful / 4 * 3 - pads;
    out.reserve(estimated);
    let inv = &BASE64_INV;
    let mut q = [0u8; 4];
    let mut qi = 0usize;
    for &b in s {
        if is_ws(b) {
            continue;
        }
        q[qi] = b;
        qi += 1;
        if qi == 4 {
            let v0 = inv[q[0] as usize];
            if v0 == 255 {
                return false;
            }
            let v1 = inv[q[1] as usize];
            if v1 == 255 {
                return false;
            }
            let v2 = if q[2] == b'=' {
                0
            } else {
                let v = inv[q[2] as usize];
                if v == 255 {
                    return false;
                }
                v
            };
            let v3 = if q[3] == b'=' {
                0
            } else {
                let v = inv[q[3] as usize];
                if v == 255 {
                    return false;
                }
                v
            };
            let n = ((v0 as u32) << 18) | ((v1 as u32) << 12) | ((v2 as u32) << 6) | (v3 as u32);
            out.push(((n >> 16) & 0xFF) as u8);
            if q[2] != b'=' {
                out.push(((n >> 8) & 0xFF) as u8);
            }
            if q[3] != b'=' {
                out.push((n & 0xFF) as u8);
            }
            qi = 0;
        }
    }
    qi == 0
}

#[inline]
fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\r' | b'\t')
}

#[inline]
fn bytes_to_f64_exact_into(b: &[u8], little: bool, want: usize) -> Vec<f64> {
    let len = want.min(b.len() / 8);
    let mut out = Vec::with_capacity(len);
    let bytes = &b[..len * 8];

    if little {
        for c in bytes.chunks_exact(8) {
            let bits = u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
            out.push(f64::from_bits(bits));
        }
    } else {
        for c in bytes.chunks_exact(8) {
            let bits = u64::from_be_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
            out.push(f64::from_bits(bits));
        }
    }
    out
}

#[inline]
fn bytes_to_f32_as_f64_exact_into(b: &[u8], little: bool, want: usize) -> Vec<f64> {
    let len = want.min(b.len() / 4);
    let mut out = Vec::with_capacity(len);
    let words = &b[..len * 4];

    for c in words.chunks_exact(4) {
        let bits = if little {
            u32::from_le_bytes([c[0], c[1], c[2], c[3]])
        } else {
            u32::from_be_bytes([c[0], c[1], c[2], c[3]])
        };
        out.push(f32::from_bits(bits) as f64);
    }
    out
}

fn memmem(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    let first = needle[0];
    let mut i = 0usize;
    while let Some(rel) = memchr(&hay[i..], first) {
        let p = i + rel;
        if hay.get(p..p + needle.len()).map_or(false, |w| w == needle) {
            return Some(p);
        }
        i = p + 1;
    }
    None
}

fn memchr(hay: &[u8], byte: u8) -> Option<usize> {
    hay.iter().position(|&b| b == byte)
}

fn strip_ws(s: &[u8]) -> &[u8] {
    let mut a = 0;
    let mut b = s.len();
    while a < b && is_ws(s[a]) {
        a += 1;
    }
    while b > a && is_ws(s[b - 1]) {
        b -= 1;
    }
    &s[a..b]
}

fn parse_u64_ascii(s: &[u8]) -> Option<u64> {
    let t = strip_ws(s);
    if t.is_empty() {
        return None;
    }
    let mut v: u64 = 0;
    for &c in t {
        if c < b'0' || c > b'9' {
            return None;
        }
        v = v.checked_mul(10)?.checked_add((c - b'0') as u64)?;
    }
    Some(v)
}

const fn build_b64_inv() -> [u8; 256] {
    let mut t = [255u8; 256];
    let alphabet = *b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut i = 0;
    while i < 64 {
        t[alphabet[i] as usize] = i as u8;
        i += 1;
    }
    t
}
static BASE64_INV: [u8; 256] = build_b64_inv();
