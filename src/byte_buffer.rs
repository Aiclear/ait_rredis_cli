use std::io::{Read, Write};

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

    pub fn get_slice_until(&mut self, until: &[u8]) -> &[u8] {
        // mark position if buff don't have complete data
        self.mark();

        let old_pos = self.r_pos;
        let mut bytes_count = 0;
        let mut terminator_state = 0;

        while self.has_remaining() {
            let byte = self.get_u8();
            if until[terminator_state] == byte {
                terminator_state += 1;
            } else {
                terminator_state = 0;
                bytes_count += 1;
            }

            if terminator_state == until.len() {
                break;
            }
        }

        // handle incomplete data
        if terminator_state != until.len() {
            self.reset();
        }

        self.slice(old_pos, bytes_count)
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
}
