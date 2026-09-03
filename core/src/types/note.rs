use super::{LineColour,LineMetadata};
use serde::{Deserialize,Serialize};
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
pub struct Note { pub id:i64,pub title:String,pub content:String,pub highlights:Vec<LineColour>,pub list_structure:Vec<LineMetadata>,pub pinned:bool,pub created_at:i64,pub modified_at:i64 }
impl Note { pub fn new(title:impl Into<String>,content:impl Into<String>)->Self{let content=content.into();let n=content.split('\n').count().max(1);Self{id:0,title:title.into(),content,highlights:vec![LineColour::None;n],list_structure:vec![LineMetadata::default();n],pinned:false,created_at:0,modified_at:0}} }
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
pub struct NoteData { pub id:i64,pub title:String,pub content:String,#[serde(default)]pub highlights:Vec<LineColour>,#[serde(default)]pub list_structure:Vec<LineMetadata>,#[serde(default)]pub pinned:bool }
impl Default for NoteData {fn default()->Self{Self{id:0,title:String::new(),content:String::new(),highlights:Vec::new(),list_structure:Vec::new(),pinned:false}}}
impl From<Note> for NoteData {fn from(n:Note)->Self{Self{id:n.id,title:n.title,content:n.content,highlights:n.highlights,list_structure:n.list_structure,pinned:n.pinned}}}
