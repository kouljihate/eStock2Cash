//! Embedded TrueType text for the PDF writer.
//!
//! A PDF has no text shaping engine: the producer picks the glyphs. This module
//! therefore does the three things a Unicode script like Arabic needs and that
//! a base-14 Helvetica cannot do:
//!
//! 1. parse enough of a TrueType file to map characters to glyph ids and read
//!    their advance widths (`cmap`, `hmtx`, `head`, `hhea`, `maxp`),
//! 2. join Arabic letters into their isolated/final/initial/medial forms (and
//!    the lam-alef ligatures), from the generated table in `arabic_forms.rs`,
//! 3. reorder the result into visual order, so an RTL run is emitted right to
//!    left while embedded Latin and digits keep their own direction.
//!
//! The whole font file is embedded in the PDF (`FontFile2`), so no glyph
//! subsetting is needed - the embedding cost is paid once per document.

use std::sync::OnceLock;

use crate::arabic_forms::{FORMS, LAM_ALEF};

// ---------------------------------------------------------------------------
// Binary parsing helpers
// ---------------------------------------------------------------------------

fn u8_at(d: &[u8], o: usize) -> Option<u8> {
    d.get(o).copied()
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(((u8_at(d, o)? as u16) << 8) | u8_at(d, o + 1)? as u16)
}

fn i16_at(d: &[u8], o: usize) -> Option<i16> {
    u16_at(d, o).map(|v| v as i16)
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(((u16_at(d, o)? as u32) << 16) | u16_at(d, o + 2)? as u32)
}

// ---------------------------------------------------------------------------
// cmap
// ---------------------------------------------------------------------------

enum Cmap {
    /// Format 4: segment mapping to delta values.
    Seg4 {
        ends: Vec<u16>,
        starts: Vec<u16>,
        deltas: Vec<i16>,
        range_offsets: Vec<u16>,
        /// File offset of each `idRangeOffset` entry, needed by the format.
        range_pos: Vec<usize>,
        data: Vec<u8>,
    },
    /// Format 12: segmented coverage.
    Groups(Vec<(u32, u32, u32)>),
}

impl Cmap {
    fn lookup(&self, c: u32) -> Option<u16> {
        match self {
            Cmap::Seg4 {
                ends,
                starts,
                deltas,
                range_offsets,
                range_pos,
                data,
            } => {
                if c > 0xFFFF {
                    return None;
                }
                let c = c as u16;
                // Segments are sorted by end code.
                let i = ends.partition_point(|&e| e < c);
                if i >= ends.len() {
                    return None;
                }
                if c < starts[i] {
                    return None;
                }
                let ro = range_offsets[i];
                if ro == 0 {
                    return Some(c.wrapping_add(deltas[i] as u16));
                }
                let addr = range_pos[i] + ro as usize + 2 * (c - starts[i]) as usize;
                let gid = u16_at(data, addr)?;
                if gid == 0 {
                    None
                } else {
                    Some(gid.wrapping_add(deltas[i] as u16))
                }
            }
            Cmap::Groups(groups) => {
                let i = groups.partition_point(|&(start, _, _)| start <= c);
                if i == 0 {
                    return None;
                }
                let (start, end, gid) = groups[i - 1];
                if c > end {
                    return None;
                }
                Some((gid + (c - start)) as u16)
            }
        }
    }
}

fn parse_cmap(data: &[u8], base: usize) -> Option<Cmap> {
    let count = u16_at(data, base + 2)? as usize;
    let mut best: Option<(u32, usize)> = None; // (preference, subtable offset)
    for i in 0..count {
        let rec = base + 4 + i * 8;
        let platform = u16_at(data, rec)?;
        let encoding = u16_at(data, rec + 2)?;
        let offset = u32_at(data, rec + 4)? as usize;
        // Prefer full Unicode coverage, then BMP Unicode, then any.
        let score = match (platform, encoding) {
            (3, 10) | (0, 4) | (0, 6) => 4,
            (3, 1) | (0, 3) => 3,
            (0, 0) | (0, 1) | (0, 2) => 2,
            _ => 1,
        };
        if best.map(|(s, _)| score > s).unwrap_or(true) {
            best = Some((score, base + offset));
        }
    }
    let sub = best?.1;
    let format = u16_at(data, sub)?;
    match format {
        4 => {
            let seg_x2 = u16_at(data, sub + 6)? as usize;
            let seg_count = seg_x2 / 2;
            let ends_at = sub + 14;
            let starts_at = ends_at + seg_x2 + 2; // + reservedPad
            let deltas_at = starts_at + seg_x2;
            let ranges_at = deltas_at + seg_x2;
            let mut ends = Vec::with_capacity(seg_count);
            let mut starts = Vec::with_capacity(seg_count);
            let mut deltas = Vec::with_capacity(seg_count);
            let mut range_offsets = Vec::with_capacity(seg_count);
            let mut range_pos = Vec::with_capacity(seg_count);
            for i in 0..seg_count {
                ends.push(u16_at(data, ends_at + i * 2)?);
                starts.push(u16_at(data, starts_at + i * 2)?);
                deltas.push(i16_at(data, deltas_at + i * 2)?);
                range_offsets.push(u16_at(data, ranges_at + i * 2)?);
                range_pos.push(ranges_at + i * 2);
            }
            Some(Cmap::Seg4 {
                ends,
                starts,
                deltas,
                range_offsets,
                range_pos,
                data: data.to_vec(),
            })
        }
        12 => {
            let groups = u32_at(data, sub + 12)? as usize;
            let mut out = Vec::with_capacity(groups);
            for i in 0..groups {
                let g = sub + 16 + i * 12;
                out.push((
                    u32_at(data, g)?,
                    u32_at(data, g + 4)?,
                    u32_at(data, g + 8)?,
                ));
            }
            Some(Cmap::Groups(out))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Font
// ---------------------------------------------------------------------------

pub struct TrueTypeFont {
    data: Vec<u8>,
    pub name: String,
    units_per_em: u16,
    bbox: [i16; 4],
    ascent: i16,
    descent: i16,
    advances: Vec<u16>,
    cmap: Cmap,
    /// Flate-compressed font program, produced on first use.
    payload: OnceLock<Vec<u8>>,
}

impl TrueTypeFont {
    pub fn parse(data: Vec<u8>, name: &str) -> Result<Self, String> {
        if u32_at(&data, 0) != Some(0x0001_0000) {
            return Err("not a TrueType file (bad sfnt version)".into());
        }
        let num_tables = u16_at(&data, 4).ok_or("truncated table directory")?;
        let table = |tag: &[u8; 4]| -> Option<usize> {
            (0..num_tables).find_map(|i| {
                let rec = 12 + i as usize * 16;
                if data.get(rec..rec + 4)? == tag {
                    u32_at(&data, rec + 8).map(|o| o as usize)
                } else {
                    None
                }
            })
        };
        let head = table(b"head").ok_or("missing head table")?;
        let hhea = table(b"hhea").ok_or("missing hhea table")?;
        let maxp = table(b"maxp").ok_or("missing maxp table")?;
        let cmap_at = table(b"cmap").ok_or("missing cmap table")?;
        let hmtx = table(b"hmtx").ok_or("missing hmtx table")?;

        let units_per_em = u16_at(&data, head + 18).ok_or("bad head table")?;
        if units_per_em == 0 {
            return Err("invalid unitsPerEm".into());
        }
        let bbox = [
            i16_at(&data, head + 36).ok_or("bad head bbox")?,
            i16_at(&data, head + 38).ok_or("bad head bbox")?,
            i16_at(&data, head + 40).ok_or("bad head bbox")?,
            i16_at(&data, head + 42).ok_or("bad head bbox")?,
        ];
        let ascent = i16_at(&data, hhea + 4).ok_or("bad hhea")?;
        let descent = i16_at(&data, hhea + 6).ok_or("bad hhea")?;
        let num_h_metrics = u16_at(&data, hhea + 34).ok_or("bad hhea")? as usize;
        let num_glyphs = u16_at(&data, maxp + 4).ok_or("bad maxp")? as usize;
        if num_h_metrics == 0 {
            return Err("font has no horizontal metrics".into());
        }

        let mut advances = Vec::with_capacity(num_glyphs);
        for gid in 0..num_glyphs {
            let idx = gid.min(num_h_metrics - 1);
            advances.push(u16_at(&data, hmtx + idx * 4).ok_or("bad hmtx table")?);
        }

        let cmap = parse_cmap(&data, cmap_at).ok_or("unsupported cmap format")?;

        Ok(Self {
            data,
            name: name.to_string(),
            units_per_em,
            bbox,
            ascent,
            descent,
            advances,
            cmap,
            payload: OnceLock::new(),
        })
    }

    pub fn glyph(&self, c: char) -> Option<u16> {
        self.cmap.lookup(c as u32)
    }

    pub fn advance(&self, gid: u16) -> u16 {
        self.advances.get(gid as usize).copied().unwrap_or(0)
    }

    /// Width of an already laid-out run, in points.
    pub fn run_width(&self, run: &[(u16, Vec<u32>)], size: f64) -> f64 {
        run.iter()
            .map(|&(gid, _)| self.advance(gid) as f64)
            .sum::<f64>()
            * size
            / self.units_per_em as f64
    }

    pub fn width(&self, text: &str, size: f64) -> f64 {
        self.layout(text)
            .iter()
            .map(|piece| match piece {
                Piece::Arabic(run) => self.run_width(run, size),
                Piece::Latin(s) => super::pdf::latin_width(s, size, false),
            })
            .sum()
    }

    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    pub fn bbox(&self) -> [i16; 4] {
        self.bbox
    }

    pub fn ascent_descent(&self) -> (i16, i16) {
        (self.ascent, self.descent)
    }

    /// Flate-compressed font program, for the `FontFile2` stream.
    pub fn compressed(&self) -> &[u8] {
        self.payload.get_or_init(|| {
            use std::io::Write as _;
            let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
            let _ = enc.write_all(&self.data);
            enc.finish().unwrap_or_default()
        })
    }

    pub fn raw_len(&self) -> usize {
        self.data.len()
    }

    /// Lay the text out in visual order: RTL runs reversed, Latin runs kept
    /// as-is, Arabic letters replaced by their context-sensitive forms.
    pub fn layout(&self, text: &str) -> Vec<Piece> {
        let clusters = shape_clusters(text);
        if clusters.is_empty() {
            return Vec::new();
        }
        let rtl_base = clusters
            .iter()
            .find(|c| c.dir != Dir::Neutral)
            .map(|c| c.dir == Dir::Rtl)
            .unwrap_or(false);

        // Give every neutral cluster the direction of its neighbours, so a
        // space stays with the words around it.
        let mut dirs: Vec<Dir> = clusters.iter().map(|c| c.dir).collect();
        for i in 0..dirs.len() {
            if dirs[i] == Dir::Neutral {
                dirs[i] = dirs[..i]
                    .iter()
                    .rev()
                    .find(|d| **d != Dir::Neutral)
                    .copied()
                    .or_else(|| dirs[i + 1..].iter().find(|d| **d != Dir::Neutral).copied())
                    .unwrap_or(if rtl_base { Dir::Rtl } else { Dir::Ltr });
            }
        }

        let mut runs: Vec<(Dir, Vec<usize>)> = Vec::new();
        for (i, dir) in dirs.iter().enumerate() {
            match runs.last_mut() {
                Some((d, idx)) if d == dir => idx.push(i),
                _ => runs.push((*dir, vec![i])),
            }
        }
        // Visual order: with an RTL paragraph, runs start from the left with
        // the last logical run.
        if rtl_base {
            runs.reverse();
        }

        let mut pieces = Vec::new();
        for (dir, idxs) in runs {
            match dir {
                Dir::Rtl => {
                    let mut run = Vec::new();
                    for &i in idxs.iter().rev() {
                        for (k, ch) in clusters[i].chars.iter().enumerate() {
                            let gid = self
                                .glyph(*ch)
                                .or_else(|| self.glyph(' '))
                                .unwrap_or(0);
                            // Only the base glyph carries the source text; a
                            // combining mark maps to itself.
                            let uni = if k == 0 {
                                clusters[i].source.clone()
                            } else {
                                vec![*ch as u32]
                            };
                            run.push((gid, uni));
                        }
                    }
                    if !run.is_empty() {
                        pieces.push(Piece::Arabic(run));
                    }
                }
                _ => {
                    let mut s = String::new();
                    for &i in &idxs {
                        for ch in &clusters[i].chars {
                            s.push(mirror_in_rtl(*ch, rtl_base));
                        }
                    }
                    if !s.is_empty() {
                        pieces.push(Piece::Latin(s));
                    }
                }
            }
        }
        pieces
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dir {
    Rtl,
    Ltr,
    Neutral,
}

struct Cluster {
    /// Glyph characters to look up, in drawing order within the cluster.
    chars: Vec<char>,
    /// The logical text behind the first glyph (for `ToUnicode`), so copying
    /// "فاتورة" gives base letters and not presentation forms.
    source: Vec<u32>,
    dir: Dir,
}

/// True when the string contains characters Helvetica/WinAnsi cannot encode.
pub fn needs_embedded_face(text: &str) -> bool {
    text.chars().any(is_arabic)
}

fn is_arabic(c: char) -> bool {
    matches!(c as u32,
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF |
        0xFB50..=0xFDFF | 0xFE70..=0xFEFF)
}

fn is_mark(c: char) -> bool {
    matches!(c as u32,
        0x0610..=0x061A | 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06DC |
        0x06DF..=0x06E4 | 0x06E7..=0x06E8 | 0x06EA..=0x06ED)
}

fn form(c: char) -> Option<(u32, u32, u32, u32, u32)> {
    let cp = c as u32;
    FORMS
        .binary_search_by_key(&cp, |row| row.0)
        .ok()
        .map(|i| FORMS[i])
}

fn joins_forward(c: char) -> bool {
    form(c).map(|f| f.3 != 0).unwrap_or(false)
}

fn joins_backward(c: char) -> bool {
    // A letter can connect to the one before it only if it has a final form.
    form(c).map(|f| f.2 != 0).unwrap_or(false)
}

fn lam_alef(lam_joins: bool, alef: char) -> Option<char> {
    let alef_cp = alef as u32;
    LAM_ALEF
        .iter()
        .find(|&&(a, final_, _)| a == alef_cp && final_ == lam_joins)
        .map(|&(_, _, gid)| char::from_u32(gid))
        .flatten()
}

/// Split, join and classify the text into drawable clusters.
fn shape_clusters(text: &str) -> Vec<Cluster> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Cluster> = Vec::new();
    let mut prev_logical: Option<char> = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        // Combining marks belong to the preceding cluster.
        if is_mark(c) {
            if let Some(last) = out.last_mut() {
                last.chars.push(c);
                i += 1;
                continue;
            }
        }

        // Joining depends on the *logical* neighbour, never on the glyph we
        // substituted for it.
        let prev_joins = prev_logical.map(joins_forward).unwrap_or(false);

        // Lam followed by an alef becomes one ligature glyph.
        if c == '\u{0644}' {
            if let Some(&next) = chars.get(i + 1) {
                if let Some(lig) = lam_alef(prev_joins, next) {
                    out.push(Cluster {
                        chars: vec![lig],
                        source: vec![c as u32, next as u32],
                        dir: Dir::Rtl,
                    });
                    prev_logical = Some(next);
                    i += 2;
                    continue;
                }
            }
        }

        if let Some(f) = form(c) {
            let next_joins = chars
                .get(i + 1)
                .map(|&n| joins_backward(n) && joins_forward(c))
                .unwrap_or(false);
            let chosen = match (prev_joins, next_joins) {
                (true, true) if f.4 != 0 => f.4,
                (true, _) if f.2 != 0 => f.2,
                (_, true) if f.3 != 0 => f.3,
                _ => f.1,
            };
            let ch = char::from_u32(chosen).unwrap_or(c);
            out.push(Cluster {
                chars: vec![ch],
                source: vec![c as u32],
                dir: Dir::Rtl,
            });
            prev_logical = Some(c);
            i += 1;
            continue;
        }

        let dir = if is_arabic(c) {
            Dir::Rtl
        } else if c.is_ascii_alphanumeric() || c.is_alphanumeric() {
            Dir::Ltr
        } else {
            Dir::Neutral
        };
        out.push(Cluster {
            chars: vec![c],
            source: vec![c as u32],
            dir,
        });
        prev_logical = Some(c);
        i += 1;
    }
    out
}

/// Parentheses and friends are mirrored when they sit inside an RTL run.
fn mirror_in_rtl(c: char, rtl: bool) -> char {
    if !rtl {
        return c;
    }
    match c {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        _ => c,
    }
}

/// A laid-out run: Arabic glyph ids, or Latin text for the base-14 font.
pub enum Piece {
    /// `(glyph id, the characters it stands for)`.
    Arabic(Vec<(u16, Vec<u32>)>),
    Latin(String),
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// The Arabic face used for PDFs, loaded once from `static/fonts/arabic.ttf`
/// (override with `PDF_ARABIC_FONT`). Missing or invalid files are not fatal:
/// the writer falls back to the Latin-only layout.
pub fn arabic() -> Option<&'static TrueTypeFont> {
    static FONT: OnceLock<Option<TrueTypeFont>> = OnceLock::new();
    FONT.get_or_init(|| {
        let path = std::env::var("PDF_ARABIC_FONT")
            .unwrap_or_else(|_| "static/fonts/arabic.ttf".to_string());
        let data = match std::fs::read(&path) {
            Ok(data) => data,
            Err(err) => {
                tracing::debug!(path, %err, "no Arabic PDF font; Arabic documents use Latin labels");
                return None;
            }
        };
        match TrueTypeFont::parse(data, "ArabicPDF") {
            Ok(font) => {
                tracing::info!(path, glyphs = font.advances.len(), "loaded Arabic PDF font");
                Some(font)
            }
            Err(err) => {
                tracing::warn!(path, %err, "invalid Arabic PDF font");
                None
            }
        }
    })
    .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_letters_by_position() {
        // "ببب": initial, medial, final.
        let clusters = shape_clusters("ببب");
        let forms: Vec<char> = clusters.iter().map(|c| c.chars[0]).collect();
        assert_eq!(forms, vec!['\u{FE91}', '\u{FE92}', '\u{FE90}']);
    }

    #[test]
    fn respects_right_joining_letters() {
        // Alef is right-joining: it takes its isolated form and cannot pass the
        // connection on, so the following beh stays isolated too.
        let clusters = shape_clusters("اب");
        assert_eq!(clusters[0].chars[0], '\u{FE8D}');
        assert_eq!(clusters[1].chars[0], '\u{FE8F}');

        // Beh joins forward, so it hands the connection to the next letter.
        let clusters = shape_clusters("بب");
        assert_eq!(clusters[0].chars[0], '\u{FE91}');
        assert_eq!(clusters[1].chars[0], '\u{FE90}');
    }

    #[test]
    fn builds_lam_alef_ligature() {
        let clusters = shape_clusters("لا");
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].chars[0], '\u{FEFB}');
    }

    #[test]
    fn marks_stay_with_their_letter() {
        let clusters = shape_clusters("بَ");
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].chars[0], '\u{FE8F}');
        assert_eq!(clusters[0].chars[1], '\u{064E}');
    }

    #[test]
    fn reorders_rtl_and_keeps_latin_runs() {
        let font = TrueTypeFont::parse(
            std::fs::read("static/fonts/arabic.ttf").expect("bundled font"),
            "Test",
        )
        .expect("parse bundled font");
        // The Arabic words end up reversed, the code keeps its own direction.
        let pieces = font.layout("سعر SKU-2");
        let mut latin = None;
        for piece in &pieces {
            if let Piece::Latin(s) = piece {
                latin = Some(s.clone());
            }
        }
        assert_eq!(latin.as_deref(), Some("SKU-2"));
        // With an RTL paragraph the Latin run comes first, left to right.
        assert!(matches!(pieces.first(), Some(Piece::Latin(_))));
        assert!(matches!(pieces.last(), Some(Piece::Arabic(_))));
    }
}
