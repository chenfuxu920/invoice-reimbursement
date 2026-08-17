use rust_xlsxwriter::{Worksheet, XlsxError};
use std::collections::HashMap;

/// Approximate the display width of a string in pixels, using the same
/// character-width table that Excel/rust_xlsxwriter use for column autofit.
pub(crate) fn pixel_width(text: &str) -> u16 {
    let mut length = 0u32;

    for ch in text.chars() {
        length += match ch {
            ' ' | '\'' => 3,
            ',' | '.' | ':' | ';' | 'I' | '`' | 'i' | 'j' | 'l' => 4,
            '!' | '(' | ')' | '-' | 'J' | '[' | ']' | 'f' | 'r' | 't' | '{' | '}' => 5,
            '"' | '/' | 'L' | '\\' | 'c' | 's' | 'z' => 6,
            '#' | '$' | '*' | '+' | '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9'
            | '<' | '=' | '>' | '?' | 'E' | 'F' | 'S' | 'T' | 'Y' | 'Z' | '^' | '_' | 'a' | 'g'
            | 'k' | 'v' | 'x' | 'y' | '|' | '~' => 7,
            'B' | 'C' | 'K' | 'P' | 'R' | 'X' | 'b' | 'd' | 'e' | 'h' | 'n' | 'o' | 'p' | 'q'
            | 'u' => 8,
            'A' | 'D' | 'G' | 'H' | 'U' | 'V' => 9,
            '&' | 'N' | 'O' | 'Q' => 10,
            '%' | 'w' => 11,
            'M' | 'm' => 12,
            '@' | 'W' => 13,
            _ if ch.is_ascii() => 8,
            // CJK / full-width characters are double the ASCII digit width.
            // Excel's Calibri-based metrics use 7px per digit, so CJK is ~14px.
            _ => 14,
        };
    }

    length.min(1790) as u16
}

/// Convert a pixel width to an Excel column width (characters), matching
/// rust_xlsxwriter's built-in autofit conversion.
pub(crate) fn pixels_to_width(pixels: u16) -> f64 {
    let max_digit_width = 7.0_f64;
    let padding = 5.0_f64;
    let mut width = f64::from(pixels);

    if width < 12.0 {
        width /= max_digit_width + padding;
    } else {
        width = (width - padding) / max_digit_width;
    }

    width.max(0.0)
}

/// Convert an Excel column width (characters) back to approximate pixels.
fn width_to_pixels(width: f64) -> f64 {
    let max_digit_width = 7.0_f64;
    let padding = 5.0_f64;

    if width < 12.0 {
        width * (max_digit_width + padding)
    } else {
        width * max_digit_width + padding
    }
}

/// Auto-fit column widths and row heights based on the text written to the
/// sheet.
///
/// `cell_texts` should contain every non-merged text cell that participates in
/// autofit: `(row, col, text)`. `base_col_widths` should contain the explicit
/// column widths already set on the worksheet; these are used as the starting
/// point for merged-cell width calculations and for columns without text.
///
/// Row heights are estimated from the wrapped line count and written to the
/// sheet. This mimics the WPS/Excel "最适合行高" behaviour closely enough for
/// exported forms while still producing a self-contained file.
pub(crate) fn autofit_sheet(
    ws: &mut Worksheet,
    cell_texts: &[(u32, u16, String)],
    base_col_widths: &[(u16, f64)],
    max_width_pixels: u16,
) -> Result<(), XlsxError> {
    autofit_sheet_full(ws, cell_texts, &[], base_col_widths, max_width_pixels, &[])
}

/// Like [`autofit_sheet`], but also understands horizontally merged cells and
/// never shrinks listed rows below the given minimum heights.
///
/// `merged_texts` is `(row, first_col, last_col, text)` for each merged cell
/// that contains text. Merged text is excluded from single-column width
/// calculations (otherwise one merged title would stretch its first column),
/// but is included in row-height wrapping calculations using the total width of
/// the merged range.
pub(crate) fn autofit_sheet_with_min(
    ws: &mut Worksheet,
    cell_texts: &[(u32, u16, String)],
    merged_texts: &[(u32, u16, u16, String)],
    base_col_widths: &[(u16, f64)],
    max_width_pixels: u16,
    min_row_heights: &[(u32, f64)],
) -> Result<(), XlsxError> {
    autofit_sheet_full(
        ws,
        cell_texts,
        merged_texts,
        base_col_widths,
        max_width_pixels,
        min_row_heights,
    )
}

fn autofit_sheet_full(
    ws: &mut Worksheet,
    cell_texts: &[(u32, u16, String)],
    merged_texts: &[(u32, u16, u16, String)],
    base_col_widths: &[(u16, f64)],
    max_width_pixels: u16,
    min_row_heights: &[(u32, f64)],
) -> Result<(), XlsxError> {
    // 1. Find the widest non-merged text per column. Multi-line strings are
    //    measured by their widest single line so explicit newlines don't
    //    inflate column width.
    let mut col_max_pixels: HashMap<u16, u16> = HashMap::new();
    for (_, col, text) in cell_texts {
        let width = text
            .lines()
            .map(pixel_width)
            .max()
            .unwrap_or(0)
            .min(max_width_pixels);
        let entry = col_max_pixels.entry(*col).or_insert(0);
        if width > *entry {
            *entry = width;
        }
    }

    // 2. Build the effective column-width map and set autofitted widths.
    let mut col_widths: HashMap<u16, f64> = base_col_widths
        .iter()
        .map(|(col, width)| (*col, *width))
        .collect();

    for (col, pixels) in &col_max_pixels {
        let capped = (*pixels + 7).min(max_width_pixels);
        let calculated = pixels_to_width(capped);
        // Only widen columns; never shrink an explicitly set width. This keeps
        // hand-tuned form columns wide enough for numeric/date cells that are
        // not tracked in `cell_texts` (e.g. J 标准, E 日期).
        let base = base_col_widths
            .iter()
            .find(|(c, _)| *c == *col)
            .map(|(_, width)| *width)
            .unwrap_or(0.0);
        let width = calculated.max(base);
        ws.set_column_width(*col, width)?;
        col_widths.insert(*col, width);
    }

    // 3. Estimate the number of wrapped lines per data row.
    let mut row_max_lines: HashMap<u32, u32> = HashMap::new();

    for (row, col, text) in cell_texts {
        let col_width = col_widths.get(col).copied().unwrap_or(8.43);
        let col_pixels = width_to_pixels(col_width);
        let usable_pixels = (col_pixels - 7.0).max(1.0);
        let lines = wrapped_line_count(text, usable_pixels);
        update_max_lines(*row, lines, &mut row_max_lines);
    }

    for (row, first_col, last_col, text) in merged_texts {
        let total_pixels: f64 = (*first_col..=*last_col)
            .map(|col| {
                let col_width = col_widths.get(&col).copied().unwrap_or(8.43);
                width_to_pixels(col_width)
            })
            .sum();
        let usable_pixels = (total_pixels - 7.0).max(1.0);
        let lines = wrapped_line_count(text, usable_pixels);
        update_max_lines(*row, lines, &mut row_max_lines);
    }

    // 4. Set row heights. A single line uses Excel's default 15pt height;
    //    each additional wrapped line adds another 15pt.
    for (row, lines) in &row_max_lines {
        let mut height = f64::from(*lines) * 15.0;

        if let Some((_, min_height)) = min_row_heights.iter().find(|(r, _)| r == row) {
            height = height.max(*min_height);
        }

        ws.set_row_height(*row, height)?;
    }

    // Ensure rows that only have a minimum height (e.g. fixed-layout rows with
    // no tracked text) still get that height.
    for (row, min_height) in min_row_heights {
        if !row_max_lines.contains_key(row) {
            ws.set_row_height(*row, *min_height)?;
        }
    }

    Ok(())
}

/// Count how many display lines a string occupies inside `usable_pixels` of
/// horizontal space. Explicit newlines always start a new line.
fn wrapped_line_count(text: &str, usable_pixels: f64) -> u32 {
    let mut lines = 0u32;
    for segment in text.lines() {
        let text_pixels = f64::from(pixel_width(segment));
        lines += (text_pixels / usable_pixels).ceil().max(1.0) as u32;
    }
    lines.max(1)
}

fn update_max_lines(row: u32, lines: u32, row_max_lines: &mut HashMap<u32, u32>) {
    let entry = row_max_lines.entry(row).or_insert(1);
    if lines > *entry {
        *entry = lines;
    }
}
