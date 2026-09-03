pub mod extractor;
pub mod palette;
pub mod stats;
pub use extractor::extract_by_colour;
pub use palette::{highlight_rgb,palette,PaletteEntry};
pub use stats::highlight_stats;
pub use crate::types::HighlightStats;
