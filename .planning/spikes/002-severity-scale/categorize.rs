// Spike 002 reference implementation — categorization of grains/m³ readings
// using the European Aeroallergen Network (EAN) scale that CAMS collaborates
// with. Not wired into the build; this is the proposed pattern for landing
// in `src/weather.rs` later. Compile in isolation with `rustc categorize.rs`
// to run the asserts.

/// Pollen severity bucket, EAN-aligned.
///
/// `OffSeason` is added on top of the four EAN tiers to preserve the
/// semantic distinction surfaced by Spike 001: weathervane returns 0.0
/// for "this species is not actively producing pollen here right now"
/// (off-season or not regionally present), which is meaningfully
/// different from "Low" (1-5 grains/m³ for grass). Treating 0.0 as
/// "Low" would surface every species year-round and bury the signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollenLevel {
    OffSeason,
    Low,
    Moderate,
    High,
    VeryHigh,
}

/// Which threshold family a species uses. Trees, grasses/weeds, and
/// olive each have different cutoffs because they shed at different
/// volumes — a "High" grass count is 50 grains/m³, but a "High" birch
/// count is 1000.
#[derive(Debug, Clone, Copy)]
enum Scale {
    Tree,      // alder, birch
    GrassWeed, // grass, mugwort, ragweed
    Olive,     // olive
}

impl Scale {
    /// Categorize a non-negative grains/m³ reading. Caller is responsible
    /// for routing 0.0 to `OffSeason` separately — this function assumes
    /// a positive value.
    fn categorize(self, grains: f32) -> PollenLevel {
        match self {
            // EAN tree thresholds: ≤10 Low, ≤100 Moderate, ≤1000 High, else Very High
            Scale::Tree => match grains {
                g if g <= 10.0 => PollenLevel::Low,
                g if g <= 100.0 => PollenLevel::Moderate,
                g if g <= 1000.0 => PollenLevel::High,
                _ => PollenLevel::VeryHigh,
            },
            // EAN grass/weed: ≤5 Low, ≤20 Moderate, ≤50 High, else Very High
            Scale::GrassWeed => match grains {
                g if g <= 5.0 => PollenLevel::Low,
                g if g <= 20.0 => PollenLevel::Moderate,
                g if g <= 50.0 => PollenLevel::High,
                _ => PollenLevel::VeryHigh,
            },
            // EAN olive: ≤10 Low, ≤50 Moderate, ≤200 High, else Very High
            Scale::Olive => match grains {
                g if g <= 10.0 => PollenLevel::Low,
                g if g <= 50.0 => PollenLevel::Moderate,
                g if g <= 200.0 => PollenLevel::High,
                _ => PollenLevel::VeryHigh,
            },
        }
    }
}

/// Species the API returns. Mirrors `weathervane::PollenData` fields.
#[derive(Debug, Clone, Copy)]
pub enum Species {
    Alder,
    Birch,
    Grass,
    Mugwort,
    Olive,
    Ragweed,
}

impl Species {
    fn scale(self) -> Scale {
        match self {
            Species::Alder | Species::Birch => Scale::Tree,
            Species::Grass | Species::Mugwort | Species::Ragweed => Scale::GrassWeed,
            Species::Olive => Scale::Olive,
        }
    }
}

/// Categorize a reading for a given species. 0.0 collapses to OffSeason
/// per the Spike 001 finding that weathervane uses 0.0 to mean "not
/// actively producing," distinct from "Low."
pub fn categorize(species: Species, grains: f32) -> PollenLevel {
    if grains == 0.0 {
        return PollenLevel::OffSeason;
    }
    species.scale().categorize(grains)
}

fn main() {
    // Spike 001 fixtures, categorized:
    let cases = [
        (Species::Grass, 19.1, "Rome", PollenLevel::Moderate), // just under the 20 threshold
        (Species::Grass, 0.7, "Berlin", PollenLevel::Low),
        (Species::Grass, 0.2, "Paris", PollenLevel::Low),
        (Species::Olive, 0.6, "Rome", PollenLevel::Low),
        (Species::Birch, 0.0, "Berlin", PollenLevel::OffSeason),
        // Synthetic boundary checks — pulled from the EAN scale doc:
        (
            Species::Birch,
            10.0,
            "boundary tree Low/Mod",
            PollenLevel::Low,
        ),
        (
            Species::Birch,
            10.01,
            "boundary tree Low/Mod",
            PollenLevel::Moderate,
        ),
        (
            Species::Birch,
            1000.0,
            "boundary tree High/VeryH",
            PollenLevel::High,
        ),
        (
            Species::Birch,
            1000.1,
            "boundary tree High/VeryH",
            PollenLevel::VeryHigh,
        ),
        (
            Species::Grass,
            5.0,
            "boundary grass Low/Mod",
            PollenLevel::Low,
        ),
        (
            Species::Grass,
            50.1,
            "boundary grass High/VeryH",
            PollenLevel::VeryHigh,
        ),
        (
            Species::Olive,
            200.0,
            "boundary olive High/VeryH",
            PollenLevel::High,
        ),
        (
            Species::Olive,
            200.1,
            "boundary olive High/VeryH",
            PollenLevel::VeryHigh,
        ),
    ];

    for (species, grains, label, expected) in cases {
        let got = categorize(species, grains);
        assert_eq!(
            got, expected,
            "{label}: {species:?}@{grains} -> {got:?}, want {expected:?}"
        );
        println!("ok  {species:?} @ {grains:>6.1} grains/m³  ->  {got:?}   ({label})");
    }
}
