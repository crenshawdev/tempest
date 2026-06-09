// SPDX-License-Identifier: GPL-3.0-only

//! Pride Month accent: a tasteful 6-color rainbow flag stripe.
//!
//! Holds the fixed flag palette, two pure decision predicates (so the
//! month/visibility logic is unit-testable without any `cosmic`/render state),
//! and a discrete-segment [`rainbow_bar`] builder. The bar is composed from
//! existing `cosmic::widget` containers (one per color) rather than a canvas —
//! per CLAUDE.md, no new charting/canvas surface is introduced here.

use cosmic::iced::{Color, Length};
use cosmic::widget::{self, container};
use cosmic::Element;

// ── Flag palette (FIXED, theme-independent) ─────────────────────────────────────
//
// DELIBERATE, DOCUMENTED EXCEPTION to the "defers to system theme" convention
// (CONVENTIONS.md / src/meteogram.rs:59-67 documents the same exception for its
// series palette). These six hues *are* the Pride flag — they carry meaning, so
// unlike all other applet chrome they do NOT follow the theme accent. The
// canonical 6-stripe order (top → bottom / left → right) is red, orange, yellow,
// green, blue, purple. DO NOT "fix" these back to theme colors.

/// Red — top stripe.
const RED: Color = Color::from_rgb8(0xE4, 0x03, 0x03);
/// Orange.
const ORANGE: Color = Color::from_rgb8(0xFF, 0x8C, 0x00);
/// Yellow.
const YELLOW: Color = Color::from_rgb8(0xFF, 0xED, 0x00);
/// Green.
const GREEN: Color = Color::from_rgb8(0x00, 0x80, 0x26);
/// Blue.
const BLUE: Color = Color::from_rgb8(0x00, 0x4D, 0xFF);
/// Purple — bottom stripe.
const PURPLE: Color = Color::from_rgb8(0x75, 0x07, 0x87);

/// The six flag colors in canonical stripe order.
const PALETTE: [Color; 6] = [RED, ORANGE, YELLOW, GREEN, BLUE, PURPLE];

/// Returns `true` iff `month` is June (the month is 1-based, matching
/// `chrono::Datelike::month`). Pure — no clock read, no `self`.
#[must_use]
pub fn is_pride_month(month: u32) -> bool {
    month == 6
}

/// Decides whether the panel readout should carry the rainbow accent.
///
/// True iff it is Pride month AND the accent is enabled AND the panel size tier
/// is roomy enough (M/L/XL). Pure — takes the three already-resolved booleans so
/// it can be exhaustively unit-tested without any `cosmic` types.
#[must_use]
pub fn should_show_panel_accent(is_pride_month: bool, enabled: bool, roomy_tier: bool) -> bool {
    is_pride_month && enabled && roomy_tier
}

/// Builds the rainbow accent as six abutting solid-color segments.
///
/// `horizontal` picks the layout: a full-width `Row` of equal-portion segments
/// (popup stripe / horizontal-panel underline) vs. a full-height `Column`
/// (vertical-panel side-stripe). `thickness` is the bar's cross-axis size — the
/// ONE intentional fixed pixel value here (~3px); every other dimension is
/// `Fill`/`FillPortion`. Segments abut with no spacing so the stripes read as a
/// continuous flag.
#[must_use]
pub fn rainbow_bar<'a, M: 'a>(horizontal: bool, thickness: f32) -> Element<'a, M> {
    let segments = PALETTE.into_iter().map(|color| {
        let seg = widget::container(
            widget::Space::new()
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .style(move |_theme| container::Style::default().background(color));
        if horizontal {
            seg.width(Length::FillPortion(1))
                .height(Length::Fixed(thickness))
                .into()
        } else {
            seg.height(Length::FillPortion(1))
                .width(Length::Fixed(thickness))
                .into()
        }
    });

    if horizontal {
        widget::Row::with_children(segments)
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fixed(thickness))
            .into()
    } else {
        widget::Column::with_children(segments)
            .spacing(0)
            .height(Length::Fill)
            .width(Length::Fixed(thickness))
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::{is_pride_month, should_show_panel_accent};

    #[test]
    fn is_pride_month_only_june() {
        assert!(is_pride_month(6));
        // Sample non-June months across the range.
        for m in [1, 5, 7, 12] {
            assert!(!is_pride_month(m), "month {m} must not be Pride month");
        }
    }

    #[test]
    fn panel_accent_true_only_when_all_three() {
        // The single true case.
        assert!(should_show_panel_accent(true, true, true));

        // The seven false cases — every other combination of the three flags.
        let false_cases = [
            (true, true, false),
            (true, false, true),
            (true, false, false),
            (false, true, true),
            (false, true, false),
            (false, false, true),
            (false, false, false),
        ];
        for (pride, enabled, roomy) in false_cases {
            assert!(
                !should_show_panel_accent(pride, enabled, roomy),
                "({pride}, {enabled}, {roomy}) must be false",
            );
        }
    }
}
