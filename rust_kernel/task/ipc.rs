// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! Message storage: per-task mailboxes and the kernel log ring

/// Largest message payload in bytes
pub const MSG_SIZE: usize = 64;

/// Messages a mailbox holds before senders block
pub const MAILBOX_DEPTH: usize = 4;

#[derive(Clone, Copy)]
pub struct Message {
    pub from: usize,
    pub len: usize,
    pub data: [u8; MSG_SIZE],
}

impl Message {
    const EMPTY: Message = Message { from: 0, len: 0, data: [0; MSG_SIZE] };

    /// A copy of `bytes`, cut to one message
    pub fn new(from: usize, bytes: &[u8]) -> Self {
        let len = bytes.len().min(MSG_SIZE);
        let mut msg = Message { from, len, data: [0; MSG_SIZE] };
        msg.data[..len].copy_from_slice(&bytes[..len]);
        msg
    }
}

/// Fixed-size FIFO of messages addressed to one task
pub struct Mailbox {
    msgs: [Message; MAILBOX_DEPTH],
    head: usize,
    count: usize,
}

impl Mailbox {
    pub const EMPTY: Mailbox = Mailbox { msgs: [Message::EMPTY; MAILBOX_DEPTH], head: 0, count: 0 };

    pub fn is_full(&self) -> bool {
        self.count == MAILBOX_DEPTH
    }

    /// Queue a copy of `bytes`; false if the mailbox is full or the
    /// payload is larger than a message
    pub fn push(&mut self, from: usize, bytes: &[u8]) -> bool {
        if self.is_full() || bytes.len() > MSG_SIZE {
            return false;
        }
        let msg = &mut self.msgs[(self.head + self.count) % MAILBOX_DEPTH];
        msg.from = from;
        msg.len = bytes.len();
        msg.data[..bytes.len()].copy_from_slice(bytes);
        self.count += 1;
        true
    }

    pub fn pop(&mut self) -> Option<Message> {
        if self.count == 0 {
            return None;
        }
        let msg = self.msgs[self.head];
        self.head = (self.head + 1) % MAILBOX_DEPTH;
        self.count -= 1;
        Some(msg)
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
    }
}

/// Bytes of kernel log the console server has not printed yet
pub const LOG_SIZE: usize = 1024;

/// One log line, built up before it is committed whole
pub struct Line {
    buf: [u8; 128],
    len: usize,
}

impl Line {
    pub fn new() -> Self {
        Line { buf: [0; 128], len: 0 }
    }

    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        for &c in b {
            if self.len < self.buf.len() {
                self.buf[self.len] = c;
                self.len += 1;
            }
        }
        self
    }

    pub fn dec(&mut self, mut n: u32) -> &mut Self {
        let mut digits = [0u8; 10];
        let mut i = digits.len();
        loop {
            i -= 1;
            digits[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        self.bytes(&digits[i..])
    }

    pub fn hex(&mut self, n: u32) -> &mut Self {
        let mut digits = *b"0x00000000";
        for i in 0..8 {
            let nibble = (n >> (28 - 4 * i)) & 0xF;
            digits[2 + i] = b"0123456789abcdef"[nibble as usize];
        }
        self.bytes(&digits)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

/// The console stream: kernel log lines and task writes, in the order
/// they were made. A kernel line that does not fit is dropped and counted
/// rather than truncated; a task write that does not fit blocks the task.
pub struct LogRing {
    buf: [u8; LOG_SIZE],
    head: usize,
    len: usize,
    pub dropped: u32,
}

impl LogRing {
    pub const fn new() -> Self {
        LogRing { buf: [0; LOG_SIZE], head: 0, len: 0, dropped: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Bytes that can still be pushed
    pub fn space(&self) -> usize {
        LOG_SIZE - self.len
    }

    /// Append a kernel line whole, or drop and count it if it does not fit
    pub fn commit(&mut self, line: &Line) {
        if !self.push(line.as_bytes()) {
            self.dropped += 1;
        }
    }

    /// Append `bytes` whole; false (and nothing written) if they do not fit
    pub fn push(&mut self, bytes: &[u8]) -> bool {
        if bytes.len() > self.space() {
            return false;
        }
        for &b in bytes {
            self.buf[(self.head + self.len) % LOG_SIZE] = b;
            self.len += 1;
        }
        true
    }

    /// Move up to `out.len()` bytes into `out`, returning how many
    pub fn take(&mut self, out: &mut [u8]) -> usize {
        let n = out.len().min(self.len);
        for slot in out[..n].iter_mut() {
            *slot = self.buf[self.head];
            self.head = (self.head + 1) % LOG_SIZE;
        }
        self.len -= n;
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mailbox_is_fifo_and_bounded() {
        let mut m = Mailbox::EMPTY;
        for i in 0..MAILBOX_DEPTH {
            assert!(m.push(i, &[i as u8; 3]));
        }
        assert!(m.is_full());
        assert!(!m.push(9, b"x"));
        for i in 0..MAILBOX_DEPTH {
            let msg = m.pop().unwrap();
            assert_eq!((msg.from, msg.len, msg.data[0]), (i, 3, i as u8));
        }
        assert!(m.pop().is_none());
        assert!(!m.push(0, &[0; MSG_SIZE + 1]));
    }

    #[test]
    fn log_keeps_lines_whole() {
        let mut log = LogRing::new();
        let mut line = Line::new();
        line.bytes(b"n=").dec(42).bytes(b" a=").hex(0x2000_0000).bytes(b"\n");
        log.commit(&line);

        let mut out = [0u8; 64];
        let n = log.take(&mut out[..5]);
        let m = log.take(&mut out[5..]);
        assert_eq!(&out[..n + m], b"n=42 a=0x20000000\n");

        let mut big = Line::new();
        big.bytes(&[b'x'; 100]);
        for _ in 0..LOG_SIZE / 100 {
            log.commit(&big);
        }
        log.commit(&big);
        assert_eq!(log.dropped, 1);
    }
}
