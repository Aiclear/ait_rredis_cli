use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

pub struct CommandHistory {
    history: VecDeque<String>,
    max_size: usize,
    file_path: Option<PathBuf>,
}

impl CommandHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_size),
            max_size,
            file_path: None,
        }
    }

    pub fn with_file(max_size: usize, file_path: PathBuf) -> anyhow::Result<Self> {
        let mut history = Self::new(max_size);
        history.load_from_file(&file_path)?;
        history.file_path = Some(file_path);
        Ok(history)
    }

    pub fn add(&mut self, command: String) {
        if command.trim().is_empty() {
            return;
        }

        if self.history.len() >= self.max_size {
            self.history.pop_front();
        }
        self.history.push_back(command);

        if let Some(file_path) = &self.file_path {
            let _ = self.append_to_file(file_path);
        }
    }

    pub fn get_all(&self) -> Vec<&String> {
        self.history.iter().collect()
    }

    pub fn get_recent(&self, n: usize) -> Vec<&String> {
        let start = if self.history.len() > n {
            self.history.len() - n
        } else {
            0
        };
        self.history.iter().skip(start).collect()
    }

    pub fn clear(&mut self) {
        self.history.clear();
        if let Some(file_path) = &self.file_path {
            let _ = File::create(file_path);
        }
    }

    fn load_from_file(&mut self, file_path: &PathBuf) -> anyhow::Result<()> {
        if let Ok(file) = File::open(file_path) {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if self.history.len() >= self.max_size {
                        self.history.pop_front();
                    }
                    self.history.push_back(line);
                }
            }
        }
        Ok(())
    }

    fn append_to_file(&self, file_path: &PathBuf) -> anyhow::Result<()> {
        if let Some(last) = self.history.back() {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(file_path)?;
            writeln!(file, "{}", last)?;
        }
        Ok(())
    }
}
