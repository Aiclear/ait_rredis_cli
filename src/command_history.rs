use std::collections::VecDeque;

const MAX_HISTORY_SIZE: usize = 1000;

pub struct CommandHistory {
    commands: VecDeque<String>,
}

impl CommandHistory {
    pub fn new() -> Self {
        Self {
            commands: VecDeque::new(),
        }
    }

    pub fn add(&mut self, command: String) {
        if command.trim().is_empty() {
            return;
        }
        if self.commands.len() >= MAX_HISTORY_SIZE {
            self.commands.pop_front();
        }
        self.commands.push_back(command);
    }

    pub fn get_history(&self) -> &VecDeque<String> {
        &self.commands
    }

    pub fn display_history(&self) {
        if self.commands.is_empty() {
            println!("(empty history)");
            return;
        }
        for (index, cmd) in self.commands.iter().enumerate() {
            println!("{:4}  {}", index + 1, cmd);
        }
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    pub fn get_last_n(&self, n: usize) -> Vec<&String> {
        self.commands.iter().rev().take(n).collect::<Vec<_>>().into_iter().rev().collect()
    }
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}
