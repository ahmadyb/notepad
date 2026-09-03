use crate::types::LineColour;
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub struct PaletteEntry{pub colour:LineColour,pub rgb:u32,pub alpha:u8,pub outline_alpha:u8}
pub fn palette()->[PaletteEntry;6]{[LineColour::Yellow,LineColour::Green,LineColour::Pink,LineColour::Blue,LineColour::Orange,LineColour::Purple].map(|colour|PaletteEntry{colour,rgb:colour.rgb().unwrap_or(0),alpha:64,outline_alpha:128})}
pub fn highlight_rgb(c:LineColour)->Option<u32>{c.rgb()}
