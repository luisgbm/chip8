//! The colors the front end draws with.

/// Background behind the menu and the status bar.
pub const BACKGROUND: u32 = 0x000C_1014;

/// Background of a panel or of the selected row.
pub const PANEL: u32 = 0x0019_212B;

/// Rules between sections.
pub const SEPARATOR: u32 = 0x0026_3040;

/// Ordinary text.
pub const TEXT: u32 = 0x00D6_DEE8;

/// Secondary text: hints, tags, sizes.
pub const DIM: u32 = 0x0070_7E8C;

/// Headings.
pub const ACCENT: u32 = 0x007F_E07F;

/// The selected row and anything else that needs to stand out.
pub const HIGHLIGHT: u32 = 0x00FF_CC66;

/// Faults and failed loads.
pub const ERROR: u32 = 0x00FF_7B72;

/// A lit CHIP-8 pixel.
pub const SCREEN_ON: u32 = 0x00D7_F2C4;

/// An unlit CHIP-8 pixel.
pub const SCREEN_OFF: u32 = 0x000C_120C;
