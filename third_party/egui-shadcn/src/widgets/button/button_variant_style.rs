//! Maps button variant + size + interaction state to concrete style values.

/// Resolves concrete button style from variant, size, and interaction state.
pub fn resolve_button_style(
    theme: &crate::theme::shadcn_theme::ShadcnTheme,
    variant: crate::tokens::button_variant::ButtonVariant,
    size: crate::tokens::component_size::ComponentSize,
    hovered: bool,
    active: bool,
    disabled: bool,
) -> super::resolved_button_style::ResolvedButtonStyle {
    let (height, h_padding, font_size) = size.metrics();
    let corner_radius = theme.radius; // rounded-lg (full radius, not radius - 2)

    let (mut bg, mut fg, border, underline) = match variant {
        crate::tokens::button_variant::ButtonVariant::Default => {
            (theme.primary, theme.primary_foreground, None, false)
        }
        crate::tokens::button_variant::ButtonVariant::Destructive => {
            // Nova: bg-destructive/10 text-destructive (tinted, not solid)
            let tint = egui::Color32::from_rgba_unmultiplied(
                theme.destructive.r(),
                theme.destructive.g(),
                theme.destructive.b(),
                26, // ~10% opacity
            );
            (tint, theme.destructive, None, false)
        }
        crate::tokens::button_variant::ButtonVariant::Outline => (
            theme.background,
            theme.foreground,
            Some(theme.border),
            false,
        ),
        crate::tokens::button_variant::ButtonVariant::Secondary => {
            (theme.secondary, theme.secondary_foreground, None, false)
        }
        crate::tokens::button_variant::ButtonVariant::Ghost => {
            (egui::Color32::TRANSPARENT, theme.foreground, None, false)
        }
        crate::tokens::button_variant::ButtonVariant::Link => {
            (egui::Color32::TRANSPARENT, theme.primary, None, true)
        }
    };

    if disabled {
        // Keep transparent backgrounds transparent (Ghost, Link) so
        // disabled menu items don't get a gray box.
        if bg.a() > 0 {
            bg = with_alpha(bg, 128);
        }
        fg = theme.muted_foreground;
    } else if active {
        match variant {
            crate::tokens::button_variant::ButtonVariant::Default
            | crate::tokens::button_variant::ButtonVariant::Secondary => {
                bg = crate::paint::interpolate_color::interpolate_color(bg, theme.background, 0.24);
            }
            crate::tokens::button_variant::ButtonVariant::Outline
            | crate::tokens::button_variant::ButtonVariant::Ghost => {
                bg = crate::paint::interpolate_color::interpolate_color(
                    theme.accent,
                    theme.background,
                    0.28,
                );
                fg = theme.accent_foreground;
            }
            crate::tokens::button_variant::ButtonVariant::Destructive => {
                bg = crate::paint::interpolate_color::interpolate_color(bg, theme.background, 0.20);
            }
            crate::tokens::button_variant::ButtonVariant::Link => {}
        }
    } else if hovered {
        match variant {
            crate::tokens::button_variant::ButtonVariant::Ghost
            | crate::tokens::button_variant::ButtonVariant::Outline => {
                bg = theme.accent;
                fg = theme.accent_foreground;
            }
            crate::tokens::button_variant::ButtonVariant::Link => {}
            crate::tokens::button_variant::ButtonVariant::Default
            | crate::tokens::button_variant::ButtonVariant::Secondary => {
                bg = crate::paint::interpolate_color::interpolate_color(
                    bg,
                    egui::Color32::WHITE,
                    0.08,
                );
            }
            crate::tokens::button_variant::ButtonVariant::Destructive => {
                bg =
                    crate::paint::interpolate_color::interpolate_color(bg, theme.destructive, 0.12);
            }
        }
    }

    super::resolved_button_style::ResolvedButtonStyle {
        bg,
        fg,
        border,
        corner_radius,
        height,
        h_padding,
        font_size,
        underline,
    }
}

fn with_alpha(c: egui::Color32, a: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}
