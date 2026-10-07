use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

pub const ACCENT_A: (u8, u8, u8) = (0x7e, 0xe7, 0x87);
pub const ACCENT_B: (u8, u8, u8) = (0xff, 0x7b, 0x72);

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub truecolor: bool,
    pub ascii: bool,
    pub reduced_motion: bool,
}

impl Theme {
    pub fn detect(ascii: bool) -> Self {
        let truecolor = std::env::var("COLORTERM")
            .map(|v| v.contains("truecolor") || v.contains("24bit"))
            .unwrap_or(false);
        Self {
            truecolor,
            ascii,
            reduced_motion: false,
        }
    }

    fn rgb_or(&self, rgb: (u8, u8, u8), indexed: u8) -> Color {
        if self.truecolor {
            Color::Rgb(rgb.0, rgb.1, rgb.2)
        } else {
            Color::Indexed(indexed)
        }
    }

    /// Convert application chrome, preserving arbitrary user text at its own seam.
    pub fn chrome(&self, text: impl AsRef<str>) -> String {
        if !self.ascii {
            return text.as_ref().to_string();
        }
        text.as_ref()
            .chars()
            .map(|c| match c {
                '·' => '.',
                '─' | '—' => '-',
                '▏' => '_',
                '▎' | '│' => '|',
                '⚠' => '!',
                '✓' => '+',
                '…' => '~',
                '→' => '>',
                other => other,
            })
            .collect()
    }

    pub fn cursor(&self) -> &'static str {
        if self.ascii { "_" } else { "▏" }
    }

    pub fn borders(&self) -> ratatui::symbols::border::Set<'static> {
        if self.ascii {
            ratatui::symbols::border::Set {
                top_left: "+",
                top_right: "+",
                bottom_left: "+",
                bottom_right: "+",
                vertical_left: "|",
                vertical_right: "|",
                horizontal_top: "-",
                horizontal_bottom: "-",
            }
        } else {
            ratatui::symbols::border::ROUNDED
        }
    }

    pub fn bg(&self) -> Color {
        self.rgb_or((0x0f, 0x11, 0x17), 233)
    }

    pub fn panel(&self) -> Color {
        self.rgb_or((0x14, 0x17, 0x20), 234)
    }

    pub fn bar(&self) -> Color {
        self.rgb_or((0x11, 0x14, 0x1c), 233)
    }

    pub fn highlight(&self) -> Color {
        self.rgb_or((0x24, 0x2b, 0x3a), 237)
    }

    pub fn dim(&self) -> Color {
        self.rgb_or((0x8b, 0x94, 0x9e), 248)
    }

    pub fn text(&self) -> Color {
        self.rgb_or((0xc9, 0xd1, 0xd9), 252)
    }

    pub fn accent(&self) -> Color {
        self.rgb_or(ACCENT_A, 114)
    }

    pub fn red(&self) -> Color {
        self.rgb_or(ACCENT_B, 210)
    }

    pub fn yellow(&self) -> Color {
        self.rgb_or((0xe3, 0xb3, 0x41), 179)
    }

    pub fn dot_running(&self) -> &'static str {
        if self.ascii { "* " } else { "● " }
    }

    pub fn dot_stopped(&self) -> &'static str {
        if self.ascii { "o " } else { "○ " }
    }

    pub fn spinner(&self, frame: usize) -> &'static str {
        if self.reduced_motion {
            return if self.ascii { "." } else { "…" };
        }
        const BRAILLE: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
        const ASCII: [&str; 4] = ["|", "/", "-", "\\"];
        if self.ascii {
            ASCII[frame % 4]
        } else {
            BRAILLE[frame % 8]
        }
    }

    pub fn lerp(&self, a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> Color {
        if !self.truecolor {
            return self.accent();
        }
        let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
        Color::Rgb(f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
    }

    pub fn gradient_spans(&self, text: &str, bold: bool) -> Vec<Span<'static>> {
        let n = text.chars().count().max(1);
        text.chars()
            .enumerate()
            .map(|(i, c)| {
                let mut style = Style::new().fg(self.lerp(
                    ACCENT_A,
                    ACCENT_B,
                    i as f32 / (n - 1).max(1) as f32,
                ));
                if bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                Span::styled(c.to_string(), style)
            })
            .collect()
    }
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut unit = 0;
    while v >= 1000.0 && unit < UNITS.len() - 1 {
        v /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Color) -> f64 {
        let rgb = match color {
            Color::Rgb(r, g, b) => [r, g, b],
            Color::Indexed(v @ 232..=255) => [8 + (v - 232) * 10; 3],
            other => panic!("unexpected palette color: {other:?}"),
        };
        rgb.into_iter()
            .zip([0.2126, 0.7152, 0.0722])
            .map(|(channel, weight)| {
                let c = channel as f64 / 255.0;
                weight
                    * if c <= 0.04045 {
                        c / 12.92
                    } else {
                        ((c + 0.055) / 1.055).powf(2.4)
                    }
            })
            .sum()
    }

    #[test]
    fn reduced_motion_spinner_is_static_in_both_glyph_sets() {
        for ascii in [false, true] {
            let th = Theme {
                truecolor: false,
                ascii,
                reduced_motion: true,
            };
            assert_eq!(th.spinner(0), th.spinner(3));
            assert_eq!(th.spinner(0), th.spinner(7));
        }
    }

    #[test]
    fn secondary_text_is_readable_on_every_surface_in_both_palettes() {
        for truecolor in [true, false] {
            let th = Theme {
                truecolor,
                ascii: false,
                reduced_motion: false,
            };
            for background in [th.bg(), th.panel(), th.bar(), th.highlight()] {
                let ratio = (luminance(th.dim()) + 0.05) / (luminance(background) + 0.05);
                assert!(ratio >= 4.5, "{truecolor}: {background:?}: {ratio}");
            }
        }
    }
}
