use miniz_oxide::inflate::decompress_to_vec_zlib;
use std::{fmt, fs};

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

impl Element {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
    pub fn find_first(&self, name: &str) -> Option<&Element> {
        for child in &self.children {
            if let Node::Element(el) = child {
                if el.name == name {
                    return Some(el);
                }
                if let Some(found) = el.find_first(name) {
                    return Some(found);
                }
            }
        }
        None
    }
    pub fn first_child_named(&self, name: &str) -> Option<&Element> {
        self.children.iter().find_map(|n| match n {
            Node::Element(e) if e.name == name => Some(e),
            _ => None,
        })
    }
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> {
        self.children.iter().filter_map(move |n| match n {
            Node::Element(e) if e.name == name => Some(e),
            _ => None,
        })
    }
}

impl fmt::Display for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn write_el(f: &mut fmt::Formatter<'_>, el: &Element, indent: usize) -> fmt::Result {
            let pad = " ".repeat(indent);
            write!(f, "{}<{}", pad, el.name)?;
            for (k, v) in &el.attrs {
                write!(f, " {}=\"{}\"", k, v)?;
            }
            writeln!(f, ">")?;
            for child in &el.children {
                match child {
                    Node::Element(c) => write_el(f, c, indent + 2)?,
                    Node::Text(t) => {
                        let t = t.trim();
                        if !t.is_empty() {
                            writeln!(f, "{}  \"{}\"", pad, t)?;
                        }
                    }
                    Node::Comment(c) => writeln!(f, "{}  <!--{}-->", pad, c)?,
                }
            }
            writeln!(f, "{}</{}>", pad, el.name)
        }
        write_el(f, self, 0)
    }
}

// <document>
pub fn parse_mzml(path: &str) -> Result<Vec<SpectrumSummary>, String> {
    let mut bytes = fs::read(path).map_err(|e| format!("read error: {e}"))?;
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        bytes.drain(0..3);
    }
    let mut p = Parser::new(bytes);
    let root = p.parse_document()?;
    Ok(extract_spectra(&root))
}

struct Parser {
    s: Vec<u8>,
    i: usize,
    n: usize,
}

impl Parser {
    fn new(s: Vec<u8>) -> Self {
        let n = s.len();
        Self { s, i: 0, n }
    }

    // <document>
    fn parse_document(&mut self) -> Result<Element, String> {
        self.skip_misc()?;
        let el = self.parse_element()?;
        self.skip_misc()?;
        Ok(el)
    }

    // <? ... ?> <!-- ... -->
    fn skip_misc(&mut self) -> Result<(), String> {
        loop {
            self.skip_ws();
            if self.starts_with(b"<?") {
                self.consume_pi()?;
            } else if self.starts_with(b"<!--") {
                self.consume_comment()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    // <element>
    fn parse_element(&mut self) -> Result<Element, String> {
        self.expect(b'<')?;
        if self.peek_is(b'/') {
            return Err("unexpected closing tag".into());
        }
        let name = self.parse_name_string()?;
        self.skip_ws();

        let mut attrs = Vec::new();
        while !self.eof() && !self.peek_is(b'>') && !self.starts_with(b"/>") {
            let k = self.parse_name_string()?;
            self.skip_ws();
            self.expect(b'=')?;
            self.skip_ws();
            let v = self.parse_attr_value_fast()?;
            attrs.push((k, v));
            self.skip_ws();
        }

        if self.starts_with(b"/>") {
            self.i += 2;
            return Ok(Element {
                name,
                attrs,
                children: Vec::new(),
            });
        }

        self.expect(b'>')?;

        let mut children = Vec::new();
        loop {
            self.skip_ws();
            if self.starts_with(b"</") {
                self.i += 2;
                let end_name = self.parse_name_string()?;
                self.skip_ws();
                self.expect(b'>')?;
                if end_name != name {
                    return Err(format!(
                        "mismatched end tag: expected </{}> got </{}>",
                        name, end_name
                    ));
                }
                break;
            }
            if self.eof() {
                return Err("unexpected EOF inside element".into());
            }
            if self.peek_is(b'<') {
                if self.starts_with(b"<!--") {
                    let c = self.consume_comment()?;
                    children.push(Node::Comment(c));
                } else if self.starts_with(b"<![CDATA[") {
                    let t = self.consume_cdata()?;
                    if !t.trim().is_empty() {
                        children.push(Node::Text(t));
                    }
                } else if self.starts_with(b"<?") {
                    self.consume_pi()?;
                } else {
                    let el = self.parse_element()?;
                    children.push(Node::Element(el));
                }
            } else {
                let t = self.parse_text_fast()?;
                if !t.is_empty() {
                    children.push(Node::Text(t));
                }
            }
        }
        Ok(Element {
            name,
            attrs,
            children,
        })
    }

    // <text>
    fn parse_text_fast(&mut self) -> Result<String, String> {
        let start = self.i;
        while !self.eof() && !self.peek_is(b'<') {
            self.i += 1;
        }
        let slice = &self.s[start..self.i];
        let s = std::str::from_utf8(slice).map_err(|_| "utf8 error")?;
        if s.as_bytes().contains(&b'&') {
            Ok(decode_entities(s))
        } else {
            Ok(s.to_string())
        }
    }

    // <name>
    fn parse_name_string(&mut self) -> Result<String, String> {
        if self.eof() {
            return Err("unexpected EOF parsing name".into());
        }
        let start = self.i;
        let mut first = true;
        while !self.eof() {
            let c = self.s[self.i];
            let ok = if first {
                is_name_start(c)
            } else {
                is_name_char(c)
            };
            if !ok {
                break;
            }
            self.i += 1;
            first = false;
        }
        if self.i == start {
            Err("expected name".into())
        } else {
            let out = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "utf8 error")?;
            Ok(out.to_string())
        }
    }

    // <attribute value>
    fn parse_attr_value_fast(&mut self) -> Result<String, String> {
        if self.eof() {
            return Err("unexpected EOF parsing attribute value".into());
        }
        let quote = self.s[self.i];
        if quote != b'"' && quote != b'\'' {
            return Err("expected ' or \" for attribute value".into());
        }
        self.i += 1;
        let start = self.i;
        while !self.eof() && self.s[self.i] != quote {
            self.i += 1;
        }
        if self.eof() {
            return Err("unterminated attribute value".into());
        }
        let raw = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "utf8 error")?;
        self.i += 1;
        if raw.as_bytes().contains(&b'&') {
            Ok(decode_entities(raw))
        } else {
            Ok(raw.to_string())
        }
    }

    // <? ... ?>
    fn consume_pi(&mut self) -> Result<(), String> {
        self.expect_str(b"<?")?;
        let mut last = 0u8;
        while !self.eof() {
            let c = self.next_byte().unwrap();
            if last == b'?' && c == b'>' {
                break;
            }
            last = c;
        }
        Ok(())
    }

    // <!-- ... -->
    fn consume_comment(&mut self) -> Result<String, String> {
        self.expect_str(b"<!--")?;
        let start = self.i;
        while !self.eof() && !self.starts_with(b"-->") {
            self.i += 1;
        }
        if self.eof() {
            return Err("unterminated comment".into());
        }
        let s = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "utf8 error")?;
        self.i += 3;
        Ok(s.to_string())
    }

    // <![CDATA[ ... ]]>
    fn consume_cdata(&mut self) -> Result<String, String> {
        self.expect_str(b"<![CDATA[")?;
        let start = self.i;
        while !self.eof() && !self.starts_with(b"]]>") {
            self.i += 1;
        }
        if self.eof() {
            return Err("unterminated CDATA".into());
        }
        let s = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| "utf8 error")?;
        self.i += 3;
        Ok(s.to_string())
    }

    fn eof(&self) -> bool {
        self.i >= self.n
    }
    fn peek_is(&self, b: u8) -> bool {
        !self.eof() && self.s[self.i] == b
    }
    fn starts_with(&self, pat: &[u8]) -> bool {
        let m = pat.len();
        self.i + m <= self.n && &self.s[self.i..self.i + m] == pat
    }
    fn expect(&mut self, b: u8) -> Result<(), String> {
        if self.eof() || self.s[self.i] != b {
            return Err(format!("expected '{}'", b as char));
        }
        self.i += 1;
        Ok(())
    }
    fn expect_str(&mut self, pat: &[u8]) -> Result<(), String> {
        if !self.starts_with(pat) {
            return Err(format!("expected \"{}\"", String::from_utf8_lossy(pat)));
        }
        self.i += pat.len();
        Ok(())
    }
    fn next_byte(&mut self) -> Option<u8> {
        if self.eof() {
            None
        } else {
            let ch = self.s[self.i];
            self.i += 1;
            Some(ch)
        }
    }
    fn skip_ws(&mut self) {
        while !self.eof() {
            match self.s[self.i] {
                b' ' | b'\n' | b'\r' | b'\t' => self.i += 1,
                _ => break,
            }
        }
    }
}

fn is_name_start(c: u8) -> bool {
    (c >= b'a' && c <= b'z') || (c >= b'A' && c <= b'Z') || c == b'_' || c == b':'
}
fn is_name_char(c: u8) -> bool {
    is_name_start(c) || (c >= b'0' && c <= b'9') || c == b'-' || c == b'.'
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '&' {
            let mut ent = String::new();
            while let Some(&nc) = it.peek() {
                ent.push(nc);
                it.next();
                if nc == ';' {
                    break;
                }
            }
            let repl: String = match ent.as_str() {
                "lt;" => "<".into(),
                "gt;" => ">".into(),
                "amp;" => "&".into(),
                "apos;" => "'".into(),
                "quot;" => "\"".into(),
                _ => {
                    if let Some(hex) = ent.strip_prefix("#x").and_then(|s| s.strip_suffix(';')) {
                        u32::from_str_radix(hex, 16)
                            .ok()
                            .and_then(char::from_u32)
                            .map(|ch| ch.to_string())
                            .unwrap_or_else(|| format!("&{}", ent))
                    } else if let Some(dec) =
                        ent.strip_prefix('#').and_then(|s| s.strip_suffix(';'))
                    {
                        dec.parse::<u32>()
                            .ok()
                            .and_then(char::from_u32)
                            .map(|ch| ch.to_string())
                            .unwrap_or_else(|| format!("&{}", ent))
                    } else {
                        format!("&{}", ent)
                    }
                }
            };
            out.push_str(&repl);
        } else {
            out.push(c);
        }
    }
    out
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

// <spectrumList><spectrum>
fn extract_spectra(root: &Element) -> Vec<SpectrumSummary> {
    let mut out = Vec::new();
    if let Some(list) = root.find_first("spectrumList") {
        for spec in list.children_named("spectrum") {
            if let Some(sum) = spectrum_summary_from_element(spec) {
                out.push(sum);
            }
        }
    }
    out
}

// <spectrum>
fn spectrum_summary_from_element(spec: &Element) -> Option<SpectrumSummary> {
    if spec.name != "spectrum" {
        return None;
    }
    let index = attr_usize(spec, "index").unwrap_or(0);
    let id = spec.attr("id").unwrap_or("").to_string();
    let array_length = attr_usize(spec, "defaultArrayLength").unwrap_or(0);

    let mut cv = Vec::new();
    collect_cvparams(spec, &mut cv);

    let ms_level = cv_value_u32(&cv, "ms level");
    let scan_type = if has_cv_name(&cv, "MS1 spectrum") {
        Some("MS1".into())
    } else if has_cv_name(&cv, "MSn spectrum") {
        Some("MSn".into())
    } else {
        None
    };
    let polarity = if has_cv_name(&cv, "positive scan") {
        Some("positive".into())
    } else if has_cv_name(&cv, "negative scan") {
        Some("negative".into())
    } else {
        None
    };
    let spectrum_type = if has_cv_name(&cv, "profile spectrum") {
        Some("profile".into())
    } else if has_cv_name(&cv, "centroid spectrum") {
        Some("centroid".into())
    } else {
        None
    };
    let total_ion_current = cv_value_f64(&cv, "total ion current");
    let base_peak_intensity = cv_value_f64(&cv, "base peak intensity");
    let base_peak_mz = cv_value_f64(&cv, "base peak m/z");

    let retention_time = cv_param_by_name(&cv, "scan start time").and_then(|p| {
        let v = attr_f64(p, "value")?;
        match p.attr("unitName") {
            Some("second") => Some(v / 60.0),
            _ => Some(v),
        }
    });

    let scan_window_lower_limit = cv_value_f64(&cv, "scan window lower limit");
    let scan_window_upper_limit = cv_value_f64(&cv, "scan window upper limit");

    let (mz_array, intensity_array) = decode_arrays(spec, array_length);

    Some(SpectrumSummary {
        index,
        id,
        array_length,
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

// <cvParam>
fn collect_cvparams<'a>(el: &'a Element, out: &mut Vec<&'a Element>) {
    for ch in &el.children {
        if let Node::Element(e) = ch {
            if e.name == "cvParam" {
                out.push(e);
            }
            collect_cvparams(e, out);
        }
    }
}

// <cvParam>
fn cv_param_by_name<'a>(cv: &'a [&'a Element], name: &str) -> Option<&'a Element> {
    cv.iter().copied().find(|p| p.attr("name") == Some(name))
}

// <cvParam>
fn has_cv_name(cv: &[&Element], name: &str) -> bool {
    cv_param_by_name(cv, name).is_some()
}

// <cvParam>
fn cv_value_f64(cv: &[&Element], name: &str) -> Option<f64> {
    cv_param_by_name(cv, name).and_then(|p| attr_f64(p, "value"))
}

// <cvParam>
fn cv_value_u32(cv: &[&Element], name: &str) -> Option<u32> {
    cv_param_by_name(cv, name).and_then(|p| attr_u32(p, "value"))
}

fn attr_usize(el: &Element, key: &str) -> Option<usize> {
    el.attr(key)?.parse().ok()
}
fn attr_u32(el: &Element, key: &str) -> Option<u32> {
    el.attr(key)?.parse().ok()
}
fn attr_f64(el: &Element, key: &str) -> Option<f64> {
    el.attr(key)?.parse().ok()
}

// <binaryDataArrayList><binaryDataArray><cvParam><binary>
fn decode_arrays(spec: &Element, expected_len: usize) -> (Option<Vec<f64>>, Option<Vec<f64>>) {
    let list = match spec.first_child_named("binaryDataArrayList") {
        Some(x) => x,
        None => return (None, None),
    };
    let mut mz: Option<Vec<f64>> = None;
    let mut inten: Option<Vec<f64>> = None;

    for bda in list.children_named("binaryDataArray") {
        let mut kind_mz = false;
        let mut kind_int = false;
        let mut is_zlib = false;
        let mut is_f64 = false;
        let mut is_f32 = false;
        let mut little = true;

        for p in bda.children_named("cvParam") {
            match p.attr("name") {
                Some("m/z array") => kind_mz = true,
                Some("intensity array") => kind_int = true,
                Some("zlib compression") => is_zlib = true,
                Some("64-bit float") => is_f64 = true,
                Some("32-bit float") => is_f32 = true,
                Some("little endian") => little = true,
                Some("big endian") => little = false,
                _ => {}
            }
        }

        let bin_text = bda
            .first_child_named("binary")
            .and_then(|b| first_text_ref(b));
        if bin_text.is_none() {
            continue;
        }
        let bin_text = bin_text.unwrap();

        let mut bytes = match decode_base64_ws(bin_text) {
            Some(b) => b,
            None => continue,
        };

        if is_zlib {
            if let Ok(d) = decompress_to_vec_zlib(&bytes) {
                bytes = d;
            } else {
                continue;
            }
        }

        if is_f64 {
            let want = if expected_len > 0 {
                expected_len
            } else {
                bytes.len() / 8
            };
            let vals = bytes_to_f64_exact(&bytes, little, want);
            if kind_mz {
                mz = Some(vals);
            } else if kind_int {
                inten = Some(vals);
            }
        } else if is_f32 {
            let want = if expected_len > 0 {
                expected_len
            } else {
                bytes.len() / 4
            };
            let vals = bytes_to_f32_as_f64_exact(&bytes, little, want);
            if kind_mz {
                mz = Some(vals);
            } else if kind_int {
                inten = Some(vals);
            }
        }
    }

    (mz, inten)
}

// <binary>
fn first_text_ref<'a>(el: &'a Element) -> Option<&'a str> {
    for ch in &el.children {
        if let Node::Text(t) = ch {
            let s = t.trim();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
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

#[inline]
fn is_b64_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\r' | b'\t')
}

fn decode_base64_ws(s: &str) -> Option<Vec<u8>> {
    let bytes = s.as_bytes();

    let mut useful = 0usize;
    let mut pads = 0usize;
    for &b in bytes.iter().rev() {
        if is_b64_ws(b) {
            continue;
        }
        if b == b'=' {
            pads += 1;
        } else {
            break;
        }
    }
    for &b in bytes {
        if !is_b64_ws(b) {
            useful += 1;
        }
    }
    if useful == 0 {
        return Some(Vec::new());
    }
    if useful % 4 != 0 {
        return None;
    }

    let out_len = useful / 4 * 3 - pads;
    let mut out = Vec::with_capacity(out_len);

    let inv = &BASE64_INV;
    let mut q = [0u8; 4];
    let mut qi = 0usize;

    for &b in bytes {
        if is_b64_ws(b) {
            continue;
        }
        q[qi] = b;
        qi += 1;
        if qi == 4 {
            let b0 = q[0];
            let b1 = q[1];
            let b2 = q[2];
            let b3 = q[3];

            if b0 == b'=' || b1 == b'=' {
                return None;
            }
            let v0 = inv[b0 as usize];
            if v0 == 255 {
                return None;
            }
            let v1 = inv[b1 as usize];
            if v1 == 255 {
                return None;
            }

            let v2 = if b2 == b'=' {
                0
            } else {
                let v = inv[b2 as usize];
                if v == 255 {
                    return None;
                }
                v
            };
            let v3 = if b3 == b'=' {
                0
            } else {
                let v = inv[b3 as usize];
                if v == 255 {
                    return None;
                }
                v
            };

            let n = ((v0 as u32) << 18) | ((v1 as u32) << 12) | ((v2 as u32) << 6) | (v3 as u32);
            out.push(((n >> 16) & 0xFF) as u8);
            if b2 != b'=' {
                out.push(((n >> 8) & 0xFF) as u8);
            }
            if b3 != b'=' {
                out.push((n & 0xFF) as u8);
            }

            qi = 0;
        }
    }
    if qi != 0 {
        return None;
    }
    Some(out)
}

fn bytes_to_f64_exact(b: &[u8], little: bool, want: usize) -> Vec<f64> {
    let n = b.len() / 8;
    let len = want.min(n);
    let mut out = Vec::with_capacity(len);
    let mut i = 0usize;
    for _ in 0..len {
        let mut arr = [0u8; 8];
        arr.copy_from_slice(&b[i..i + 8]);
        let v = if little {
            f64::from_le_bytes(arr)
        } else {
            f64::from_be_bytes(arr)
        };
        out.push(v);
        i += 8;
    }
    out
}

fn bytes_to_f32_as_f64_exact(b: &[u8], little: bool, want: usize) -> Vec<f64> {
    let n = b.len() / 4;
    let len = want.min(n);
    let mut out = Vec::with_capacity(len);
    let mut i = 0usize;
    for _ in 0..len {
        let mut arr = [0u8; 4];
        arr.copy_from_slice(&b[i..i + 4]);
        let v32 = if little {
            f32::from_le_bytes(arr)
        } else {
            f32::from_be_bytes(arr)
        };
        out.push(v32 as f64);
        i += 4;
    }
    out
}
