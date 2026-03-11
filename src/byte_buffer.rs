use std::io::{Read, Write};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BufferError {
    #[error("Incomplete data, need more bytes")]
    Incomplete,
    #[error("Invalid data: {0}")]
    Invalid(String),
}

// need a simple and easy struc for read bytes
pub struct BytesBuffer {
    /// buffer read position
    r_pos: usize,
    /// buffer write position
    w_pos: usize,

    capacity: usize,
    mark: Option<usize>,
    bytes: Box<[u8]>,
}

impl BytesBuffer {
    pub fn new(capacity: usize) -> Self {
        BytesBuffer {
            r_pos: 0,
            w_pos: 0,
            capacity,
            mark: None,
            bytes: vec![0u8; capacity].into_boxed_slice(),
        }
    }

    pub fn read_bytes(&mut self, reader: &mut impl Read) -> anyhow::Result<usize> {
        let count = reader.read(&mut self.bytes[self.w_pos..self.capacity])?;
        self.w_pos += count;

        Ok(count)
    }

    pub fn write_bytes(&mut self, writer: &mut impl Write) -> anyhow::Result<()> {
        writer.write_all(&self.bytes[self.r_pos..self.w_pos])?;
        self.r_pos = self.w_pos;
        self.compact();
        Ok(())
    }

    pub fn mark(&mut self) {
        self.mark = Some(self.r_pos);
    }

    pub fn reset(&mut self) {
        if let Some(m_pos) = self.mark {
            self.r_pos = m_pos;
            self.mark = None;
        }
    }

    pub fn get_u8(&mut self) -> u8 {
        let byte = self.bytes[self.r_pos];
        self.r_pos += 1;
        byte
    }

    pub fn put_u8(&mut self, byte: u8) {
        self.bytes[self.w_pos] = byte;
        self.w_pos += 1;
    }

    pub fn put_u8_slice(&mut self, slice: &[u8]) {
        self.bytes[self.w_pos..self.w_pos + slice.len()].copy_from_slice(slice);
        self.w_pos += slice.len();
    }

    pub fn has_remaining(&self) -> bool {
        self.r_pos < self.w_pos
    }

    fn slice(&self, offset: usize, length: usize) -> &[u8] {
        &self.bytes[offset..offset + length]
    }

    pub fn get_slice(&mut self, length: usize) -> &[u8] {
        let old_pos = self.r_pos;
        self.r_pos += length;
        &self.bytes[old_pos..self.r_pos]
    }

    pub fn get_slice_until(&mut self, until: &[u8]) -> Option<&[u8]> {
        self.mark();

        let old_pos = self.r_pos;
        let mut i = old_pos;

        while i < self.w_pos {
            let mut found = true;
            for j in 0..until.len() {
                if i + j >= self.w_pos || self.bytes[i + j] != until[j] {
                    found = false;
                    break;
                }
            }

            if found {
                self.r_pos = i + until.len();
                return Some(self.slice(old_pos, i - old_pos));
            }

            i += 1;
        }

        // Incomplete data, reset position
        self.reset();
        None
    }

    pub fn compact(&mut self) {
        if self.r_pos == self.w_pos {
            self.r_pos = 0;
            self.w_pos = 0;
        } else {
            let bytes_count = self.w_pos - self.r_pos;
            self.bytes.copy_within(self.r_pos..self.w_pos, 0);
            self.w_pos = bytes_count;
            self.r_pos = 0;
        }
    }

    /// Get the number of remaining bytes available to read
    pub fn remaining(&self) -> usize {
        self.w_pos - self.r_pos
    }

    /// Get the current read position
    pub fn r_pos(&self) -> usize {
        self.r_pos
    }

    /// Get the current write position
    pub fn w_pos(&self) -> usize {
        self.w_pos
    }
    
    /// Reset buffer to empty state
    pub fn reset_buffer(&mut self) {
        self.r_pos = 0;
        self.w_pos = 0;
        self.mark = None;
    }
    
    /// Get available data slice from read position to write position
    pub fn get_available_data(&self) -> &[u8] {
        &self.bytes[self.r_pos..self.w_pos]
    }
    
    /// Get mutable slice for writing
    pub fn get_write_slice(&mut self) -> &mut [u8] {
        &mut self.bytes[self.w_pos..self.capacity]
    }
}
