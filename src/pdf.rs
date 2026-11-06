//! Minimal, dependency-free PDF writer for invoices and tickets.
//!
//! Latin text uses the built-in Helvetica faces and WinAnsi encoding, so a
//! French or English document needs no font embedding and renders identically
//! everywhere. Arabic cannot be expressed that way: it needs a Unicode font and
//! contextual glyph selection, so when `static/fonts/arabic.ttf` is present the
//! writer embeds it (Type0 / Identity-H) and shapes the text with [`crate::fonts`].
//! Company branding (primary/accent colours, header, footer, JPEG logo) comes
//! from the invoice template.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;

use crate::fonts::{self, Piece, TrueTypeFont};
use crate::i18n::Lang;
use crate::models::{Company, Invoice, InvoiceTemplate, Ticket};
use crate::util::{money, qty};

const PAGE_W: f64 = 595.28;
const PAGE_H: f64 = 841.89;

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

fn pdf_escape(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 4);
    for ch in text.chars() {
        match ch {
            '(' => out.extend_from_slice(b"\\("),
            ')' => out.extend_from_slice(b"\\)"),
            '\\' => out.extend_from_slice(b"\\\\"),
            c if (c as u32) < 32 => out.push(b' '),
            c if (c as u32) < 127 => out.push(c as u8),
            // Latin-1 accents map straight to WinAnsi; anything else degrades.
            c if (c as u32) <= 255 => out.push(c as u32 as u8),
            _ => out.push(b'?'),
        }
    }
    out
}

/// Approximate Helvetica advance width (in 1/1000 em).
fn char_width(ch: char, bold: bool) -> f64 {
    let base = match ch {
        'i' | 'l' | 'j' | 't' | 'f' | 'r' | ' ' | '.' | ',' | ':' | ';' | '\'' | '|' => 30.0,
        'm' | 'w' | 'M' | 'W' => 85.0,
        'A'..='Z' => 70.0,
        '0'..='9' => 56.0,
        '-' | '/' | '(' | ')' | '[' | ']' => 35.0,
        _ => 52.0,
    };
    if bold { base * 1.06 } else { base }
}

pub fn latin_width(text: &str, size: f64, bold: bool) -> f64 {
    text.chars().map(|c| char_width(c, bold)).sum::<f64>() * size / 1000.0
}

fn rgb(hex: &str) -> (f64, f64, f64) {
    let h = hex.trim().trim_start_matches('#');
    let expand = |s: &str| {
        let v = u8::from_str_radix(s, 16).unwrap_or(0);
        v as f64 / 255.0
    };
    match h.len() {
        6 => (
            expand(&h[0..2]),
            expand(&h[2..4]),
            expand(&h[4..6]),
        ),
        3 => (
            expand(&format!("{}{}", &h[0..1], &h[0..1])),
            expand(&format!("{}{}", &h[1..2], &h[1..2])),
            expand(&format!("{}{}", &h[2..3], &h[2..3])),
        ),
        _ => (0.12, 0.16, 0.24),
    }
}

// ---------------------------------------------------------------------------
// Page content
// ---------------------------------------------------------------------------

type GlyphUse = Rc<RefCell<BTreeMap<u16, Vec<u32>>>>;

pub struct Content {
    ops: Vec<u8>,
    /// Embedded face, available to any document that needs it.
    font: Option<&'static TrueTypeFont>,
    /// Draw every string with the embedded face (Arabic documents), instead of
    /// only the strings that WinAnsi cannot encode.
    force_font: bool,
    /// Glyph ids drawn so far, mapped to the character they came from, so the
    /// document can emit a `ToUnicode` CMap (text stays selectable/searchable).
    used: GlyphUse,
}

impl Content {
    fn new(font: Option<&'static TrueTypeFont>, force_font: bool, used: GlyphUse) -> Self {
        Self {
            ops: Vec::new(),
            font,
            force_font,
            used,
        }
    }

    /// The face this string must be drawn with, if the base-14 fonts will not do.
    fn face_for(&self, text: &str) -> Option<&'static TrueTypeFont> {
        let font = self.font?;
        if self.force_font || fonts::needs_embedded_face(text) {
            Some(font)
        } else {
            None
        }
    }

    fn measure(&self, text: &str, size: f64, bold: bool) -> f64 {
        match self.face_for(text) {
            Some(font) => font.width(text, size),
            None => latin_width(text, size, bold),
        }
    }

    fn raw(&mut self, s: &str) {
        self.ops.extend_from_slice(s.as_bytes());
        self.ops.push(b'\n');
    }

    fn fill_color(&mut self, hex: &str) -> (f64, f64, f64) {
        let (r, g, b) = rgb(hex);
        self.raw(&format!("{r:.3} {g:.3} {b:.3} rg"));
        (r, g, b)
    }

    fn stroke_color(&mut self, hex: &str) {
        let (r, g, b) = rgb(hex);
        self.raw(&format!("{r:.3} {g:.3} {b:.3} RG"));
    }

    fn rect(&mut self, x: f64, y_top: f64, w: f64, h: f64, hex: &str) {
        self.fill_color(hex);
        self.raw(&format!(
            "{x:.2} {:.2} {w:.2} {h:.2} re f",
            PAGE_H - y_top - h
        ));
    }

    fn line(&mut self, x1: f64, y1_top: f64, x2: f64, y2_top: f64, hex: &str, width: f64) {
        self.stroke_color(hex);
        self.raw(&format!("{width:.2} w"));
        self.raw(&format!(
            "{x1:.2} {:.2} m {x2:.2} {:.2} l S",
            PAGE_H - y1_top,
            PAGE_H - y2_top
        ));
    }

    fn text(&mut self, x: f64, y_top: f64, size: f64, bold: bool, hex: &str, text: &str) {
        if text.is_empty() {
            return;
        }
        let baseline = PAGE_H - y_top;
        let Some(font) = self.face_for(text) else {
            self.fill_color(hex);
            let name = if bold { "F2" } else { "F1" };
            let mut s = Vec::new();
            s.extend_from_slice(
                format!("BT /{name} {size:.2} Tf 1 0 0 1 {x:.2} {baseline:.2} Tm ").as_bytes(),
            );
            s.push(b'(');
            s.extend_from_slice(&pdf_escape(text));
            s.extend_from_slice(b") Tj ET\n");
            self.ops.extend_from_slice(&s);
            return;
        };

        // Mixed content is drawn run by run: Arabic glyphs from the embedded
        // face, Latin and digits from Helvetica, each advancing the cursor.
        let pieces = font.layout(text);
        let mut cursor = x;
        for piece in pieces {
            match piece {
                Piece::Arabic(run) => {
                    let mut hexs = String::with_capacity(run.len() * 4);
                    for (gid, uni) in &run {
                        hexs.push_str(&format!("{gid:04X}"));
                        self.used
                            .borrow_mut()
                            .entry(*gid)
                            .or_insert_with(|| uni.clone());
                    }
                    let (r, g, b) = rgb(hex);
                    if bold {
                        // The face ships a single weight; thicken the fill with
                        // a stroke in the same colour.
                        self.raw(&format!(
                            "q {r:.3} {g:.3} {b:.3} rg {r:.3} {g:.3} {b:.3} RG\
                             {:.2} w 2 Tr BT /F3 {size:.2} Tf 1 0 0 1 {cursor:.2} {baseline:.2} Tm <{hexs}> Tj ET 0 Tr Q",
                            size * 0.04
                        ));
                    } else {
                        self.raw(&format!(
                            "{r:.3} {g:.3} {b:.3} rg BT /F3 {size:.2} Tf 1 0 0 1 {cursor:.2} {baseline:.2} Tm <{hexs}> Tj ET"
                        ));
                    }
                    cursor += font.run_width(&run, size);
                }
                Piece::Latin(run) => {
                    self.fill_color(hex);
                    let name = if bold { "F2" } else { "F1" };
                    let mut s = Vec::new();
                    s.extend_from_slice(
                        format!("BT /{name} {size:.2} Tf 1 0 0 1 {cursor:.2} {baseline:.2} Tm ")
                            .as_bytes(),
                    );
                    s.push(b'(');
                    s.extend_from_slice(&pdf_escape(&run));
                    s.extend_from_slice(b") Tj ET\n");
                    self.ops.extend_from_slice(&s);
                    cursor += latin_width(&run, size, bold);
                }
            }
        }
    }

    fn text_right(&mut self, right: f64, y_top: f64, size: f64, bold: bool, hex: &str, text: &str) {
        let w = self.measure(text, size, bold);
        self.text(right - w, y_top, size, bold, hex, text);
    }

    fn text_center(&mut self, center: f64, y_top: f64, size: f64, bold: bool, hex: &str, text: &str) {
        let w = self.measure(text, size, bold);
        self.text(center - w / 2.0, y_top, size, bold, hex, text);
    }
}

// ---------------------------------------------------------------------------
// PDF assembly
// ---------------------------------------------------------------------------

struct JpegInfo {
    width: u32,
    height: u32,
    components: u8,
}

fn jpeg_info(data: &[u8]) -> Option<JpegInfo> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 9 < data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = data[i + 1];
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        let len = ((data[i + 2] as usize) << 8) | data[i + 3] as usize;
        let is_sof = (0xC0..=0xCF).contains(&marker)
            && marker != 0xC4
            && marker != 0xC8
            && marker != 0xCC;
        if is_sof && i + 9 < data.len() {
            let height = ((data[i + 5] as u32) << 8) | data[i + 6] as u32;
            let width = ((data[i + 7] as u32) << 8) | data[i + 8] as u32;
            let components = data[i + 9];
            return Some(JpegInfo {
                width,
                height,
                components,
            });
        }
        i += 2 + len;
    }
    None
}

fn decode_logo(company: &Company) -> Option<(Vec<u8>, JpegInfo)> {
    if company.logo.trim().is_empty() {
        return None;
    }
    let raw = company
        .logo
        .split(',')
        .last()
        .unwrap_or(&company.logo)
        .trim();
    let bytes = B64.decode(raw).ok()?;
    let info = jpeg_info(&bytes)?;
    Some((bytes, info))
}

struct PdfDoc {
    pages: Vec<Content>,
    logo: Option<(Vec<u8>, JpegInfo)>,
    page_w: f64,
    font: Option<&'static TrueTypeFont>,
    force_font: bool,
    used: GlyphUse,
}

impl PdfDoc {
    fn new(
        logo: Option<(Vec<u8>, JpegInfo)>,
        page_w: f64,
        font: Option<&'static TrueTypeFont>,
        force_font: bool,
    ) -> Self {
        Self {
            pages: Vec::new(),
            logo,
            page_w,
            font,
            force_font,
            used: Rc::new(RefCell::new(BTreeMap::new())),
        }
    }

    fn page(&mut self) -> &mut Content {
        if self.pages.is_empty() {
            let used = self.used.clone();
            self.pages.push(Content::new(self.font, self.force_font, used));
        }
        self.pages.last_mut().unwrap()
    }

    fn new_page(&mut self) {
        let used = self.used.clone();
        self.pages.push(Content::new(self.font, self.force_font, used));
    }

    fn finish(self) -> Vec<u8> {
        // Fixed objects: 1 catalog, 2 pages, 3 Helvetica, 4 Helvetica-Bold.
        // Optional objects follow: five for an embedded face (Type0, CIDFont,
        // descriptor, font program, ToUnicode), one for the logo; then two per
        // page (page, content stream).
        let used = self.used.borrow().clone();
        // Embed the face only when some glyph was actually drawn with it.
        let font = self.font.filter(|_| !used.is_empty());
        let has_logo = self.logo.is_some();
        let page_count = self.pages.len().max(1);

        let mut next = 5usize;
        let font_ids = font.map(|_| {
            let first = next;
            next += 5;
            (first, first + 1, first + 2, first + 3, first + 4)
        });
        let logo_id = if has_logo {
            let id = next;
            next += 1;
            Some(id)
        } else {
            None
        };
        let first_page_obj = next;

        let mut objects: Vec<Vec<u8>> = Vec::new();

        objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        let kids = (0..page_count)
            .map(|i| format!("{} 0 R", first_page_obj + i * 2))
            .collect::<Vec<_>>()
            .join(" ");
        objects.push(
            format!("<< /Type /Pages /Kids [{kids}] /Count {page_count} >>").into_bytes(),
        );
        objects.push(
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        );
        objects.push(
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        );

        if let (Some(font), Some((_type0, cid, descriptor, program, to_unicode))) = (font, font_ids) {
            objects.push(
                format!(
                    "<< /Type /Font /Subtype /Type0 /BaseFont /{} /Encoding /Identity-H \
                     /DescendantFonts [{cid} 0 R] /ToUnicode {to_unicode} 0 R >>",
                    font.name
                )
                .into_bytes(),
            );

            let upem = font.units_per_em() as f64;
            let mut widths = String::from("[");
            for &gid in used.keys() {
                let w = (font.advance(gid) as f64 * 1000.0 / upem).round() as i64;
                widths.push_str(&format!("{gid} [{w}] "));
            }
            widths.push(']');
            objects.push(
                format!(
                    "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{} \
                     /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
                     /FontDescriptor {descriptor} 0 R /DW 600 /W {widths} /CIDToGIDMap /Identity >>",
                    font.name
                )
                .into_bytes(),
            );

            let scale = |v: i16| (v as f64 * 1000.0 / upem).round() as i64;
            let [x0, y0, x1, y1] = font.bbox();
            let (ascent, descent) = font.ascent_descent();
            objects.push(
                format!(
                    "<< /Type /FontDescriptor /FontName /{} /Flags 32 /FontBBox [{} {} {} {}] \
                     /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile2 {program} 0 R >>",
                    font.name,
                    scale(x0),
                    scale(y0),
                    scale(x1),
                    scale(y1),
                    scale(ascent),
                    scale(descent),
                    scale(ascent * 7 / 10),
                )
                .into_bytes(),
            );

            let program_data = font.compressed();
            let mut stream = format!(
                "<< /Length {} /Length1 {} /Filter /FlateDecode >>\nstream\n",
                program_data.len(),
                font.raw_len()
            )
            .into_bytes();
            stream.extend_from_slice(program_data);
            stream.extend_from_slice(b"\nendstream");
            objects.push(stream);

            // Glyph id -> character, so the text can be selected and searched.
            let mut cmap = String::from(
                "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
                 /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
                 /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
                 1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
            );
            let entries: Vec<(&u16, &Vec<u32>)> = used.iter().collect();
            for chunk in entries.chunks(100) {
                cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
                for (gid, source) in chunk {
                    let mut utf16 = String::new();
                    for cp in source.iter() {
                        if *cp <= 0xFFFF {
                            utf16.push_str(&format!("{cp:04X}"));
                        } else {
                            let v = cp - 0x10000;
                            utf16.push_str(&format!("{:04X}{:04X}", 0xD800 + (v >> 10), 0xDC00 + (v & 0x3FF)));
                        }
                    }
                    if !utf16.is_empty() {
                        cmap.push_str(&format!("<{gid:04X}> <{utf16}>\n"));
                    }
                }
                cmap.push_str("endbfchar\n");
            }
            cmap.push_str(
                "endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n",
            );
            let mut obj = format!("<< /Length {} >>\nstream\n", cmap.len()).into_bytes();
            obj.extend_from_slice(cmap.as_bytes());
            obj.extend_from_slice(b"\nendstream");
            objects.push(obj);
        }

        if let Some((data, info)) = self.logo {
            let cs = match info.components {
                1 => "DeviceGray",
                4 => "DeviceCMYK",
                _ => "DeviceRGB",
            };
            let mut obj = Vec::new();
            obj.extend_from_slice(
                format!(
                    "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /{cs} /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
                    info.width,
                    info.height,
                    data.len()
                )
                .as_bytes(),
            );
            obj.extend_from_slice(&data);
            obj.extend_from_slice(b"\nendstream");
            objects.push(obj);
        }

        let xobject = match logo_id {
            Some(id) => format!(" /XObject << /Im0 {id} 0 R >>"),
            None => String::new(),
        };
        let embedded = match font_ids {
            Some((type0, ..)) => format!(" /F3 {type0} 0 R"),
            None => String::new(),
        };

        for (i, page) in self.pages.iter().enumerate() {
            let page_obj = first_page_obj + i * 2;
            let content_obj = page_obj + 1;
            objects.push(
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {PAGE_H:.2}] \
                     /Resources << /Font << /F1 3 0 R /F2 4 0 R{embedded} >>{xobject} >> /Contents {content_obj} 0 R >>",
                    self.page_w
                )
                .into_bytes(),
            );
            let mut content = Vec::new();
            content.extend_from_slice(format!("<< /Length {} >>\nstream\n", page.ops.len()).as_bytes());
            content.extend_from_slice(&page.ops);
            content.extend_from_slice(b"\nendstream");
            objects.push(content);
        }

        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        let mut offsets = Vec::with_capacity(objects.len());
        for (i, obj) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(obj);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref_pos = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        out
    }
}

// ---------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------

pub struct PdfLabels {
    /// True when the labels and the document body are Arabic and the embedded
    /// face is available.
    pub arabic: bool,
    pub invoice: String,
    pub number: String,
    pub date: String,
    pub due_date: String,
    pub bill_to: String,
    pub from: String,
    pub description: String,
    pub qty: String,
    pub unit_price: String,
    pub tax: String,
    pub amount: String,
    pub subtotal: String,
    pub total: String,
    pub paid: String,
    pub outstanding: String,
    pub reference: String,
    pub ticket: String,
    pub cashier: String,
    pub change: String,
    pub thank_you: String,
}

impl PdfLabels {
    pub fn new(lang: Lang) -> Self {
        // Arabic needs an embedded shaping font; without one, fall back to
        // English so the document never shows blanks.
        let arabic = matches!(lang, Lang::Ar) && fonts::arabic().is_some();
        let l = if matches!(lang, Lang::Ar) && !arabic {
            Lang::En
        } else {
            lang
        };
        let t = |key: &str| crate::i18n::tr_text(l, key);
        Self {
            arabic,
            invoice: match l {
                Lang::Fr => "FACTURE".into(),
                Lang::Ar => "فاتورة".into(),
                Lang::En => "INVOICE".into(),
            },
            number: t("lbl_number"),
            date: t("lbl_date"),
            due_date: t("lbl_due_date"),
            bill_to: match l {
                Lang::Fr => "Facturé à".into(),
                Lang::Ar => "إلى".into(),
                Lang::En => "Bill to".into(),
            },
            from: match l {
                Lang::Fr => "Émetteur".into(),
                Lang::Ar => "من".into(),
                Lang::En => "From".into(),
            },
            description: t("lbl_description"),
            qty: t("lbl_quantity"),
            unit_price: t("lbl_unit_price"),
            tax: t("lbl_tax"),
            amount: t("lbl_total"),
            subtotal: t("lbl_subtotal"),
            total: t("lbl_total"),
            paid: t("lbl_paid"),
            outstanding: t("lbl_outstanding"),
            reference: t("lbl_reference"),
            ticket: match l {
                Lang::Fr => "TICKET DE CAISSE".into(),
                Lang::Ar => "تذكرة بيع".into(),
                Lang::En => "SALES TICKET".into(),
            },
            cashier: t("lbl_cashier"),
            change: t("lbl_change"),
            thank_you: match l {
                Lang::Fr => "Merci de votre visite".into(),
                Lang::Ar => "شكراً لتعاملكم معنا".into(),
                Lang::En => "Thank you for your business".into(),
            },
        }
    }
}

pub struct PartyBlock {
    pub name: String,
    pub lines: Vec<String>,
}

fn draw_logo(doc: &mut PdfDoc, company: &Company, right: f64, top: f64, max_h: f64) {
    let Some((_, info)) = doc.logo.as_ref() else { return };
    let ratio = info.width as f64 / info.height.max(1) as f64;
    let mut h = max_h.min(56.0);
    let mut w = h * ratio;
    if w > 110.0 {
        w = 110.0;
        h = w / ratio;
    }
    let x = right - 40.0 - w;
    let y = PAGE_H - top - h;
    let page = doc.page();
    page.raw("q");
    page.raw(&format!("{w:.2} 0 0 {h:.2} {x:.2} {y:.2} cm"));
    page.raw("/Im0 Do");
    page.raw("Q");
    let _ = company;
}

fn draw_header(
    doc: &mut PdfDoc,
    company: &Company,
    tmpl: &InvoiceTemplate,
    labels: &PdfLabels,
    title: &str,
    number: &str,
    date: &str,
    due: &str,
    compact: bool,
) {
    let h = if compact { 62.0 } else { 92.0 };
    {
        let page = doc.page();
        page.rect(0.0, 0.0, PAGE_W, h, &tmpl.primary);
        page.text(40.0, if compact { 26.0 } else { 30.0 }, 17.0, true, "#ffffff", &company.name);
        let mut meta = Vec::new();
        if !company.address.trim().is_empty() {
            meta.push(company.address.clone());
        }
        if !company.phone.trim().is_empty() {
            meta.push(company.phone.clone());
        }
        if !company.email.trim().is_empty() {
            meta.push(company.email.clone());
        }
        if !company.tax_id.trim().is_empty() {
            meta.push(format!("{}: {}", labels.reference, company.tax_id));
        }
        if !compact {
            page.text(40.0, 50.0, 8.5, false, "#ffffff", &meta.join("  ·  "));
            if !tmpl.header.trim().is_empty() {
                page.text(40.0, 66.0, 8.5, false, "#ffffff", &tmpl.header);
            }
        }
    }
    draw_logo(doc, company, PAGE_W, 8.0, h - 20.0);

    let page = doc.page();
    page.text_right(PAGE_W - 40.0, 46.0, 20.0, true, "#ffffff", title);
    page.text_right(PAGE_W - 40.0, 66.0, 10.0, false, "#ffffff", number);
    if !compact {
        page.text_right(
            PAGE_W - 40.0,
            82.0,
            8.5,
            false,
            "#ffffff",
            &format!("{} {}  ·  {} {}", labels.date, date, labels.due_date, due),
        );
    }
}

fn draw_party(page: &mut Content, top: f64, labels: &PdfLabels, party: &PartyBlock, company: &Company) {
    page.text(40.0, top, 8.0, true, "#64748b", &labels.bill_to.to_uppercase());
    page.text(40.0, top + 16.0, 11.0, true, "#111827", &party.name);
    let mut y = top + 30.0;
    for line in party.lines.iter().take(4) {
        page.text(40.0, y, 8.5, false, "#4b5563", line);
        y += 11.0;
    }

    page.text(PAGE_W - 220.0, top, 8.0, true, "#64748b", &labels.from.to_uppercase());
    page.text(PAGE_W - 220.0, top + 16.0, 10.0, false, "#111827", &company.name);
    let mut y2 = top + 30.0;
    for line in [company.address.clone(), company.phone.clone(), company.email.clone()]
        .iter()
        .filter(|s| !s.trim().is_empty())
        .take(3)
    {
        page.text(PAGE_W - 220.0, y2, 8.5, false, "#4b5563", line);
        y2 += 11.0;
    }
}

fn table_header(page: &mut Content, top: f64, tmpl: &InvoiceTemplate, labels: &PdfLabels) {
    page.rect(40.0, top, PAGE_W - 80.0, 20.0, &tmpl.primary);
    page.text(46.0, top + 13.5, 8.5, true, "#ffffff", &labels.description);
    page.text_right(392.0, top + 13.5, 8.5, true, "#ffffff", &labels.qty);
    page.text_right(455.0, top + 13.5, 8.5, true, "#ffffff", &labels.unit_price);
    page.text_right(500.0, top + 13.5, 8.5, true, "#ffffff", &labels.tax);
    page.text_right(PAGE_W - 46.0, top + 13.5, 8.5, true, "#ffffff", &labels.amount);
}

fn table_row(page: &mut Content, top: f64, line: &crate::models::DocLine, zebra: bool, tmpl: &InvoiceTemplate) {
    if zebra {
        let (r, g, b) = rgb(&tmpl.primary);
        page.raw("q");
        page.raw(&format!("{r:.3} {g:.3} {b:.3} rg"));
        page.raw(&format!("0.06 w"));
        page.raw("Q");
        page.rect(40.0, top, PAGE_W - 80.0, 18.0, "#f1f5f9");
    }
    let desc = if line.description.chars().count() > 58 {
        let truncated: String = line.description.chars().take(55).collect();
        format!("{truncated}...")
    } else {
        line.description.clone()
    };
    page.text(46.0, top + 12.5, 8.7, false, "#111827", &desc);
    page.text_right(392.0, top + 12.5, 8.7, false, "#111827", &qty(line.qty));
    page.text_right(455.0, top + 12.5, 8.7, false, "#111827", &money(line.unit_price));
    page.text_right(
        500.0,
        top + 12.5,
        8.7,
        false,
        "#64748b",
        &format!("{}%", qty(line.tax_rate)),
    );
    page.text_right(PAGE_W - 46.0, top + 12.5, 8.7, false, "#111827", &money(line.net()));
}

fn draw_footer(page: &mut Content, tmpl: &InvoiceTemplate, page_no: usize, pages: usize) {
    let y = PAGE_H - 40.0;
    page.line(40.0, y - 10.0, PAGE_W - 40.0, y - 10.0, "#e2e8f0", 0.8);
    let footer = if tmpl.footer.trim().is_empty() {
        tmpl.terms.clone()
    } else {
        tmpl.footer.clone()
    };
    page.text(40.0, y + 2.0, 8.0, false, "#64748b", &footer);
    page.text_right(PAGE_W - 40.0, y + 2.0, 8.0, false, "#94a3b8", &format!("{page_no}/{pages}"));
}

/// Build the invoice PDF. `party` is the supplier (for bills) or customer.
pub fn invoice_pdf(
    inv: &Invoice,
    company: &Company,
    party: &PartyBlock,
    labels: &PdfLabels,
) -> Vec<u8> {
    let tmpl = company.template.clone();
    let logo = if tmpl.show_logo { decode_logo(company) } else { None };
    // The face is available to every document (an English invoice can still
    // carry an Arabic customer name); it is only embedded when used.
    let mut doc = PdfDoc::new(logo, PAGE_W, fonts::arabic(), labels.arabic);

    const PAGE_ROWS: usize = 22;
    let chunks: Vec<&[crate::models::DocLine]> = if inv.lines.is_empty() {
        vec![&[]]
    } else {
        inv.lines.chunks(PAGE_ROWS).collect()
    };
    let total_pages = chunks.len();
    let status = inv.display_status();

    for (idx, chunk) in chunks.iter().enumerate() {
        if idx > 0 {
            doc.new_page();
        }
        let last = idx + 1 == total_pages;
        draw_header(
            &mut doc,
            company,
            &tmpl,
            labels,
            &labels.invoice,
            &inv.number,
            &inv.invoice_date,
            &inv.due_date,
            idx > 0,
        );

        let mut top = if idx == 0 { 118.0 } else { 84.0 };
        if idx == 0 {
            draw_party(doc.page(), top, labels, party, company);
            top += 92.0;
        } else {
            doc.page().text(40.0, top, 9.0, false, "#64748b", &format!("{} · {}", labels.number, inv.number));
            top += 18.0;
        }

        table_header(doc.page(), top, &tmpl, labels);
        top += 20.0;
        for (i, line) in chunk.iter().enumerate() {
            table_row(doc.page(), top, line, i % 2 == 1, &tmpl);
            top += 18.0;
        }

        if last {
            top += 10.0;
            let totals = &inv.totals;
            let label_x = PAGE_W - 240.0;
            let value_right = PAGE_W - 46.0;
            doc.page().line(label_x, top, value_right, top, "#e2e8f0", 0.8);
            top += 14.0;
            doc.page().text(label_x, top, 9.0, false, "#4b5563", &labels.subtotal);
            doc.page().text_right(value_right, top, 9.0, false, "#111827", &money(totals.subtotal));
            top += 15.0;
            if tmpl.show_tax {
                doc.page().text(label_x, top, 9.0, false, "#4b5563", &labels.tax);
                doc.page().text_right(value_right, top, 9.0, false, "#111827", &money(totals.tax));
                top += 15.0;
            }
            doc.page().rect(label_x - 8.0, top - 2.0, value_right - label_x + 12.0, 24.0, &tmpl.primary);
            doc.page().text(label_x, top + 14.0, 10.5, true, "#ffffff", &labels.total);
            doc.page().text_right(value_right, top + 14.0, 11.5, true, "#ffffff", &money(totals.total));
            top += 34.0;

            if inv.paid_amount > 0.0 {
                doc.page().text(label_x, top, 9.0, false, "#4b5563", &labels.paid);
                doc.page().text_right(value_right, top, 9.0, false, "#111827", &money(inv.paid_amount));
                top += 15.0;
                doc.page().text(label_x, top, 9.0, true, "#4b5563", &labels.outstanding);
                doc.page().text_right(value_right, top, 9.0, true, "#b91c1c", &money(inv.outstanding()));
                top += 15.0;
            }

            doc.page().text(
                40.0,
                top + 16.0,
                8.5,
                false,
                "#64748b",
                &format!("{}: {}", labels.due_date, inv.due_date),
            );
            if !inv.notes.trim().is_empty() {
                doc.page().text(40.0, top + 30.0, 8.5, false, "#4b5563", &inv.notes);
            }
            doc.page().text(
                40.0,
                top + 44.0,
                8.0,
                false,
                "#94a3b8",
                &format!("{} · {}", labels.reference, status.as_str().to_uppercase()),
            );
        }

        draw_footer(doc.page(), &tmpl, idx + 1, total_pages);
    }

    doc.finish()
}

/// Narrow receipt PDF for the direct (POS) ticket.
pub fn ticket_pdf(ticket: &Ticket, company: &Company, labels: &PdfLabels) -> Vec<u8> {
    let tmpl = company.template.clone();
    let width = 226.77; // 80 mm
    let mut doc = PdfDoc::new(None, width, fonts::arabic(), labels.arabic);
    {
        let page = doc.page();
        page.rect(0.0, 0.0, width, 46.0, &tmpl.primary);
        page.text_center(width / 2.0, 24.0, 11.0, true, "#ffffff", &company.name);
        page.text_center(width / 2.0, 38.0, 7.5, false, "#ffffff", &labels.ticket);
    }
    let mut y = 66.0;
    {
        let page = doc.page();
        page.text(12.0, y, 7.5, false, "#4b5563", &format!("{}: {}", labels.number, ticket.number));
        page.text_right(width - 12.0, y, 7.5, false, "#4b5563", &ticket.date);
        y += 14.0;
        page.line(12.0, y, width - 12.0, y, "#cbd5e1", 0.7);
        y += 14.0;
    }
    for item in &ticket.items {
        if y > 640.0 {
            doc.new_page();
            y = 40.0;
        }
        let page = doc.page();
        page.text(12.0, y, 8.0, false, "#111827", &item.name);
        page.text_right(width - 12.0, y, 8.0, false, "#111827", &money(item.line_total()));
        y += 11.0;
        page.text(12.0, y, 7.0, false, "#64748b", &format!("{} x {}", qty(item.qty), money(item.price)));
        y += 13.0;
    }
    {
        let page = doc.page();
        page.line(12.0, y, width - 12.0, y, "#cbd5e1", 0.7);
        y += 14.0;
        page.text(12.0, y, 8.0, false, "#4b5563", &labels.subtotal);
        page.text_right(width - 12.0, y, 8.0, false, "#111827", &money(ticket.subtotal));
        y += 12.0;
        page.text(12.0, y, 8.0, false, "#4b5563", &labels.tax);
        page.text_right(width - 12.0, y, 8.0, false, "#111827", &money(ticket.tax));
        y += 16.0;
        page.text(12.0, y, 10.0, true, "#111827", &labels.total);
        page.text_right(width - 12.0, y, 10.0, true, "#111827", &money(ticket.total));
        y += 14.0;
        page.text(12.0, y, 8.0, false, "#4b5563", &labels.paid);
        page.text_right(width - 12.0, y, 8.0, false, "#111827", &money(ticket.paid));
        y += 12.0;
        page.text(12.0, y, 8.0, false, "#4b5563", &labels.change);
        page.text_right(width - 12.0, y, 8.0, false, "#111827", &money(ticket.change));
        y += 18.0;
        page.text(12.0, y, 7.0, false, "#64748b", &format!("{}: {}", labels.cashier, ticket.cashier));
        y += 14.0;
        page.text_center(width / 2.0, y, 7.5, false, "#94a3b8", &labels.thank_you);
    }
    doc.finish()
}
