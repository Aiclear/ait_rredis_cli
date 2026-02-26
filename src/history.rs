use std::collections::VecDeque;

const MAX_HISTORY_SIZE: usize = 1000;

pub struct CommandHistory {
    commands: VecDeque<String>,
}

impl CommandHistory {
    pub fn new() -> Self {
        Self {
            commands: VecDeque::with_capacity(MAX_HISTORY_SIZE),
        }
    }

    pub fn add(&mut self, command: &str) {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return;
        }

        if self.commands.back().map_or(true, |last| last != trimmed) {
            if self.commands.len() >= MAX_HISTORY_SIZE {
                self.commands.pop_front();
            }
            self.commands.push_back(trimmed.to_string());
        }
    }

    pub fn get_all(&self) -> &VecDeque<String> {
        &self.commands
    }

    pub fn display(&self) {
        if self.commands.is_empty() {
            println!("No command history.");
            return;
        }

        println!("Command History:");
        println!("{}", "-".repeat(50));
        for (idx, cmd) in self.commands.iter().enumerate() {
            println!("{:4}: {}", idx + 1, cmd);
        }
        println!("{}", "-".repeat(50));
        println!("Total: {} commands", self.commands.len());
    }

    pub fn is_history_command(input: &str) -> bool {
        input.trim().to_lowercase() == "_history"
    }
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}
