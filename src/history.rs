use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    path::Path,
};

pub struct CmdHistory {
    history: Vec<String>,
    max_size: usize,
}

impl CmdHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            history: Vec::new(),
            max_size,
        }
    }

    pub fn load<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        self.history.clear();
        
        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    self.history.push(line);
                }
            }
        }
        
        Ok(())
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let mut file = File::create(path)?;
        for entry in &self.history {
            writeln!(file, "{}", entry)?;
        }
        Ok(())
    }

    pub fn add(&mut self, entry: &str) {
        let entry = entry.trim();
        if entry.is_empty() || entry == "_history" || entry == "_monitor" {
            return;
        }

        if let Some(last) = self.history.last() {
            if last == entry {
                return;
            }
        }

        self.history.push(entry.to_string());
        if self.history.len() > self.max_size {
            self.history.remove(0);
        }
    }

    pub fn entries(&self) -> &[String] {
        &self.history
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }
}

pub fn format_history(history: &CmdHistory, page_size: usize) -> Vec<Vec<String>> {
    let entries = history.entries();
    let mut pages = Vec::new();
    let mut current_page = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        current_page.push(format!("{:4}: {}", i + 1, entry));
        if current_page.len() >= page_size {
            pages.push(current_page);
            current_page = Vec::new();
        }
    }

    if !current_page.is_empty() {
        pages.push(current_page);
    }

    pages
}
