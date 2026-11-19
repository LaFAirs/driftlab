//! Embedded fonts: Inter (SIL OFL 1.1) + Phosphor icons (MIT).

use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::sync::Arc;

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        ("inter", &include_bytes!("../../../assets/fonts/Inter-Regular.ttf")[..]),
        ("inter-medium", &include_bytes!("../../../assets/fonts/Inter-Medium.ttf")[..]),
        ("inter-semibold", &include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf")[..]),
    ] {
        fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }
    // Proportional: Inter first, then egui defaults as glyph fallback.
    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    prop.insert(0, "inter".to_owned());
    // "num" family: Inter Medium (Inter has tabular figures; egui renders default figures, so use fixed-width digits via monospace fallback for stability).
    fonts.families.insert(FontFamily::Name("num".into()), vec!["inter-medium".to_owned()]);
    fonts.families.insert(FontFamily::Name("bold".into()), vec!["inter-semibold".to_owned()]);
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
    ctx.global_style_mut(|s| {
        s.text_styles.insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(15.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Heading, FontId::new(26.0, FontFamily::Name("bold".into())));
        s.text_styles.insert(TextStyle::Small, FontId::new(12.5, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Monospace, FontId::new(13.5, FontFamily::Monospace));
    });
}
