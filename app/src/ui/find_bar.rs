use notepad_core::{find_all, FindMatch, FindOptions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindField {
    Query,
    Replacement,
}

#[derive(Debug, Clone)]
pub struct FindBarState {
    pub open: bool,
    pub show_replace: bool,
    pub query: String,
    pub replacement: String,
    pub options: FindOptions,
    pub matches: Vec<FindMatch>,
    pub current: usize,
    pub focus: FindField,
}

impl Default for FindBarState {
    fn default() -> Self {
        Self {
            open: false,
            show_replace: false,
            query: String::new(),
            replacement: String::new(),
            options: FindOptions::default(),
            matches: Vec::new(),
            current: 0,
            focus: FindField::Query,
        }
    }
}

impl FindBarState {
    pub fn refresh(&mut self, text: &str) -> Result<(), String> {
        self.matches = find_all(text, &self.query, &self.options).map_err(|error| error.to_string())?;
        if self.matches.is_empty() {
            self.current = 0;
        } else {
            self.current = self.current.min(self.matches.len() - 1);
        }
        Ok(())
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.replacement.clear();
        self.matches.clear();
        self.current = 0;
    }

    pub fn next(&mut self) -> Option<&FindMatch> {
        if self.matches.is_empty() {
            return None;
        }
        self.current = (self.current + 1) % self.matches.len();
        self.matches.get(self.current)
    }

    pub fn previous(&mut self) -> Option<&FindMatch> {
        if self.matches.is_empty() {
            return None;
        }
        self.current = if self.current == 0 {
            self.matches.len() - 1
        } else {
            self.current - 1
        };
        self.matches.get(self.current)
    }

    pub fn current_match(&self) -> Option<&FindMatch> {
        self.matches.get(self.current)
    }

    pub fn counter(&self) -> String {
        if self.matches.is_empty() {
            if self.query.is_empty() {
                "Find in document".into()
            } else {
                "0 matches".into()
            }
        } else {
            format!("{} of {}", self.current + 1, self.matches.len())
        }
    }
}
