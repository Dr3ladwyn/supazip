//! `supazip list` text table, driven by `design/cli-table.json`.
//!
//! The JSON file is a machine-readable mirror of the column contract in
//! `design/cli-table.tera` (widths, alignment, rule character, CRYPT
//! literals, footer). This crate does not depend on Tera.

use std::fmt::Write as _;
use std::io::IsTerminal;
use std::sync::LazyLock;

use anstyle::{Color, RgbColor, Style};
use serde::Deserialize;
use supazip_core::ArchiveEntry;

const TABLE_SPEC_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../design/cli-table.json"
));

const TOKENS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../design/tokens.json"
));

static SPEC: LazyLock<TableSpec> = LazyLock::new(|| {
    serde_json::from_str(TABLE_SPEC_JSON)
        .expect("design/cli-table.json must deserialize as TableSpec")
});

/// Column layout loaded from `design/cli-table.json`.
#[derive(Debug, Clone, Deserialize)]
struct TableSpec {
    total_width: usize,
    column_gap: String,
    rule_char: String,
    ellipsis: String,
    crypt_yes: String,
    crypt_no: String,
    footer_label: String,
    columns: Vec<ColumnSpec>,
}

#[derive(Debug, Clone, Deserialize)]
struct ColumnSpec {
    id: String,
    header: String,
    #[serde(default)]
    width: Option<usize>,
    align: Align,
    #[serde(default)]
    truncate: bool,
    #[serde(default)]
    format: CellFormat,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Align {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum CellFormat {
    #[default]
    Plain,
    HumanSize,
}

/// Dark-theme styles parsed from `design/tokens.json` (`themes.dark`).
#[derive(Debug, Clone)]
pub struct CliStyles {
    header: Style,
    primary: Style,
    muted: Style,
    warning: Style,
    rule: Style,
}

#[derive(Deserialize)]
struct TokensFile {
    themes: Themes,
}

#[derive(Deserialize)]
struct Themes {
    dark: DarkTheme,
}

#[derive(Deserialize)]
struct DarkTheme {
    color: DarkColor,
}

#[derive(Deserialize)]
struct DarkColor {
    fg: FgColors,
    semantic: SemanticColors,
    border: BorderColors,
}

#[derive(Deserialize)]
struct FgColors {
    primary: String,
    secondary: String,
    muted: String,
}

#[derive(Deserialize)]
struct SemanticColors {
    warning: String,
}

#[derive(Deserialize)]
struct BorderColors {
    strong: String,
}

impl CliStyles {
    /// Map `themes.dark` token hex values to `anstyle` RGB foregrounds.
    pub fn from_dark_tokens() -> Self {
        let tokens: TokensFile = serde_json::from_str(TOKENS_JSON)
            .expect("design/tokens.json must contain themes.dark colors");
        let color = tokens.themes.dark.color;
        Self {
            header: fg_hex(&color.fg.secondary),
            primary: fg_hex(&color.fg.primary),
            muted: fg_hex(&color.fg.muted),
            warning: fg_hex(&color.semantic.warning),
            rule: fg_hex(&color.border.strong),
        }
    }
}

fn fg_hex(hex: &str) -> Style {
    Style::new().fg_color(Some(Color::Rgb(parse_rgb(hex))))
}

fn parse_rgb(hex: &str) -> RgbColor {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 {
        panic!("expected #RRGGBB token, got {hex:?}");
    }
    let r = u8::from_str_radix(&hex[0..2], 16).expect("token R");
    let g = u8::from_str_radix(&hex[2..4], 16).expect("token G");
    let b = u8::from_str_radix(&hex[4..6], 16).expect("token B");
    RgbColor(r, g, b)
}

/// Colour the text table only when stdout is a TTY that supports colour.
///
/// Piped stdout (tests, `|`, redirects) stays plain. `NO_COLOR` disables
/// colour. `--output json|yaml` never calls this path.
pub fn tty_styles() -> Option<CliStyles> {
    if !should_color_stdout() {
        return None;
    }
    #[cfg(windows)]
    {
        let _ = anstyle_query::windows::enable_ansi_colors();
    }
    Some(CliStyles::from_dark_tokens())
}

fn should_color_stdout() -> bool {
    if anstyle_query::no_color() {
        return false;
    }
    std::io::stdout().is_terminal() && anstyle_query::term_supports_color()
}

/// Render the list table. `styles` is `None` for plain text (piped / tests).
pub fn render_list_table(entries: &[ArchiveEntry], styles: Option<&CliStyles>) -> String {
    let spec = &*SPEC;
    let mut out = String::new();

    let header = join_columns(
        spec,
        spec.columns.iter().map(|col| {
            let cell = format_cell(&col.header, col, spec);
            paint(&cell, styles.map(|s| s.header))
        }),
    );
    let _ = writeln!(out, "{header}");

    let rule = rule_line(spec);
    let _ = writeln!(out, "{}", paint(&rule, styles.map(|s| s.rule)));

    for (idx, entry) in entries.iter().enumerate() {
        let row = join_columns(
            spec,
            spec.columns.iter().map(|col| {
                let raw = raw_value(spec, col, idx, entry);
                let cell = format_cell(&raw, col, spec);
                paint(&cell, styles.map(|s| style_for_cell(s, col, &raw)))
            }),
        );
        let _ = writeln!(out, "{row}");
    }

    let footer = format!("{} {}", entries.len(), spec.footer_label);
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", paint(&footer, styles.map(|s| s.header)));
    out
}

fn style_for_cell(styles: &CliStyles, col: &ColumnSpec, raw: &str) -> Style {
    if col.id == "crypt" {
        if raw == SPEC.crypt_yes {
            styles.warning
        } else {
            styles.muted
        }
    } else {
        styles.primary
    }
}

fn raw_value(spec: &TableSpec, col: &ColumnSpec, idx: usize, entry: &ArchiveEntry) -> String {
    match col.id.as_str() {
        "idx" => idx.to_string(),
        "method" => entry.compression_method.clone(),
        "size" => format_by_kind(col.format, entry.size),
        "compressed" => format_by_kind(col.format, entry.compressed_size),
        "crypt" => {
            if entry.encrypted {
                spec.crypt_yes.clone()
            } else {
                spec.crypt_no.clone()
            }
        }
        "name" => entry.name.clone(),
        _ => String::new(),
    }
}

fn format_by_kind(format: CellFormat, bytes: u64) -> String {
    match format {
        CellFormat::Plain => bytes.to_string(),
        CellFormat::HumanSize => format_size(bytes),
    }
}

/// Humanised byte count: integer below 1 KiB, one decimal from KiB up.
fn format_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if (bytes as f64) < MIB {
        format!("{:.1} KiB", bytes as f64 / KIB)
    } else if (bytes as f64) < GIB {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else {
        format!("{:.1} GiB", bytes as f64 / GIB)
    }
}

fn format_cell(raw: &str, col: &ColumnSpec, spec: &TableSpec) -> String {
    let Some(width) = col.width else {
        return raw.to_string();
    };
    let text = if col.truncate && raw.chars().count() > width {
        truncate_with_ellipsis(raw, width, &spec.ellipsis)
    } else {
        raw.to_string()
    };
    pad_align(&text, width, col.align)
}

fn truncate_with_ellipsis(text: &str, width: usize, ellipsis: &str) -> String {
    let ellipsis_len = ellipsis.chars().count();
    let keep = width.saturating_sub(ellipsis_len);
    let mut out: String = text.chars().take(keep).collect();
    out.push_str(ellipsis);
    out
}

fn pad_align(text: &str, width: usize, align: Align) -> String {
    let len = text.chars().count();
    if len >= width {
        return text.to_string();
    }
    let pad = width - len;
    match align {
        Align::Left => format!("{text}{}", " ".repeat(pad)),
        Align::Right => format!("{}{text}", " ".repeat(pad)),
    }
}

fn join_columns(spec: &TableSpec, cells: impl IntoIterator<Item = String>) -> String {
    let mut iter = cells.into_iter();
    let Some(first) = iter.next() else {
        return String::new();
    };
    let mut line = first;
    for cell in iter {
        line.push_str(&spec.column_gap);
        line.push_str(&cell);
    }
    line
}

fn rule_line(spec: &TableSpec) -> String {
    let ch = spec.rule_char.chars().next().unwrap_or('─');
    ch.to_string().repeat(spec.total_width)
}

fn paint(text: &str, style: Option<Style>) -> String {
    match style {
        Some(style) => format!("{style}{text}{style:#}"),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        name: &str,
        method: &str,
        size: u64,
        compressed: u64,
        encrypted: bool,
    ) -> ArchiveEntry {
        ArchiveEntry {
            name: name.to_string(),
            path: name.to_string(),
            is_dir: false,
            size,
            compressed_size: compressed,
            modified: None,
            compression_method: method.to_string(),
            crc32: None,
            encrypted,
        }
    }

    #[test]
    fn spec_should_parse_and_match_tera_comment_contract() {
        let spec = &*SPEC;
        assert_eq!(spec.total_width, 72);
        assert_eq!(spec.column_gap, "  ");
        assert_eq!(spec.rule_char, "─");
        assert_eq!(spec.ellipsis, "…");
        assert_eq!(spec.crypt_yes, "yes");
        assert_eq!(spec.crypt_no, "-");
        assert_eq!(spec.footer_label, "entries");

        let ids: Vec<&str> = spec.columns.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            ["idx", "method", "size", "compressed", "crypt", "name"]
        );
        assert_eq!(spec.columns[0].width, Some(4));
        assert_eq!(spec.columns[0].align, Align::Right);
        assert_eq!(spec.columns[1].width, Some(12));
        assert_eq!(spec.columns[1].align, Align::Left);
        assert!(spec.columns[1].truncate);
        assert_eq!(spec.columns[2].format, CellFormat::HumanSize);
        assert_eq!(spec.columns[3].format, CellFormat::HumanSize);
        assert_eq!(spec.columns[4].width, Some(8));
        assert_eq!(spec.columns[5].width, None);

        let tera = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../design/cli-table.tera"
        ));
        assert!(
            tera.contains("IDX        : width  4, right-aligned"),
            "tera contract must still document IDX width 4"
        );
        assert!(tera.contains("METHOD     : width 12, left-aligned"));
        assert!(tera.contains("SIZE       : width 12, right-aligned"));
        assert!(tera.contains("COMPRESSED : width 12, right-aligned"));
        assert!(tera.contains("CRYPT      : width  8, left-aligned"));
        assert!(tera.contains("NAME       : unbounded"));
        assert!(tera.contains("U+2500"));
        assert!(tera.contains("repeated 72 times"));
    }

    #[test]
    fn render_should_contain_header_columns_and_plain_footer() {
        let rows = [
            entry("hello.txt", "deflate", 15, 19, false),
            entry("data.bin", "store", 4, 4, true),
        ];
        let text = render_list_table(&rows, None);
        for col in ["IDX", "METHOD", "SIZE", "COMPRESSED", "CRYPT", "NAME"] {
            assert!(
                text.contains(col),
                "missing header column {col:?} in:\n{text}"
            );
        }
        assert!(
            text.contains(&"─".repeat(72)),
            "missing 72-wide U+2500 rule in:\n{text}"
        );
        assert!(text.contains("hello.txt"));
        assert!(text.contains("data.bin"));
        assert!(text.contains("2 entries"));
        assert!(
            text.contains("yes"),
            "encrypted row must use CRYPT literal yes"
        );
        assert!(
            !text.contains("true") && !text.contains("false"),
            "CRYPT must not leak boolean literals:\n{text}"
        );
        assert!(
            !text.contains('\u{1b}'),
            "plain render must not emit ANSI:\n{text:?}"
        );
    }

    #[test]
    fn format_size_should_use_kib_steps() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(4), "4 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 KiB");
        assert_eq!(format_size(1536), "1.5 KiB");
        assert_eq!(format_size(1024 * 1024), "1.0 MiB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GiB");
    }

    #[test]
    fn method_should_truncate_with_ellipsis_when_longer_than_width() {
        let rows = [entry("x", "LZMA2:48kfast", 1, 1, false)];
        let text = render_list_table(&rows, None);
        // Width 12 → 11 visible chars + U+2026 so the cell stays 12 columns.
        // (The tera worked-example string is one character longer than the
        // width contract; the width wins.)
        assert!(
            text.contains("LZMA2:48kfa…"),
            "expected 11 chars + ellipsis, got:\n{text}"
        );
        assert!(!text.contains("LZMA2:48kfast"));
    }

    #[test]
    fn colored_render_should_use_dark_token_rgb() {
        let styles = CliStyles::from_dark_tokens();
        let text = render_list_table(&[entry("a.txt", "store", 1, 1, false)], Some(&styles));
        // themes.dark.color.fg.secondary = #9DA7B3
        assert!(
            text.contains("\u{1b}[38;2;157;167;179m"),
            "header should use dark fg.secondary RGB, got:\n{text:?}"
        );
        // themes.dark.color.fg.primary = #E6EDF3
        assert!(
            text.contains("\u{1b}[38;2;230;237;243m"),
            "body should use dark fg.primary RGB, got:\n{text:?}"
        );
    }
}
