use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

pub const ACCENT_A: (u8, u8, u8) = (0x7e, 0xe7, 0x87);
pub const ACCENT_B: (u8, u8, u8) = (0xff, 0x7b, 0x72);

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub truecolor: bool,
    pub ascii: bool,
}

impl Theme {
    pub fn detect(ascii: bool) -> Self {
        let truecolor = std::env::var("COLORTERM")
            .map(|v| v.contains("truecolor") || v.contains("24bit"))
            .unwrap_or(false);
        Self { truecolor, ascii }
    }

    fn rgb_or(&self, rgb: (u8, u8, u8), indexed: u8) -> Color {
        if self.truecolor {
            Color::Rgb(rgb.0, rgb.1, rgb.2)
        } else {
            Color::Indexed(indexed)
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
        self.rgb_or((0x5c, 0x63, 0x70), 242)
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
