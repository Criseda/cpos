// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! ARMv7-M (PMSAv7) MPU region encoding
//!
//! Region layout while a task runs:
//!
//! | Region | Memory              | Unprivileged access     |
//! |--------|---------------------|-------------------------|
//! | 0      | Flash (code, rodata)| read + execute          |
//! | 1      | The task's own slot | read + write, no execute|
//! | 2      | UART0 registers     | read + write, console only |
//!
//! Everything else (kernel data, kernel heap, other tasks' slots, the
//! kernel stack, peripherals) has no region, so an unprivileged access
//! there is a MemManage fault. The kernel keeps the default memory map
//! through PRIVDEFENA.

/// UART0 register block on the LM3S6965
pub const UART0_BASE: u32 = 0x4000_C000;
pub const UART0_SIZE: u32 = 0x1000;

const RBAR_VALID: u32 = 1 << 4;

const RASR_ENABLE: u32 = 1;
const XN: u32 = 1 << 28;
const AP_FULL: u32 = 0b011 << 24;
const AP_READ_ONLY: u32 = 0b110 << 24;
/// Normal memory, write-back (TEX=000, C=1, B=1)
const NORMAL: u32 = (1 << 17) | (1 << 16);
/// Normal memory, write-through (TEX=000, C=1, B=0), for flash
const NORMAL_WT: u32 = 1 << 17;
/// Shared device (TEX=000, C=0, B=1, S=1)
const DEVICE: u32 = (1 << 18) | (1 << 16);

/// Region attributes for a task, ready to write to RBAR/RASR
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub rbar: u32,
    pub rasr: u32,
}

impl Region {
    pub const fn disabled(number: u32) -> Self {
        Region { rbar: RBAR_VALID | number, rasr: 0 }
    }

    /// Build a region, or `None` if `size` is not a power of two of at
    /// least 32 bytes or `base` is not aligned to it (PMSAv7 rules)
    pub const fn new(number: u32, base: u32, size: u32, attrs: u32) -> Option<Self> {
        if size < 32 || !size.is_power_of_two() || base % size != 0 || number > 7 {
            return None;
        }
        let size_field = size.trailing_zeros() - 1;
        Some(Region {
            rbar: base | RBAR_VALID | number,
            rasr: attrs | (size_field << 1) | RASR_ENABLE,
        })
    }

    pub const fn pair(self) -> (u32, u32) {
        (self.rbar, self.rasr)
    }
}

/// Region 0: all of flash, read-only and executable for everyone
pub const fn flash(size: u32) -> Option<Region> {
    Region::new(0, 0, size, AP_READ_ONLY | NORMAL_WT)
}

/// Region 1: the running task's slot
pub const fn task_slot(base: u32, size: u32) -> Option<Region> {
    Region::new(1, base, size, AP_FULL | XN | NORMAL)
}

/// Region 2: UART0, only mapped for the console server
pub const fn uart() -> Option<Region> {
    Region::new(2, UART0_BASE, UART0_SIZE, AP_FULL | XN | DEVICE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_task_slot() {
        let r = task_slot(0x2000_3800, 2048).unwrap();
        assert_eq!(r.rbar, 0x2000_3800 | 0x10 | 1);
        // SIZE field 10 means 2^(10+1) = 2048 bytes
        assert_eq!((r.rasr >> 1) & 0x1F, 10);
        assert_eq!((r.rasr >> 24) & 0b111, 0b011);
        assert_ne!(r.rasr & XN, 0);
        assert_eq!(r.rasr & 1, 1);
    }

    #[test]
    fn encodes_flash_read_only_executable() {
        let r = flash(512 * 1024).unwrap();
        assert_eq!(r.rbar, 0x10);
        assert_eq!((r.rasr >> 1) & 0x1F, 18);
        assert_eq!((r.rasr >> 24) & 0b111, 0b110);
        assert_eq!(r.rasr & XN, 0);
    }

    #[test]
    fn rejects_unaligned_or_odd_sizes() {
        assert!(task_slot(0x2000_3400, 2048).is_none());
        assert!(task_slot(0x2000_3000, 3000).is_none());
        assert!(task_slot(0x2000_3000, 16).is_none());
        assert!(uart().is_some());
    }
}
