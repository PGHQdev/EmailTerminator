//! HTML to text for reading receipts. Receipts are tables, so a cell ends
//! with a tab and a row with a newline: "Total" and "$12.00" stay apart and
//! on one line.

use std::sync::LazyLock;

use regex::Regex;

static DROP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<(head|style|script|title)\b.*?</(head|style|script|title)\s*>|<!--.*?-->")
        .expect("drop pattern")
});
static CELL_END: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)</t[dh]\s*>").expect("cell pattern"));
static LINE_END: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<br\s*/?>|</(p|div|tr|li|h[1-6]|table|ul|ol|blockquote)\s*>")
        .expect("line pattern")
});
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").expect("tag pattern"));
static ENTITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"&(#[0-9]+|#[xX][0-9a-fA-F]+|[a-zA-Z]+);").expect("entity pattern")
});
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \r\n]+").expect("space pattern"));

pub(super) fn to_text(html: &str) -> String {
    let html = DROP.replace_all(html, "");
    // Source line breaks are layout, not text; collapse them before tags add
    // the meaningful ones.
    let html = SPACES.replace_all(&html, " ");
    let html = CELL_END.replace_all(&html, "\t");
    let html = LINE_END.replace_all(&html, "\n");
    let text = TAG.replace_all(&html, "");
    let text = ENTITY.replace_all(&text, |caps: &regex::Captures<'_>| entity(&caps[1]));
    text.lines()
        .map(|line| line.trim_matches([' ', '\t']))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn entity(name: &str) -> String {
    let code = if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
        u32::from_str_radix(hex, 16).ok()
    } else if let Some(dec) = name.strip_prefix('#') {
        dec.parse().ok()
    } else {
        match name {
            "amp" => Some(38),
            "lt" => Some(60),
            "gt" => Some(62),
            "quot" => Some(34),
            "apos" => Some(39),
            "nbsp" => Some(32),
            "euro" => Some(0x20ac),
            "pound" => Some(0xa3),
            "yen" => Some(0xa5),
            "copy" => Some(0xa9),
            "ndash" => Some(0x2013),
            "mdash" => Some(0x2014),
            _ => None,
        }
    };
    code.and_then(char::from_u32)
        .map(String::from)
        .unwrap_or_else(|| format!("&{name};"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_cells_stay_apart_on_one_line() {
        let html = "<html><head><title>Receipt</title><style>td{}</style></head><body>\n\
            <table><tr><td>App</td><td>Skyline Weather</td></tr>\n\
            <tr><td>Total</td><td>&yen;1,200</td></tr></table><p>Thanks&nbsp;&amp; bye</p></body></html>";
        assert_eq!(
            to_text(html),
            "App\tSkyline Weather\nTotal\t¥1,200\nThanks & bye"
        );
    }
}
