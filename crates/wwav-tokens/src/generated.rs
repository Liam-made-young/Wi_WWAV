// Generated from design/tokens.json by tools/tokens/compile.mjs.
// Don't edit it: change the JSON and run the compiler again.

pub mod desk {
    //! Heat and the Console's chrome, Settings and every sheet (8.2). Light and
    //! dark are Heat's tokens with 8.2's five fixes, and two inks moved so they
    //! hold on every stop of their ground (see $contrast). A gel pill draws its
    //! stops top to bottom with a hard break at gelBreak: a plain gel's two as
    //! first, first, second, first; a blue gel's four as listed. A segment or
    //! chip draws a plain gel's two as a plain gradient.
    pub const INK: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0x1b, 0x1b, 0x1b), dark: crate::Color::rgb(0xec, 0xec, 0xec) };
    pub const INK2: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0x4a, 0x4a, 0x4a), dark: crate::Color::rgb(0xc2, 0xc2, 0xc2) };
    pub const INK3: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0x5f, 0x5f, 0x5f), dark: crate::Color::rgb(0x9a, 0x9a, 0x9a) };
    pub const WELL: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0xff, 0xff, 0xff), dark: crate::Color::rgb(0x26, 0x28, 0x2b) };
    pub const STRIPE: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0xed, 0xf3, 0xfe), dark: crate::Color::rgb(0x2c, 0x30, 0x38) };
    pub const CASE_METAL: crate::Themed<&[crate::Color]> = crate::Themed { light: &[crate::Color::rgb(0xe4, 0xe4, 0xe4), crate::Color::rgb(0xc9, 0xc9, 0xc9)], dark: &[crate::Color::rgb(0x5a, 0x5d, 0x61), crate::Color::rgb(0x3f, 0x42, 0x46)] };
    pub const DECK_METAL: crate::Themed<&[crate::Color]> = crate::Themed { light: &[crate::Color::rgb(0xfb, 0xfc, 0xfd), crate::Color::rgb(0xe6, 0xea, 0xef), crate::Color::rgb(0xc9, 0xd0, 0xd8)], dark: &[crate::Color::rgb(0x5a, 0x5d, 0x61), crate::Color::rgb(0x3f, 0x42, 0x46)] };
    pub const DECK_EDGE: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0x98, 0xa2, 0xad), dark: crate::Color::rgb(0x26, 0x28, 0x2b) };
    pub const DECK_PINSTRIPE: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0xe9, 0xef, 0xf6), dark: crate::Color::rgba(0x00, 0x00, 0x00, 0.0) };
    pub const PINSTRIPE_EVERY_PX: f32 = 4.0;
    pub const DECK_INK: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0x16, 0x1d, 0x26), dark: crate::Color::rgb(0xec, 0xec, 0xec) };
    pub const DECK_INK_SOFT: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0x4f, 0x59, 0x63), dark: crate::Color::rgb(0xec, 0xec, 0xec) };
    pub const SELECT: crate::Color = crate::Color::rgb(0x1b, 0x4c, 0x8c);
    pub const SELECT_INK: crate::Color = crate::Color::rgb(0xff, 0xff, 0xff);
    pub const SELECT_BAR_PX: f32 = 3.0;
    pub const HIGHLIGHT: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0x38, 0x75, 0xd7), dark: crate::Color::rgb(0x3a, 0x6f, 0xc4) };
    pub const FOCUS: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgba(0x38, 0x75, 0xd7, 0.6), dark: crate::Color::rgba(0x3a, 0x6f, 0xc4, 0.6) };
    pub const FOCUS_WIDTH_PX: f32 = 3.0;
    pub const GEL: crate::Themed<&[crate::Color]> = crate::Themed { light: &[crate::Color::rgb(0xfe, 0xfe, 0xfe), crate::Color::rgb(0xd2, 0xd2, 0xd2)], dark: &[crate::Color::rgb(0x5e, 0x61, 0x65), crate::Color::rgb(0x46, 0x49, 0x4d)] };
    pub const GEL_BLUE: &[crate::Color] = &[crate::Color::rgb(0xa9, 0xcd, 0xf6), crate::Color::rgb(0x6a, 0xa8, 0xef), crate::Color::rgb(0x3e, 0x86, 0xdc), crate::Color::rgb(0x7f, 0xb8, 0xf5)];
    pub const GEL_BLUE_INK: crate::Color = crate::Color::rgb(0x1b, 0x1b, 0x1b);
    pub const GEL_BREAK: &[f32] = &[0.45, 0.55];
    pub const GEL_SELECTED: &[crate::Color] = &[crate::Color::rgb(0x33, 0x6d, 0xcc), crate::Color::rgb(0x1b, 0x4c, 0x8c)];
    pub const GEL_PRESS: f32 = 0.88;
    pub const BACKDROP: crate::Color = crate::Color::rgba(0x00, 0x00, 0x00, 0.25);

    pub mod lcd {
        //! The Now strip: Heat's olive LCD (2.2). Its dim text is darkened in
        //! light (8.2) and lightened in dark, so it holds on both stops.
        pub const GROUND: crate::Themed<&[crate::Color]> = crate::Themed { light: &[crate::Color::rgb(0xf2, 0xf4, 0xe4), crate::Color::rgb(0xdf, 0xe3, 0xc6)], dark: &[crate::Color::rgb(0x2c, 0x31, 0x21), crate::Color::rgb(0x20, 0x24, 0x1a)] };
        pub const INK: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0x26, 0x2a, 0x17), dark: crate::Color::rgb(0xd7, 0xe0, 0xa8) };
        pub const DIM: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0x4b, 0x50, 0x34), dark: crate::Color::rgb(0x9c, 0xa3, 0x82) };
    }

    pub mod source_list {
        //! The library drawer and Heat's sidebar, with headings darkened (8.2).
        pub const GROUND: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0xe7, 0xec, 0xf2), dark: crate::Color::rgb(0x2e, 0x31, 0x36) };
        pub const HEADING: crate::Themed<crate::Color> = crate::Themed { light: crate::Color::rgb(0x4f, 0x57, 0x61), dark: crate::Color::rgb(0x9a, 0xa3, 0xad) };
    }
}

pub mod screen {
    //! The Game Boy inside: every Console readout is a DMG screen, the same in
    //! light and dark (8.2, 5.2; MI-WWAV-OS theme.dart). Meters are DMG green
    //! to -6 dBFS, amber to -1, rec above.

    pub mod dmg {
        pub const DEEP: crate::Color = crate::Color::rgb(0x0b, 0x1f, 0x0b);
        pub const DARK: crate::Color = crate::Color::rgb(0x0f, 0x38, 0x0f);
        pub const MID: crate::Color = crate::Color::rgb(0x30, 0x62, 0x30);
        pub const LIT: crate::Color = crate::Color::rgb(0x8b, 0xac, 0x0f);
    }
    pub const GLOW: crate::Color = crate::Color::rgb(0xc6, 0xe2, 0x4a);
    pub const AMBER: crate::Color = crate::Color::rgb(0xf0, 0xa3, 0x2e);
    pub const REC: crate::Color = crate::Color::rgb(0xe0, 0x45, 0x3a);
    pub const METER_AMBER_FROM_DB: f32 = -6.0;
    pub const METER_REC_FROM_DB: f32 = -1.0;
}

pub mod night {
    //! Space and the expanded player, in both appearances (8.2; v3, v4). Seven
    //! ink opacities are the only hierarchy scale; royal blue has three uses
    //! and is never text.
    pub const GROUND: crate::Color = crate::Color::rgb(0x07, 0x0a, 0x18);
    pub const DEEP: crate::Color = crate::Color::rgb(0x03, 0x05, 0x10);
    pub const CLAY: crate::Color = crate::Color::rgb(0x1a, 0x21, 0x42);
    pub const INK: crate::Color = crate::Color::rgb(0xf4, 0xef, 0xe6);
    pub const INK_STEPS: &[f32] = &[1.0, 0.7, 0.5, 0.35, 0.2, 0.12, 0.07];

    pub mod text_steps {
        pub const BODY: &[f32] = &[1.0, 0.7];
        pub const SECONDARY: &[f32] = &[0.5];
    }
    pub const STAR: crate::Color = crate::Color::rgb(0xdd, 0xe4, 0xfb);
    pub const ACCENT: crate::Color = crate::Color::rgb(0x29, 0x46, 0xff);
    pub const ACCENT_USES: &[&str] = &["armed", "branch", "commit"];
    pub const SUN: &[crate::Color] = &[crate::Color::rgb(0xff, 0xf8, 0xe6), crate::Color::rgb(0xff, 0xd8, 0x9a), crate::Color::rgb(0xf0, 0xa2, 0x3c), crate::Color::rgb(0xb2, 0x5f, 0x12)];
    pub const FOCUS_WIDTH_PX: f32 = 2.0;
}

pub mod room {
    //! Unquantized, lit rather than painted (8.2; v3's mock, PRANA, Crater).
    //! Dusk is light, after hours is dark: the walls fall to limousine and
    //! paper stays paper.

    pub mod field {
        pub const GROUND: crate::Themed<&[crate::Color]> = crate::Themed { light: &[crate::Color::rgb(0xf7, 0xf4, 0xee), crate::Color::rgb(0xe6, 0xdf, 0xd0)], dark: &[crate::Color::rgb(0x0f, 0x0c, 0x09), crate::Color::rgb(0x0f, 0x0c, 0x09)] };
        pub const ROSE: crate::Color = crate::Color::rgb(0xfb, 0xed, 0xe8);
        pub const COOL: crate::Color = crate::Color::rgb(0xdd, 0xe2, 0xe6);
        pub const DRIFT_SHORTEST_MS: f32 = 53000.0;
        pub const DRIFT_LONGEST_MS: f32 = 131000.0;
    }

    pub mod material {
        pub const SAND: crate::Color = crate::Color::rgb(0xe8, 0xdc, 0xc8);
        pub const SAND_DEEP: crate::Color = crate::Color::rgb(0xd4, 0xc4, 0xa8);
        pub const CLAY: crate::Color = crate::Color::rgb(0xb8, 0x98, 0x78);
        pub const CLAY_DEEP: crate::Color = crate::Color::rgb(0x7a, 0x5e, 0x45);
    }
    pub const INK: crate::Color = crate::Color::rgb(0x3d, 0x2e, 0x22);
    pub const INK2: crate::Color = crate::Color::rgb(0x6b, 0x56, 0x43);
    pub const PAPER: crate::Color = crate::Color::rgb(0xff, 0xe9, 0xc8);
    pub const ACCENT: crate::Color = crate::Color::rgb(0xc8, 0x96, 0x68);
    pub const LAMP: &[crate::Color] = &[crate::Color::rgb(0xe8, 0x91, 0x5b), crate::Color::rgb(0xf4, 0xb4, 0x83)];
    pub const SKY: &[crate::Color] = &[crate::Color::rgb(0xf8, 0xd0, 0xa4), crate::Color::rgb(0xd4, 0x76, 0x3f)];
    pub const HAIRLINE_PX: f32 = 1.0;
    pub const FOCUS_WIDTH_PX: f32 = 2.0;
}

pub mod stem {
    //! PRANA's stem colours, the same everywhere, in stem order (8.3). Every
    //! stem mark carries a ring in its register's ink; state is shape: muted is
    //! a stem-colour ring, soloed adds an outer ring, silent under a solo fills
    //! at 45% with a dashed ring, selected adds a notch.
    pub const VOCALS: crate::Color = crate::Color::rgb(0xd2, 0x3c, 0x2a);
    pub const DRUMS: crate::Color = crate::Color::rgb(0xf0, 0xb9, 0x0b);
    pub const OTHER: crate::Color = crate::Color::rgb(0x2e, 0x9a, 0x55);
    pub const BASS: crate::Color = crate::Color::rgb(0x1f, 0x4e, 0x9e);
    pub const RING_PX: f32 = 1.0;
    pub const MUTED_RING_PX: f32 = 2.0;
    pub const SOLO_RING_PX: f32 = 2.0;
    pub const SILENT_FILL: f32 = 0.45;
    pub const NOTCH_PX: f32 = 2.0;
}

pub mod heat {
    //! Heat's level colours (3.1). They fill tubes, pill borders and dots; the
    //! level word is ink.
    pub const COOL: crate::Color = crate::Color::rgb(0x4f, 0x9b, 0xe6);
    pub const WARM: crate::Color = crate::Color::rgb(0xef, 0xa4, 0x31);
    pub const HOT: crate::Color = crate::Color::rgb(0xe0, 0x40, 0x2c);
    pub const OVERDUE: crate::Color = crate::Color::rgb(0x8f, 0x1d, 0x16);
}

pub mod label {
    //! The six colour labels, for sorting only (2.5).
    pub const RED: crate::Color = crate::Color::rgb(0xe0, 0x38, 0x3e);
    pub const ORANGE: crate::Color = crate::Color::rgb(0xf0, 0x8a, 0x2c);
    pub const YELLOW: crate::Color = crate::Color::rgb(0xe8, 0xc7, 0x3a);
    pub const GREEN: crate::Color = crate::Color::rgb(0x3c, 0xaa, 0x68);
    pub const BLUE: crate::Color = crate::Color::rgb(0x3d, 0x7d, 0xd5);
    pub const PURPLE: crate::Color = crate::Color::rgb(0x93, 0x59, 0xc9);
}

pub mod key {
    //! A work's key colour (8.3, v4 keyColor.js): hue = ((pc * 7) mod 12) * 30
    //! with A = 0, in hsl with the mode's saturation and lightness. An unknown
    //! key is night indigo in the minor look. The glow tone an orb's rim takes
    //! is the same hue and saturation at the glow lightness.
    pub const UNKNOWN_HUE: f32 = 232.0;

    pub mod major {
        pub const SATURATION: f32 = 72.0;
        pub const LIGHTNESS: f32 = 58.0;
    }

    pub mod minor {
        pub const SATURATION: f32 = 58.0;
        pub const LIGHTNESS: f32 = 42.0;
    }
    pub const GLOW_LIGHTNESS: f32 = 65.0;
}

pub mod light {
    //! One light, from the upper left, as a fraction of the box (8.3, v3
    //! WWAVLight.sun).

    pub mod sun {
        pub const X: f32 = 0.3;
        pub const Y: f32 = 0.25;
    }
}

pub mod motion {
    //! Built on MI-WWAV-OS's 140 ms beat with v4's curves (8.6). Under Reduce
    //! Motion every move is a beat-long cross-fade. Nothing bounces.
    pub const PRESS_MS: f32 = 70.0;
    pub const BEAT_MS: f32 = 140.0;
    pub const FADE_MS: f32 = 280.0;
    pub const MORPH_MS: f32 = 420.0;
    pub const WASH_MS: f32 = 760.0;
    pub const DIVE_MS: f32 = 900.0;
    pub const TOAST_MS: f32 = 2600.0;
    pub const REST: crate::Curve = crate::Curve { x1: 0.22, y1: 1.0, x2: 0.36, y2: 1.0 };
    pub const SHEET: crate::Curve = crate::Curve { x1: 0.32, y1: 0.72, x2: 0.0, y2: 1.0 };
}

pub mod space {
    //! A 4 px grid (8.3).
    pub const S1_PX: f32 = 4.0;
    pub const S2_PX: f32 = 8.0;
    pub const S3_PX: f32 = 12.0;
    pub const S4_PX: f32 = 16.0;
    pub const S5_PX: f32 = 24.0;
    pub const S6_PX: f32 = 32.0;
    pub const S7_PX: f32 = 48.0;
    pub const S8_PX: f32 = 64.0;
}

pub mod radius {
    //! Corners (8.3). Gels are full pills; a sheet rounds only its lower
    //! corners.
    pub const CHIP_PX: f32 = 6.0;
    pub const BUTTON_PX: f32 = 10.0;
    pub const CARD_PX: f32 = 14.0;
    pub const SHEET_PX: f32 = 8.0;
    pub const PILL_PX: f32 = 999.0;
}

pub mod r#type {
    //! Faces and sizes (8.4). Nothing is under 11; text read for more than a
    //! line is at least the reading size (8.9, open: 17 or 19).

    pub mod desk {
        pub const FAMILY: &[&str] = &["Lucida Grande", "Lucida Sans Unicode", "Lucida Sans", "sans-serif"];
        pub const SMALL_PX: f32 = 11.0;
        pub const BODY_PX: f32 = 13.0;
        pub const LARGE_PX: f32 = 15.0;
        pub const TITLE_PX: f32 = 20.0;
        pub const LINE_HEIGHT: f32 = 1.35;
    }

    pub mod mono {
        pub const FAMILY: &[&str] = &["Menlo", "Consolas", "monospace"];
        pub const SMALL_PX: f32 = 11.0;
        pub const BODY_PX: f32 = 13.0;
        pub const LARGE_PX: f32 = 20.0;
    }

    pub mod name {
        pub const FAMILY: &[&str] = &["Cormorant Garamond", "serif"];
        pub const MIN_PX: f32 = 20.0;
        pub const HEADER_PX: f32 = 30.0;
        pub const SUN_PX: f32 = 46.0;
        pub const LINE_HEIGHT: f32 = 1.18;
        pub const WEIGHT: f32 = 400.0;
        pub const LIGHT_WEIGHT: f32 = 300.0;
        pub const LIGHT_WEIGHT_FROM_PX: f32 = 28.0;
    }

    pub mod text {
        pub const FAMILY: &[&str] = &["Inter", "sans-serif"];
        pub const LABEL_PX: f32 = 11.0;
        pub const LABEL_TRACKING_EM: f32 = 0.08;
        pub const SMALL_PX: f32 = 13.0;
        pub const BODY_PX: f32 = 15.0;
        pub const READING_PX: f32 = 17.0;
    }

    pub mod sign {
        pub const FAMILY: &[&str] = &["Jost", "sans-serif"];
        pub const MIN_PX: f32 = 11.0;
        pub const MAX_PX: f32 = 15.0;
        pub const TRACKING_EM: f32 = 0.34;
    }
}

/// Every token, by its path in design/tokens.json.
pub const ALL: &[(&str, crate::Token)] = &[
    ("desk.ink", crate::Token::ThemedColor(desk::INK)),
    ("desk.ink2", crate::Token::ThemedColor(desk::INK2)),
    ("desk.ink3", crate::Token::ThemedColor(desk::INK3)),
    ("desk.well", crate::Token::ThemedColor(desk::WELL)),
    ("desk.stripe", crate::Token::ThemedColor(desk::STRIPE)),
    ("desk.caseMetal", crate::Token::ThemedColors(desk::CASE_METAL)),
    ("desk.deckMetal", crate::Token::ThemedColors(desk::DECK_METAL)),
    ("desk.deckEdge", crate::Token::ThemedColor(desk::DECK_EDGE)),
    ("desk.deckPinstripe", crate::Token::ThemedColor(desk::DECK_PINSTRIPE)),
    ("desk.pinstripeEvery", crate::Token::Number(desk::PINSTRIPE_EVERY_PX)),
    ("desk.deckInk", crate::Token::ThemedColor(desk::DECK_INK)),
    ("desk.deckInkSoft", crate::Token::ThemedColor(desk::DECK_INK_SOFT)),
    ("desk.select", crate::Token::Color(desk::SELECT)),
    ("desk.selectInk", crate::Token::Color(desk::SELECT_INK)),
    ("desk.selectBar", crate::Token::Number(desk::SELECT_BAR_PX)),
    ("desk.highlight", crate::Token::ThemedColor(desk::HIGHLIGHT)),
    ("desk.focus", crate::Token::ThemedColor(desk::FOCUS)),
    ("desk.focusWidth", crate::Token::Number(desk::FOCUS_WIDTH_PX)),
    ("desk.gel", crate::Token::ThemedColors(desk::GEL)),
    ("desk.gelBlue", crate::Token::Colors(desk::GEL_BLUE)),
    ("desk.gelBlueInk", crate::Token::Color(desk::GEL_BLUE_INK)),
    ("desk.gelBreak", crate::Token::Numbers(desk::GEL_BREAK)),
    ("desk.gelSelected", crate::Token::Colors(desk::GEL_SELECTED)),
    ("desk.gelPress", crate::Token::Number(desk::GEL_PRESS)),
    ("desk.backdrop", crate::Token::Color(desk::BACKDROP)),
    ("desk.lcd.ground", crate::Token::ThemedColors(desk::lcd::GROUND)),
    ("desk.lcd.ink", crate::Token::ThemedColor(desk::lcd::INK)),
    ("desk.lcd.dim", crate::Token::ThemedColor(desk::lcd::DIM)),
    ("desk.sourceList.ground", crate::Token::ThemedColor(desk::source_list::GROUND)),
    ("desk.sourceList.heading", crate::Token::ThemedColor(desk::source_list::HEADING)),
    ("screen.dmg.deep", crate::Token::Color(screen::dmg::DEEP)),
    ("screen.dmg.dark", crate::Token::Color(screen::dmg::DARK)),
    ("screen.dmg.mid", crate::Token::Color(screen::dmg::MID)),
    ("screen.dmg.lit", crate::Token::Color(screen::dmg::LIT)),
    ("screen.glow", crate::Token::Color(screen::GLOW)),
    ("screen.amber", crate::Token::Color(screen::AMBER)),
    ("screen.rec", crate::Token::Color(screen::REC)),
    ("screen.meterAmberFromDb", crate::Token::Number(screen::METER_AMBER_FROM_DB)),
    ("screen.meterRecFromDb", crate::Token::Number(screen::METER_REC_FROM_DB)),
    ("night.ground", crate::Token::Color(night::GROUND)),
    ("night.deep", crate::Token::Color(night::DEEP)),
    ("night.clay", crate::Token::Color(night::CLAY)),
    ("night.ink", crate::Token::Color(night::INK)),
    ("night.inkSteps", crate::Token::Numbers(night::INK_STEPS)),
    ("night.textSteps.body", crate::Token::Numbers(night::text_steps::BODY)),
    ("night.textSteps.secondary", crate::Token::Numbers(night::text_steps::SECONDARY)),
    ("night.star", crate::Token::Color(night::STAR)),
    ("night.accent", crate::Token::Color(night::ACCENT)),
    ("night.accentUses", crate::Token::Names(night::ACCENT_USES)),
    ("night.sun", crate::Token::Colors(night::SUN)),
    ("night.focusWidth", crate::Token::Number(night::FOCUS_WIDTH_PX)),
    ("room.field.ground", crate::Token::ThemedColors(room::field::GROUND)),
    ("room.field.rose", crate::Token::Color(room::field::ROSE)),
    ("room.field.cool", crate::Token::Color(room::field::COOL)),
    ("room.field.driftShortest", crate::Token::Number(room::field::DRIFT_SHORTEST_MS)),
    ("room.field.driftLongest", crate::Token::Number(room::field::DRIFT_LONGEST_MS)),
    ("room.material.sand", crate::Token::Color(room::material::SAND)),
    ("room.material.sandDeep", crate::Token::Color(room::material::SAND_DEEP)),
    ("room.material.clay", crate::Token::Color(room::material::CLAY)),
    ("room.material.clayDeep", crate::Token::Color(room::material::CLAY_DEEP)),
    ("room.ink", crate::Token::Color(room::INK)),
    ("room.ink2", crate::Token::Color(room::INK2)),
    ("room.paper", crate::Token::Color(room::PAPER)),
    ("room.accent", crate::Token::Color(room::ACCENT)),
    ("room.lamp", crate::Token::Colors(room::LAMP)),
    ("room.sky", crate::Token::Colors(room::SKY)),
    ("room.hairline", crate::Token::Number(room::HAIRLINE_PX)),
    ("room.focusWidth", crate::Token::Number(room::FOCUS_WIDTH_PX)),
    ("stem.vocals", crate::Token::Color(stem::VOCALS)),
    ("stem.drums", crate::Token::Color(stem::DRUMS)),
    ("stem.other", crate::Token::Color(stem::OTHER)),
    ("stem.bass", crate::Token::Color(stem::BASS)),
    ("stem.ring", crate::Token::Number(stem::RING_PX)),
    ("stem.mutedRing", crate::Token::Number(stem::MUTED_RING_PX)),
    ("stem.soloRing", crate::Token::Number(stem::SOLO_RING_PX)),
    ("stem.silentFill", crate::Token::Number(stem::SILENT_FILL)),
    ("stem.notch", crate::Token::Number(stem::NOTCH_PX)),
    ("heat.cool", crate::Token::Color(heat::COOL)),
    ("heat.warm", crate::Token::Color(heat::WARM)),
    ("heat.hot", crate::Token::Color(heat::HOT)),
    ("heat.overdue", crate::Token::Color(heat::OVERDUE)),
    ("label.red", crate::Token::Color(label::RED)),
    ("label.orange", crate::Token::Color(label::ORANGE)),
    ("label.yellow", crate::Token::Color(label::YELLOW)),
    ("label.green", crate::Token::Color(label::GREEN)),
    ("label.blue", crate::Token::Color(label::BLUE)),
    ("label.purple", crate::Token::Color(label::PURPLE)),
    ("key.unknownHue", crate::Token::Number(key::UNKNOWN_HUE)),
    ("key.major.saturation", crate::Token::Number(key::major::SATURATION)),
    ("key.major.lightness", crate::Token::Number(key::major::LIGHTNESS)),
    ("key.minor.saturation", crate::Token::Number(key::minor::SATURATION)),
    ("key.minor.lightness", crate::Token::Number(key::minor::LIGHTNESS)),
    ("key.glowLightness", crate::Token::Number(key::GLOW_LIGHTNESS)),
    ("light.sun.x", crate::Token::Number(light::sun::X)),
    ("light.sun.y", crate::Token::Number(light::sun::Y)),
    ("motion.press", crate::Token::Number(motion::PRESS_MS)),
    ("motion.beat", crate::Token::Number(motion::BEAT_MS)),
    ("motion.fade", crate::Token::Number(motion::FADE_MS)),
    ("motion.morph", crate::Token::Number(motion::MORPH_MS)),
    ("motion.wash", crate::Token::Number(motion::WASH_MS)),
    ("motion.dive", crate::Token::Number(motion::DIVE_MS)),
    ("motion.toast", crate::Token::Number(motion::TOAST_MS)),
    ("motion.rest", crate::Token::Curve(motion::REST)),
    ("motion.sheet", crate::Token::Curve(motion::SHEET)),
    ("space.s1", crate::Token::Number(space::S1_PX)),
    ("space.s2", crate::Token::Number(space::S2_PX)),
    ("space.s3", crate::Token::Number(space::S3_PX)),
    ("space.s4", crate::Token::Number(space::S4_PX)),
    ("space.s5", crate::Token::Number(space::S5_PX)),
    ("space.s6", crate::Token::Number(space::S6_PX)),
    ("space.s7", crate::Token::Number(space::S7_PX)),
    ("space.s8", crate::Token::Number(space::S8_PX)),
    ("radius.chip", crate::Token::Number(radius::CHIP_PX)),
    ("radius.button", crate::Token::Number(radius::BUTTON_PX)),
    ("radius.card", crate::Token::Number(radius::CARD_PX)),
    ("radius.sheet", crate::Token::Number(radius::SHEET_PX)),
    ("radius.pill", crate::Token::Number(radius::PILL_PX)),
    ("type.desk.family", crate::Token::Names(r#type::desk::FAMILY)),
    ("type.desk.small", crate::Token::Number(r#type::desk::SMALL_PX)),
    ("type.desk.body", crate::Token::Number(r#type::desk::BODY_PX)),
    ("type.desk.large", crate::Token::Number(r#type::desk::LARGE_PX)),
    ("type.desk.title", crate::Token::Number(r#type::desk::TITLE_PX)),
    ("type.desk.lineHeight", crate::Token::Number(r#type::desk::LINE_HEIGHT)),
    ("type.mono.family", crate::Token::Names(r#type::mono::FAMILY)),
    ("type.mono.small", crate::Token::Number(r#type::mono::SMALL_PX)),
    ("type.mono.body", crate::Token::Number(r#type::mono::BODY_PX)),
    ("type.mono.large", crate::Token::Number(r#type::mono::LARGE_PX)),
    ("type.name.family", crate::Token::Names(r#type::name::FAMILY)),
    ("type.name.min", crate::Token::Number(r#type::name::MIN_PX)),
    ("type.name.header", crate::Token::Number(r#type::name::HEADER_PX)),
    ("type.name.sun", crate::Token::Number(r#type::name::SUN_PX)),
    ("type.name.lineHeight", crate::Token::Number(r#type::name::LINE_HEIGHT)),
    ("type.name.weight", crate::Token::Number(r#type::name::WEIGHT)),
    ("type.name.lightWeight", crate::Token::Number(r#type::name::LIGHT_WEIGHT)),
    ("type.name.lightWeightFrom", crate::Token::Number(r#type::name::LIGHT_WEIGHT_FROM_PX)),
    ("type.text.family", crate::Token::Names(r#type::text::FAMILY)),
    ("type.text.label", crate::Token::Number(r#type::text::LABEL_PX)),
    ("type.text.labelTracking", crate::Token::Number(r#type::text::LABEL_TRACKING_EM)),
    ("type.text.small", crate::Token::Number(r#type::text::SMALL_PX)),
    ("type.text.body", crate::Token::Number(r#type::text::BODY_PX)),
    ("type.text.reading", crate::Token::Number(r#type::text::READING_PX)),
    ("type.sign.family", crate::Token::Names(r#type::sign::FAMILY)),
    ("type.sign.min", crate::Token::Number(r#type::sign::MIN_PX)),
    ("type.sign.max", crate::Token::Number(r#type::sign::MAX_PX)),
    ("type.sign.tracking", crate::Token::Number(r#type::sign::TRACKING_EM)),
];
