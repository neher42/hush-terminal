use alacritty_terminal::vte::ansi::{Color, NamedColor};
use gpui::{Hsla, Rgba};

/// Same palettes as the web version (src/theme.ts).
pub struct Palette {
    pub foreground: u32,
    pub background: u32,
    /// Title bar and sidebar.
    pub chrome: u32,
    pub cursor: u32,
    pub selection: u32,
    pub selection_alpha: f32,
    pub ansi: [u32; 16],
    pub line: u32,
    pub secondary: u32,
    pub muted: u32,
    pub faint: u32,
    pub accent: u32,
    /// Faint wash behind a hovered command.
    pub hover: u32,
    /// The current folder in the sidebar.
    pub current_row: u32,
    pub current_row_text: u32,
    pub menu: u32,
    pub menu_selected: u32,
    pub tooltip: u32,
    pub tooltip_text: u32,
    pub dark: bool,
}

pub const LIGHT: Palette = Palette {
    foreground: 0x1b1e23,
    background: 0xffffff,
    chrome: 0xf5f6f8,
    cursor: 0x1b1e23,
    selection: 0x2a63c7,
    selection_alpha: 0.22,
    ansi: [
        0x1b1e23, 0xcc3a2f, 0x25804b, 0xa2670a, 0x2a63c7, 0x8a4ec2, 0x1b7f8c, 0xc9ced6, //
        0xa9afb8, 0xcc3a2f, 0x25804b, 0xa2670a, 0x2a63c7, 0x8a4ec2, 0x1b7f8c, 0x1b1e23,
    ],
    line: 0xe2e5ea,
    secondary: 0x3d434c,
    muted: 0x878e99,
    faint: 0xa9afb8,
    accent: 0x2a63c7,
    hover: 0xf6f7f9,
    current_row: 0xe2eafa,
    current_row_text: 0x1d4ea6,
    menu: 0xffffff,
    menu_selected: 0xe7eefa,
    tooltip: 0x1b1e23,
    tooltip_text: 0xffffff,
    dark: false,
};

pub const DARK: Palette = Palette {
    foreground: 0xdde1e7,
    background: 0x16181c,
    chrome: 0x1b1e23,
    cursor: 0xdde1e7,
    selection: 0x7aa8ff,
    selection_alpha: 0.30,
    ansi: [
        0x3a3f47, 0xff6e61, 0x7fcf98, 0xe2b25d, 0x7aa8ff, 0xc4a1ff, 0x5fc7d3, 0xdde1e7, //
        0x5a616c, 0xff6e61, 0x7fcf98, 0xe2b25d, 0x7aa8ff, 0xc4a1ff, 0x5fc7d3, 0xdde1e7,
    ],
    line: 0x2a2e35,
    secondary: 0xb9bfc8,
    muted: 0x737a85,
    faint: 0x5a616c,
    accent: 0x7aa8ff,
    hover: 0x1c1f24,
    current_row: 0x223150,
    current_row_text: 0xadc8ff,
    menu: 0x202329,
    menu_selected: 0x2a3447,
    tooltip: 0xe6e9ee,
    tooltip_text: 0x16181c,
    dark: true,
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
