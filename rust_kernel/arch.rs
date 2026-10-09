// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! Cortex-M3 system registers used by the scheduler
//!
//! Host tests replace the hardware with a recorder so the scheduler logic
//! can be checked without a CPU.

/// Number of MPU regions the kernel reprograms on every context switch
pub const SWITCHED_REGIONS: usize = 2;

#[cfg(target_arch = "arm")]
mod imp {
    use core::arch::asm;
    use core::ptr::{read_volatile, write_volatile};

    const ICSR: usize = 0xE000_ED04;
    const SHPR1: usize = 0xE000_ED18;
    const SHPR2: usize = 0xE000_ED1C;
    const SHPR3: usize = 0xE000_ED20;
    const SHCSR: usize = 0xE000_ED24;
    const CFSR: usize = 0xE000_ED28;
    const MMFAR: usize = 0xE000_ED34;
    const BFAR: usize = 0xE000_ED38;
    const SYST_CSR: usize = 0xE000_E010;
    const SYST_RVR: usize = 0xE000_E014;
    const SYST_CVR: usize = 0xE000_E018;
    const MPU_CTRL: usize = 0xE000_ED94;
    const MPU_RBAR: usize = 0xE000_ED9C;
    const MPU_RASR: usize = 0xE000_EDA0;

    const PENDSVSET: u32 = 1 << 28;

    fn write(addr: usize, value: u32) {
        // SAFETY: every address passed here is a System Control Space
        // register that exists on all ARMv7-M parts, and only privileged
        // kernel code calls into this module.
        unsafe { write_volatile(addr as *mut u32, value) }
    }

    fn read(addr: usize) -> u32 {
        // SAFETY: as in `write`
        unsafe { read_volatile(addr as *const u32) }
    }

    pub fn pend_switch() {
        write(ICSR, PENDSVSET);
    }

    pub fn set_psp(sp: usize) {
        // SAFETY: only called from handler mode, where the CPU runs on MSP,
        // so changing PSP cannot pull the stack out from under us.
        unsafe { asm!("msr psp, {}", in(reg) sp, options(nomem, nostack)) }
    }

    pub fn disable_irq() {
        // SAFETY: masking interrupts has no memory-safety effect
        unsafe { asm!("cpsid i", options(nomem, nostack)) }
    }

    pub fn enable_irq() {
        // SAFETY: as above
        unsafe { asm!("cpsie i", options(nomem, nostack)) }
    }

    /// Exception priorities: faults, SVC and SysTick share one level so
    /// they never preempt each other while holding kernel state; PendSV
    /// sits below them all.
    pub fn set_priorities() {
        write(SHPR1, 0x0080_8080); // MemManage, BusFault, UsageFault
        write(SHPR2, 0x8000_0000); // SVCall
        write(SHPR3, 0x80E0_0000); // SysTick 0x80, PendSV 0xE0
    }

    /// Route MemManage, BusFault and UsageFault to their own handlers
    /// instead of escalating to HardFault
    pub fn enable_fault_handlers() {
        write(SHCSR, read(SHCSR) | (0b111 << 16));
    }

    pub fn start_systick(reload: u32) {
        write(SYST_RVR, reload - 1);
        write(SYST_CVR, 0);
        write(SYST_CSR, 0b111); // processor clock, interrupt, enable
    }

    /// Fault status and the faulting address, if the hardware latched one.
    /// Clears the status bits so the next fault starts fresh.
    pub fn take_fault_status() -> (u32, Option<usize>) {
        let cfsr = read(CFSR);
        let addr = if cfsr & (1 << 7) != 0 {
            Some(read(MMFAR) as usize) // MMARVALID
        } else if cfsr & (1 << 15) != 0 {
            Some(read(BFAR) as usize) // BFARVALID
        } else {
            None
        };
        write(CFSR, cfsr);
        (cfsr, addr)
    }

    /// Program the regions that stay the same for every task and turn the
    /// MPU on. PRIVDEFENA keeps the default map for the kernel.
    pub fn mpu_enable(fixed: &[(u32, u32)]) {
        for &(rbar, rasr) in fixed {
            write(MPU_RBAR, rbar);
            write(MPU_RASR, rasr);
        }
        write(MPU_CTRL, 0b101); // ENABLE | PRIVDEFENA
        barrier();
    }

    pub fn mpu_switch(regions: &[(u32, u32); super::SWITCHED_REGIONS]) {
        for &(rbar, rasr) in regions {
            write(MPU_RBAR, rbar);
            write(MPU_RASR, rasr);
        }
        barrier();
    }

    fn barrier() {
        // SAFETY: barriers only order memory accesses
        unsafe { asm!("dsb", "isb", options(nostack)) }
    }
}

#[cfg(not(target_arch = "arm"))]
pub(crate) mod imp {
    extern crate std;

    use std::cell::{Cell, RefCell};

    std::thread_local! {
        pub static SWITCH_PENDING: Cell<bool> = Cell::new(false);
        pub static PSP: Cell<usize> = Cell::new(0);
        pub static MPU: RefCell<[(u32, u32); super::SWITCHED_REGIONS]> =
            RefCell::new([(0, 0); super::SWITCHED_REGIONS]);
    }

    pub fn pend_switch() {
        SWITCH_PENDING.with(|p| p.set(true));
    }
    pub fn set_psp(sp: usize) {
        PSP.with(|p| p.set(sp));
    }
    pub fn disable_irq() {}
    pub fn enable_irq() {}
    pub fn set_priorities() {}
    pub fn enable_fault_handlers() {}
    pub fn start_systick(_reload: u32) {}
    pub fn take_fault_status() -> (u32, Option<usize>) {
        (0, None)
    }
    pub fn mpu_enable(_fixed: &[(u32, u32)]) {}
    pub fn mpu_switch(regions: &[(u32, u32); super::SWITCHED_REGIONS]) {
        MPU.with(|m| *m.borrow_mut() = *regions);
    }
}

pub use imp::*;
