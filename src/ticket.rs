//! ESC/POS rendering for mini thermal printers (USB/serial or Bluetooth).
//!
//! The byte stream is what the printer expects; thermal printers only carry a
//! handful of code pages, so text is transliterated to ASCII here (Arabic
//! product names must be printed from the on-screen/PDF document instead).

use crate::models::{Company, Ticket};
use crate::util::{money, qty};

const WIDTH: usize = 42;

/// ESC/POS command bytes.
mod cmd {
    pub const INIT: &[u8] = &[0x1b, 0x40];
    pub const ALIGN_LEFT: &[u8] = &[0x1b, 0x61, 0x00];
    pub const ALIGN_CENTER: &[u8] = &[0x1b, 0x61, 0x01];
    pub const BOLD_ON: &[u8] = &[0x1b, 0x45, 0x01];
    pub const BOLD_OFF: &[u8] = &[0x1b, 0x45, 0x00];
    pub const SIZE_DOUBLE: &[u8] = &[0x1d, 0x21, 0x11];
    pub const SIZE_NORMAL: &[u8] = &[0x1d, 0x21, 0x00];
    pub const FEED: &[u8] = &[0x0a];
    pub const CUT: &[u8] = &[0x1d, 0x56, 0x42, 0x00];
    pub const DRAWER: &[u8] = &[0x1b, 0x70, 0x00, 0x19, 0xfa];
}

fn ascii(text: &str) -> String {
    let map = |c: char| match c {
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'à' | 'â' | 'ä' => 'a',
        'î' | 'ï' => 'i',
        'ô' | 'ö' => 'o',
        'û' | 'ü' | 'ù' => 'u',
        'ç' => 'c',
        'É' | 'È' | 'Ê' => 'E',
        'À' | 'Â' => 'A',
        '·' | '•' => '-',
        c if c.is_ascii() => c,
        _ => '?',
    };
    text.chars().map(map).collect()
}

struct Out {
    buf: Vec<u8>,
}

impl Out {
    fn new() -> Self {
        Self {
            buf: Vec::with_capacity(512),
        }
    }

    fn cmd(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    fn text(&mut self, s: &str) {
        self.buf.extend_from_slice(ascii(s).as_bytes());
    }

    fn line(&mut self, s: &str) {
        self.text(s);
        self.buf.push(b'\n');
    }

    fn center(&mut self, s: &str) {
        self.cmd(cmd::ALIGN_CENTER);
        self.line(s);
        self.cmd(cmd::ALIGN_LEFT);
    }

    fn divider(&mut self) {
        self.line(&"-".repeat(WIDTH));
    }

    /// Left label + right value padded to the paper width.
    fn pair(&mut self, left: &str, right: &str) {
        let l = ascii(left);
        let r = ascii(right);
        let pad = WIDTH.saturating_sub(l.chars().count() + r.chars().count());
        let mut s = l;
        if pad > 0 {
            s.push_str(&" ".repeat(pad));
        } else {
            s.truncate(WIDTH.saturating_sub(r.chars().count()));
        }
        s.push_str(&r);
        self.line(&s);
    }
}

/// Raw ESC/POS bytes for a direct-purchase ticket.
pub fn escpos_ticket(ticket: &Ticket, company: &Company, labels: &crate::pdf::PdfLabels) -> Vec<u8> {
    let mut o = Out::new();
    o.cmd(cmd::INIT);
    o.cmd(cmd::ALIGN_CENTER);
    o.cmd(cmd::SIZE_DOUBLE);
    o.cmd(cmd::BOLD_ON);
    o.line(&company.name);
    o.cmd(cmd::BOLD_OFF);
    o.cmd(cmd::SIZE_NORMAL);
    if !company.address.is_empty() {
        o.line(&company.address);
    }
    if !company.phone.is_empty() {
        o.line(&company.phone);
    }
    if !company.tax_id.is_empty() {
        o.line(&format!("Tax ID: {}", company.tax_id));
    }
    o.line(&labels.ticket);
    o.cmd(cmd::ALIGN_LEFT);
    o.divider();
    o.pair(&format!("{}:", labels.number), &ticket.number);
    o.pair(&format!("{}:", labels.date), &ticket.date);
    o.pair(&format!("{}:", labels.cashier), &ticket.cashier);
    o.divider();

    for item in &ticket.items {
        o.line(&item.name);
        o.pair(
            &format!("  {} x {}", qty(item.qty), money(item.price)),
            &money(item.line_total()),
        );
    }

    o.divider();
    o.pair(&format!("{}:", labels.subtotal), &money(ticket.subtotal));
    o.pair(&format!("{}:", labels.tax), &money(ticket.tax));
    o.cmd(cmd::BOLD_ON);
    o.pair(&format!("{}:", labels.total), &money(ticket.total));
    o.cmd(cmd::BOLD_OFF);
    o.pair(&format!("{}:", labels.paid), &money(ticket.paid));
    o.pair(&format!("{}:", labels.change), &money(ticket.change));
    o.divider();
    o.center(&labels.thank_you);
    o.cmd(cmd::FEED);
    o.cmd(cmd::FEED);
    o.cmd(cmd::FEED);
    o.cmd(cmd::DRAWER);
    o.cmd(cmd::CUT);
    o.buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TicketItem;

    fn sample() -> Ticket {
        Ticket {
            id: "1".into(),
            company_id: "c".into(),
            number: "TK-1".into(),
            date: "2026-01-01".into(),
            items: vec![TicketItem {
                product_id: "p".into(),
                name: "Café".into(),
                qty: 2.0,
                price: 1.5,
            }],
            subtotal: 3.0,
            tax: 0.0,
            total: 3.0,
            paid: 5.0,
            change: 2.0,
            cashier: "admin".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            created_by: "admin".into(),
        }
    }

    #[test]
    fn ticket_starts_with_init_and_cuts() {
        let company = Company {
            id: "c".into(),
            name: "Test".into(),
            legal_name: String::new(),
            tax_id: String::new(),
            currency: "USD".into(),
            address: String::new(),
            phone: String::new(),
            email: String::new(),
            logo: String::new(),
            template: Default::default(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        let labels = crate::pdf::PdfLabels::new(crate::i18n::Lang::En);
        let bytes = escpos_ticket(&sample(), &company, &labels);
        assert_eq!(&bytes[0..2], &[0x1b, 0x40]);
        assert!(bytes.ends_with(&[0x1d, 0x56, 0x42, 0x00]));
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("TK-1"));
        assert!(text.contains("Cafe"));
    }
}
