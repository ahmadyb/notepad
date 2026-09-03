/// Shared text metrics for the raster chrome. Cosmic Text is kept as the
/// shaping-system type for distributors that want complex-script shaping;
/// fontdue provides the small, dependency-free glyph bitmap fallback used by
/// the default renderer.
pub type FontSystem = cosmic_text::FontSystem;

#[derive(Debug,Clone,Copy)]
pub struct TextMetrics{pub size:f32,pub line_height:f32,pub advance:f32}
impl TextMetrics{pub fn monospace(size:f32)->Self{Self{size,line_height:(size*1.45).ceil(),advance:size*.62}}}
