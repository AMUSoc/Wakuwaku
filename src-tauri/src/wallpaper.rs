// Windows' mode and accent colour (look), for her island's look in the
// settings and on the page.

// Windows' accent colour as #rrggbb.
#[derive(Clone, Debug, PartialEq)]
pub struct Accent {
    pub base: String,
    // As its own taskbar and menus draw with it: on a dark ground the
    // colour itself is too dim, on a light one too pale.
    pub light: String,
    pub dark: String,
}

// From its palette (Explorer\Accent's AccentPalette): eight colours of four
// bytes (red, green, blue, unused), from the lightest (Light3, Light2,
// Light1) through the colour itself to the darkest (Dark1, Dark2, Dark3).
// Light2 on dark, Dark1 on light, as Windows' own.
pub fn accent_of(palette: &[u8]) -> Option<Accent> {
    let at = |i: usize| palette.get(i * 4..i * 4 + 3).map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]));
    Some(Accent { base: at(3)?, light: at(1)?, dark: at(4)? })
}

// Windows' mode for its own taskbar and Start (Settings' "Windows mode"),
// light or not; and its accent colour.
#[cfg(windows)]
pub fn look() -> (bool, Option<Accent>) {
    let user = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let light = user.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize").and_then(|k| k.get_value::<u32, _>("SystemUsesLightTheme")).is_ok_and(|v| v != 0);
    let accent = user.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent").ok().and_then(|k| k.get_raw_value("AccentPalette").ok()).and_then(|v| accent_of(&v.bytes));
    (light, accent)
}

#[cfg(not(windows))]
pub fn look() -> (bool, Option<Accent>) {
    (false, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Windows' default blue, as its palette holds it.
    #[test]
    fn reads_the_accent_and_its_light_and_dark_ones_from_the_palette() {
        let palette = [0x99, 0xEB, 0xFF, 0, 0x4C, 0xC2, 0xFF, 0, 0x00, 0x91, 0xF8, 0, 0x00, 0x78, 0xD4, 0, 0x00, 0x67, 0xC0, 0, 0x00, 0x3E, 0x92, 0, 0x00, 0x1A, 0x68, 0, 0xF7, 0x63, 0x0C, 0];
        assert_eq!(accent_of(&palette), Some(Accent { base: "#0078d4".into(), light: "#4cc2ff".into(), dark: "#0067c0".into() }));
        assert_eq!(accent_of(&palette[..16]), None);
    }
}
