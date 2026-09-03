use super::ffi::{self, SciFnDirect, Sptr};
use std::ffi::{CStr, CString};

#[derive(Debug)]
pub struct DirectEditor {
    function: SciFnDirect,
    pointer: Sptr,
}

impl DirectEditor {
    /// Construct a direct-call wrapper from Scintilla's function and object pointers.
    ///
    /// # Safety
    /// The function pointer must be a valid `SciFnDirect` for the supplied Scintilla
    /// object pointer, and both pointers must remain valid for the lifetime of the wrapper.
    pub unsafe fn from_raw(function: Sptr, pointer: Sptr) -> Option<Self> {
        if function == 0 || pointer == 0 {
            return None;
        }
        Some(Self {
            function: unsafe { std::mem::transmute::<Sptr, SciFnDirect>(function) },
            pointer,
        })
    }

    pub fn send(&self, message: u32, wparam: usize, lparam: Sptr) -> Sptr {
        unsafe { (self.function)(self.pointer, message, wparam, lparam) }
    }

    pub fn set_text(&self, text: &str) {
        let text = c_string(text);
        self.send(ffi::SCI_SETTEXT, 0, text.as_ptr() as Sptr);
    }

    pub fn get_text(&self) -> String {
        let length = self.send(ffi::SCI_GETLENGTH, 0, 0).max(0) as usize;
        let mut buffer = vec![0_u8; length.saturating_add(1)];
        let read = self
            .send(ffi::SCI_GETTEXT, buffer.len(), buffer.as_mut_ptr() as Sptr)
            .max(0) as usize;
        buffer.truncate(read.min(buffer.len()));
        CStr::from_bytes_until_nul(&buffer).map_or_else(
            |_| String::from_utf8_lossy(&buffer).into_owned(),
            |value| value.to_string_lossy().into_owned(),
        )
    }

    pub fn set_colours(
        &self,
        foreground: (u8, u8, u8),
        background: (u8, u8, u8),
        caret: (u8, u8, u8),
    ) {
        let foreground = pack(foreground);
        let background = pack(background);
        let caret = pack(caret);
        // Scintilla's default style is style 32.  Setting style 0 alone only
        // changes the base style before a lexer is applied and leaves the native
        // child with its white background/black text defaults.  Clear all styles
        // from an explicitly configured default instead.
        self.send(ffi::SCI_STYLESETFORE, ffi::STYLE_DEFAULT as usize, foreground);
        self.send(ffi::SCI_STYLESETBACK, ffi::STYLE_DEFAULT as usize, background);
        self.send(ffi::SCI_STYLECLEARALL, 0, 0);
        self.send(ffi::SCI_SETCARETFORE, 0, caret);
        self.send(ffi::SCI_SETCARETWIDTH, 2, 0);
        self.send(ffi::SCI_SETSELFORE, 0, foreground);
        self.send(ffi::SCI_SETSELBACK, 1, pack((80, 92, 180)));
        self.send(ffi::SCI_SETBUFFEREDDRAW, 1, 0);
        self.send(ffi::SCI_SETREADONLY, 0, 0);
        // The rasterized shell owns line numbers and margins.  Leaving the
        // default symbol margin enabled is the source of the distracting black
        // strip at the left edge of the editor.
        for margin in 0..5 {
            self.send(ffi::SCI_SETMARGINWIDTHN, margin, 0);
            self.send(ffi::SCI_SETMARGINTYPEN, margin, 0);
        }
        self.send(ffi::SCI_SETMARGINLEFT, 0, 0);
        self.send(ffi::SCI_SETMARGINRIGHT, 0, 0);
    }

    pub fn set_font(&self, family: &str, size: f32) {
        let family = c_string(family);
        self.send(
            ffi::SCI_STYLESETFONT,
            ffi::STYLE_DEFAULT as usize,
            family.as_ptr() as Sptr,
        );
        self.send(
            ffi::SCI_STYLESETSIZE,
            ffi::STYLE_DEFAULT as usize,
            size.round().clamp(8.0, 96.0) as Sptr,
        );
        self.send(ffi::SCI_STYLECLEARALL, 0, 0);
    }

    pub fn set_word_wrap(&self, enabled: bool) {
        self.send(
            ffi::SCI_SETWRAPMODE,
            if enabled {
                ffi::SC_WRAP_WORD as usize
            } else {
                ffi::SC_WRAP_NONE as usize
            },
            0,
        );
    }

    pub fn get_current_pos(&self) -> usize {
        self.send(ffi::SCI_GETCURRENTPOS, 0, 0).max(0) as usize
    }

    pub fn get_anchor(&self) -> usize {
        self.send(ffi::SCI_GETANCHOR, 0, 0).max(0) as usize
    }

    pub fn set_selection(&self, anchor: usize, caret: usize) {
        self.send(ffi::SCI_SETSEL, anchor, caret as Sptr);
    }

    pub fn replace_selection(&self, text: &str) {
        let text = c_string(text);
        self.send(ffi::SCI_REPLACESEL, 0, text.as_ptr() as Sptr);
    }

    pub fn insert_text(&self, position: usize, text: &str) {
        let text = c_string(text);
        self.send(
            ffi::SCI_INSERTTEXT,
            position,
            text.as_ptr() as Sptr,
        );
    }

    pub fn begin_undo_action(&self) {
        self.send(ffi::SCI_BEGINUNDOACTION, 0, 0);
    }

    pub fn end_undo_action(&self) {
        self.send(ffi::SCI_ENDUNDOACTION, 0, 0);
    }

    pub fn undo(&self) {
        self.send(ffi::SCI_UNDO, 0, 0);
    }

    pub fn redo(&self) {
        self.send(ffi::SCI_REDO, 0, 0);
    }

    pub fn line_start(&self, line: usize) -> usize {
        self.send(ffi::SCI_POSITIONFROMLINE, line, 0).max(0) as usize
    }

    pub fn line_length(&self, line: usize) -> usize {
        self.send(ffi::SCI_LINELENGTH, line, 0).max(0) as usize
    }

    pub fn configure_indicator(&self, slot: usize, colour: (u8, u8, u8), alpha: u8) {
        self.send(ffi::SCI_INDICSETSTYLE, slot, ffi::INDIC_FULLBOX as Sptr);
        self.send(ffi::SCI_INDICSETFORE, slot, pack(colour));
        self.send(ffi::SCI_INDICSETALPHA, slot, alpha as Sptr);
    }

    pub fn clear_indicator(&self, slot: usize, start: usize, length: usize) {
        self.send(ffi::SCI_SETINDICATORCURRENT, slot, 0);
        self.send(ffi::SCI_INDICATORCLEARRANGE, length, start as Sptr);
    }

    pub fn set_indicator_range(&self, slot: usize, start: usize, length: usize) {
        self.send(ffi::SCI_SETINDICATORCURRENT, slot, 0);
        self.send(ffi::SCI_INDICATORFILLRANGE, length, start as Sptr);
    }
}

fn c_string(value: &str) -> CString {
    CString::new(value.replace('\0', "�")).expect("replacement removed NUL bytes")
}

fn pack(colour: (u8, u8, u8)) -> Sptr {
    (u32::from(colour.0) | u32::from(colour.1) << 8 | u32::from(colour.2) << 16) as Sptr
}
