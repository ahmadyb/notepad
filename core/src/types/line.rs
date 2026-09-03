use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all="snake_case")]
pub enum LineColour { #[default] None, Yellow, Green, Pink, Blue, Orange, Purple, Custom(u32) }
impl LineColour {
    pub const BUILT_INS:[Self;6]=[Self::Yellow,Self::Green,Self::Pink,Self::Blue,Self::Orange,Self::Purple];
    pub fn rgb(self)->Option<u32>{match self {Self::None=>None,Self::Yellow=>Some(0xFFE27A),Self::Green=>Some(0xA8E6A1),Self::Pink=>Some(0xFFB3D1),Self::Blue=>Some(0xA3D5FF),Self::Orange=>Some(0xFFC08A),Self::Purple=>Some(0xD5B3FF),Self::Custom(v)=>Some(v&0x00FF_FFFF)}}
    pub fn name(self)->String{match self{Self::None=>"None".into(),Self::Yellow=>"Yellow".into(),Self::Green=>"Green".into(),Self::Pink=>"Pink".into(),Self::Blue=>"Blue".into(),Self::Orange=>"Orange".into(),Self::Purple=>"Purple".into(),Self::Custom(v)=>format!("#{:06X}",v&0x00FF_FFFF)}}
    pub fn from_hex(s:&str)->Option<Self>{let s=s.trim().strip_prefix('#').unwrap_or(s.trim());(s.len()==6).then(||u32::from_str_radix(s,16).ok()).flatten().map(Self::Custom)}
}
impl fmt::Display for LineColour { fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{f.write_str(&self.name())} }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all="snake_case")]
pub enum ListType { #[default] None, Bullet, Number, Check }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineMetadata { pub colour:LineColour, pub list_type:ListType, pub indent:u8, pub checked:bool }
impl Default for LineMetadata { fn default()->Self{Self{colour:LineColour::None,list_type:ListType::None,indent:0,checked:false}} }
impl LineMetadata { pub const MAX_INDENT:u8=5; pub fn set_indent(&mut self,n:u8){self.indent=n.min(Self::MAX_INDENT)} pub fn is_list(&self)->bool{self.list_type!=ListType::None} }
