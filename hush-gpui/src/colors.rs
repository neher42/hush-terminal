use alacritty_terminal::vte::ansi::{Color, NamedColor};
use gpui::{Hsla, Rgba};

/// Same light palette as the web version (src/theme.ts).
pub struct Palette {
    pub foreground: u32,
    pub background: u32,
    pub cursor: u32,
    pub selection: u32,
    pub selection_alpha: f32,
    pub ansi: [u32; 16],
    pub line: u32,
    pub muted: u32,
    pub accent: u32,
    /// Faint wash behind a hovered command.
    pub hover: u32,
    pub menu: u32,
    pub menu_selected: u32,
    pub tooltip: u32,
    pub tooltip_text: u32,
}

pub const LIGHT: Palette = Palette {
    foreground: 0x1b1e23,
    background: 0xffffff,
    cursor: 0x1b1e23,
    selection: 0x2a63c7,
    selection_alpha: 0.22,
    ansi: [
        0x1b1e23, 0xcc3a2f, 0x25804b, 0xa2670a, 0x2a63c7, 0x8a4ec2, 0x1b7f8c, 0xc9ced6, //
        0xa9afb8, 0xcc3a2f, 0x25804b, 0xa2670a, 0x2a63c7, 0x8a4ec2, 0x1b7f8c, 0x1b1e23,
    ],
    line: 0xe2e5ea,
    muted: 0x878e99,
    accent: 0x2a63c7,
    hover: 0xf6f7f9,
    menu: 0xffffff,
    menu_selected: 0xe7eefa,
    tooltip: 0x1b1e23,
    tooltip_text: 0xffffff,
};

impl Palette {
    pub fn resolve(&self, color: Color) -> Hsla {
        hsla(match color {
            Color::Spec(rgb) => {
                (u32::from(rgb.r) << 16) | (u32::from(rgb.g) << 8) | u32::from(rgb.b)
            }
            Color::Indexed(i) => self.indexed(i),
            Color::Named(named) => self.named(named),
        })
    }

    pub fn background(&self) -> Hsla {
        hsla(self.background)
    }

    /// Translucent so colored text stays readable.
    pub fn selection(&self) -> Hsla {
        Hsla {
            a: self.selection_alpha,
            ..hsla(self.selection)
        }
    }

    pub fn cursor(&self) -> Hsla {
        hsla(self.cursor)
    }

    fn named(&self, named: NamedColor) -> u32 {
        let index = named as usize;
        if index < 16 {
            return self.ansi[index];
        }
        match named {
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            NamedColor::DimBlack => self.ansi[0],
            NamedColor::DimRed => self.ansi[1],
            NamedColor::DimGreen => self.ansi[2],
            NamedColor::DimYellow => self.ansi[3],
            NamedColor::DimBlue => self.ansi[4],
            NamedColor::DimMagenta => self.ansi[5],
            NamedColor::DimCyan => self.ansi[6],
            NamedColor::DimWhite => self.ansi[7],
            _ => self.foreground,
        }
    }

    /// xterm's 256 colors: 16 ANSI, a 6x6x6 cube, then 24 grays.
    fn indexed(&self, i: u8) -> u32 {
        match i {
            0..=15 => self.ansi[i as usize],
            16..=231 => {
                let i = u32::from(i - 16);
                let level = |v: u32| if v == 0 { 0 } else { 55 + v * 40 };
                (level(i / 36) << 16) | (level(i / 6 % 6) << 8) | level(i % 6)
            }
            _ => {
                let v = 8 + u32::from(i - 232) * 10;
                (v << 16) | (v << 8) | v
            }
        }
    }
}

fn hsla(rgb: u32) -> Hsla {
    Rgba {
        r: ((rgb >> 16) & 0xff) as f32 / 255.0,
        g: ((rgb >> 8) & 0xff) as f32 / 255.0,
        b: (rgb & 0xff) as f32 / 255.0,
        a: 1.0,
    }
    .into()
}
