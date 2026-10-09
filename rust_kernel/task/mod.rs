// SPDX-License-Identifier: MIT
// Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
//
// Author: Laurentiu Cristian Preda (criseda)
// GitHub: https://github.com/criseda
//
//! Tasks, preemptive scheduling and message passing
//!
//! Every task owns one fixed-size, size-aligned RAM slot: a private heap in
//! the low part and its stack in the high part. Tasks run unprivileged on
//! the process stack (PSP), and the MPU only lets a task touch its own slot
//! and flash. The kernel switches tasks in PendSV, driven by SysTick
//! (round robin, one tick per slice) and by blocking system calls.
//!
//! A system call that has to wait (receive on an empty mailbox, send to a
//! full one) rewinds the task's PC to the `svc` instruction and blocks the
//! task; when it is woken the call simply runs again. That keeps the kernel
//! free of saved continuation state.

pub mod ipc;
pub mod mpu;

use crate::arch;
use crate::memory::LinkedListAllocator;
use crate::syscall::numbers::*;
use crate::uart;
use ipc::{Line, LogRing, Mailbox, Message, MSG_SIZE};
use spin::Mutex;

pub const MAX_TASKS: usize = 8;
pub const MAX_PENDING: usize = 24;
/// Bytes of RAM per task: heap below, stack guard, stack above
pub const SLOT_SIZE: usize = 2048;
pub const TASK_HEAP_SIZE: usize = 1024;
/// No-access gap between a task's heap and the bottom of its stack
pub const STACK_GUARD: usize = mpu::STACK_GUARD_SIZE as usize;
/// CFSR.MSTKERR: the MPU stopped exception entry pushing onto the stack
const CFSR_MSTKERR: u32 = 1 << 4;
pub const TICK_MS: usize = 10;
/// File descriptor of console input
const STDIN: usize = 0;
/// Sender id the console sees for kernel log messages
pub const KERNEL_SENDER: usize = 0;
/// Sender id of the "UART input arrived" notification to the console
pub const IRQ_SENDER: usize = u32::MAX as usize;

/// Runs when nothing else is ready; never counted as a test task
pub const TASK_IDLE: u32 = 1 << 0;
/// The console server: owns the UART and drains the kernel log
pub const TASK_CONSOLE: u32 = 1 << 1;
/// The task exists to prove a fault is caught; exiting normally is a failure
pub const TASK_EXPECT_FAULT: u32 = 1 << 2;
/// A long-running service; not waited for when counting finished tests
pub const TASK_SERVICE: u32 = 1 << 3;

/// Thumb bit set in the initial xPSR
const INITIAL_XPSR: usize = 1 << 24;
/// r4-r11 saved by PendSV plus the 8-word hardware exception frame
const INITIAL_FRAME_WORDS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Free,
    Ready,
    Sleeping { wake: u32 },
    /// Waiting for a message
    Receiving,
    /// Waiting for space in `dest`'s mailbox
    Sending { dest: usize },
}

/// What the SVC handler does with the calling task after a system call
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Write the value to the caller's r0 and resume it
    Return(isize),
    /// The caller is blocked; re-run its `svc` when it is woken
    Retry,
}

impl Outcome {
    pub fn error(code: usize) -> Self {
        Outcome::Return(-(code as isize))
    }
}

struct Task {
    state: State,
    id: usize,
    sp: usize,
    name: &'static [u8],
    flags: u32,
    mailbox: Mailbox,
    /// SYS_READ asked the console for a line and is waiting for it
    reading: bool,
    /// The console's answer to that request, kept apart from the mailbox
    /// so a full mailbox can never block the console
    input: Option<Message>,
    heap: LinkedListAllocator,
}

impl Task {
    const EMPTY: Task = Task {
        state: State::Free,
        id: 0,
        sp: 0,
        name: b"",
        flags: 0,
        mailbox: Mailbox::EMPTY,
        reading: false,
        input: None,
        heap: LinkedListAllocator::new(),
    };
}

#[derive(Clone, Copy)]
struct Spawn {
    id: usize,
    name: &'static [u8],
    entry: usize,
    arg: usize,
    flags: u32,
}

/// Addresses the kernel needs from the linker and the C side
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Start of the task slot area; must be aligned to `SLOT_SIZE`
    pub slot_base: usize,
    /// Flash a task may hand to the kernel as a read-only buffer
    pub flash_start: usize,
    pub flash_end: usize,
    /// Where a task's entry function returns to (calls SYS_EXIT)
    pub exit_trampoline: usize,
}

pub struct Kernel {
    tasks: [Task; MAX_TASKS],
    pending: [Option<Spawn>; MAX_PENDING],
    current: Option<usize>,
    /// A task that just woke from sleep; it runs at the next switch
    boosted: Option<usize>,
    started: bool,
    finished: bool,
    ticks: u32,
    next_id: usize,
    cfg: Config,
    log: LogRing,
    exited: u32,
    faulted: u32,
    /// The UART interrupt fired and the console has not been told yet
    input_pending: bool,
    /// The UART interrupt is masked until the console has read the input
    input_masked: bool,
}

pub static KERNEL: Mutex<Kernel> = Mutex::new(Kernel::new());

/// Throwaway stack PendSV can save registers to when the outgoing task no
/// longer exists (first switch, exit, fault)
#[cfg(target_arch = "arm")]
static mut SCRATCH_STACK: [u64; 8] = [0; 8];

fn scratch_stack_top() -> usize {
    #[cfg(target_arch = "arm")]
    {
        // Only the address is taken; PendSV is the only writer and the
        // contents are never read
        core::ptr::addr_of_mut!(SCRATCH_STACK) as usize + 64
    }
    #[cfg(not(target_arch = "arm"))]
    {
        0
    }
}

/// `[addr, addr + len)` lies within `[start, end)`, without overflow
fn in_range(addr: usize, len: usize, start: usize, end: usize) -> bool {
    start <= addr && addr <= end && len <= end - addr
}

impl Kernel {
    pub const fn new() -> Self {
        Kernel {
            tasks: [Task::EMPTY; MAX_TASKS],
            pending: [None; MAX_PENDING],
            current: None,
            boosted: None,
            started: false,
            finished: false,
            ticks: 0,
            next_id: 1,
            cfg: Config { slot_base: 0, flash_start: 0, flash_end: 0, exit_trampoline: 0 },
            log: LogRing::new(),
            exited: 0,
            faulted: 0,
            input_pending: false,
            input_masked: false,
        }
    }

    pub fn started(&self) -> bool {
        self.started
    }

    pub fn ticks(&self) -> u32 {
        self.ticks
    }

    pub fn current(&self) -> Option<usize> {
        self.current
    }

    /// The idle task must always be runnable, so it may not block or exit
    pub fn is_idle(&self, slot: usize) -> bool {
        self.tasks[slot].flags & TASK_IDLE != 0
    }

    /// Queue a task to run once the scheduler starts, or as soon as a slot
    /// frees up. Returns the id it will have, or `None` if the queue is full.
    pub fn register(&mut self, name: &'static [u8], entry: usize, arg: usize, flags: u32) -> Option<usize> {
        let free = self.pending.iter_mut().find(|p| p.is_none())?;
        let id = self.next_id;
        *free = Some(Spawn { id, name, entry, arg, flags });
        self.next_id += 1;
        Some(id)
    }

    /// Spawn the first batch of tasks. Fails if the slot area is misaligned
    /// or there is no idle task to fall back on.
    pub fn start(&mut self, cfg: Config) -> Result<(), &'static str> {
        if cfg.slot_base % SLOT_SIZE != 0 {
            return Err("task slots misaligned");
        }
        let has_idle = self.pending.iter().flatten().take(MAX_TASKS).any(|s| s.flags & TASK_IDLE != 0);
        if !has_idle {
            return Err("no idle task among the first tasks");
        }
        self.cfg = cfg;
        self.started = true;
        self.spawn_pending();
        Ok(())
    }

    fn slot_range(&self, slot: usize) -> (usize, usize) {
        let start = self.cfg.slot_base + slot * SLOT_SIZE;
        (start, start + SLOT_SIZE)
    }

    /// The stack guard of `slot`: just above the heap
    fn guard_range(&self, slot: usize) -> (usize, usize) {
        let guard = self.slot_range(slot).0 + TASK_HEAP_SIZE;
        (guard, guard + STACK_GUARD)
    }

    /// `[addr, addr + len)` is in the task's slot and clear of its guard
    fn in_own_slot(&self, slot: usize, addr: usize, len: usize) -> bool {
        let (start, end) = self.slot_range(slot);
        let (guard, guard_end) = self.guard_range(slot);
        in_range(addr, len, start, guard) || in_range(addr, len, guard_end, end)
    }

    /// Move queued tasks into free slots, oldest first
    fn spawn_pending(&mut self) {
        while let Some(spawn) = self.pending[0] {
            let slot = match self.tasks.iter().position(|t| t.state == State::Free) {
                Some(s) => s,
                None => return,
            };
            self.pending.copy_within(1.., 0);
            self.pending[MAX_PENDING - 1] = None;
            self.spawn(slot, spawn);
        }
    }

    fn spawn(&mut self, slot: usize, s: Spawn) {
        let (start, end) = self.slot_range(slot);
        let sp = end - INITIAL_FRAME_WORDS * 4;
        let mut frame = [0usize; INITIAL_FRAME_WORDS];
        // frame[0..8] is r4-r11; the hardware frame follows
        frame[8] = s.arg; // r0
        frame[13] = self.cfg.exit_trampoline | 1; // lr
        frame[14] = s.entry & !1; // pc
        frame[15] = INITIAL_XPSR;

        let task = &mut self.tasks[slot];
        // SAFETY: the slot is free, so nothing else uses its memory, and the
        // layout reserves `[start, end)` for task slots. The frame sits at
        // the top of the slot, above the heap, and `sp` is 8-byte aligned.
        unsafe {
            for (i, word) in frame.iter().enumerate() {
                ((sp + i * 4) as *mut u32).write(*word as u32);
            }
            task.heap.init(start, TASK_HEAP_SIZE);
        }
        task.state = State::Ready;
        task.id = s.id;
        task.sp = sp;
        task.name = s.name;
        task.flags = s.flags;
        task.mailbox.clear();
        task.reading = false;
        task.input = None;
    }

    /// Slot of the live task with this id
    fn find(&self, id: usize) -> Option<usize> {
        self.tasks.iter().position(|t| t.state != State::Free && t.id == id && id != 0)
    }

    fn console(&self) -> Option<usize> {
        self.tasks.iter().position(|t| t.state != State::Free && t.flags & TASK_CONSOLE != 0)
    }

    /// MPU regions for the task in `slot`
    fn regions(&self, slot: usize) -> [(u32, u32); arch::SWITCHED_REGIONS] {
        let (start, _) = self.slot_range(slot);
        let own = mpu::task_slot(start as u32, SLOT_SIZE as u32).unwrap_or(mpu::Region::disabled(1));
        let uart = if self.tasks[slot].flags & TASK_CONSOLE != 0 {
            mpu::uart().unwrap_or(mpu::Region::disabled(2))
        } else {
            mpu::Region::disabled(2)
        };
        let guard = mpu::stack_guard(self.guard_range(slot).0 as u32).unwrap_or(mpu::Region::disabled(3));
        [own.pair(), uart.pair(), guard.pair()]
    }

    /// PendSV: save the outgoing task's stack pointer, pick the next task,
    /// map its memory, and return its stack pointer
    pub fn switch(&mut self, saved_sp: usize) -> usize {
        if let Some(cur) = self.current {
            self.tasks[cur].sp = saved_sp;
        }
        let next = self.pick_next();
        if self.boosted == Some(next) {
            self.boosted = None;
        }
        self.current = Some(next);
        arch::mpu_switch(&self.regions(next));
        self.tasks[next].sp
    }

    fn pick_next(&self) -> usize {
        if let Some(b) = self.boosted {
            if self.tasks[b].state == State::Ready {
                return b;
            }
        }
        let start = self.current.map_or(0, |c| c + 1);
        for i in 0..MAX_TASKS {
            let slot = (start + i) % MAX_TASKS;
            let t = &self.tasks[slot];
            if t.state == State::Ready && t.flags & TASK_IDLE == 0 {
                return slot;
            }
        }
        // `start` refuses to run without an idle task, and idle never exits
        self.tasks
            .iter()
            .position(|t| t.state == State::Ready && t.flags & TASK_IDLE != 0)
            .expect("idle task missing")
    }

    /// SysTick: advance time, wake sleepers, and end the current slice
    pub fn tick(&mut self) {
        self.ticks = self.ticks.wrapping_add(1);
        let now = self.ticks;
        for (slot, t) in self.tasks.iter_mut().enumerate() {
            if let State::Sleeping { wake } = t.state {
                if now.wrapping_sub(wake) as i32 >= 0 {
                    t.state = State::Ready;
                    self.boosted = Some(slot);
                }
            }
        }
        if self.started {
            arch::pend_switch();
        }
    }

    // ---- user memory checks -------------------------------------------

    /// The calling task may read `[addr, addr + len)`: its own slot (not
    /// the stack guard) or flash
    pub fn check_readable(&self, slot: usize, addr: usize, len: usize) -> Result<(), usize> {
        if self.in_own_slot(slot, addr, len) || in_range(addr, len, self.cfg.flash_start, self.cfg.flash_end) {
            Ok(())
        } else {
            Err(ERROR_BAD_ADDRESS)
        }
    }

    /// The calling task may write `[addr, addr + len)`: its own slot only,
    /// not the stack guard
    pub fn check_writable(&self, slot: usize, addr: usize, len: usize) -> Result<(), usize> {
        if self.in_own_slot(slot, addr, len) {
            Ok(())
        } else {
            Err(ERROR_BAD_ADDRESS)
        }
    }

    // ---- system calls made by tasks -------------------------------------

    pub fn sys_yield(&mut self) -> Outcome {
        arch::pend_switch();
        Outcome::Return(0)
    }

    pub fn sys_sleep(&mut self, slot: usize, ms: usize) -> Outcome {
        let ticks = ms.div_ceil(TICK_MS).min(i32::MAX as usize) as u32;
        if ticks == 0 {
            return self.sys_yield();
        }
        self.tasks[slot].state = State::Sleeping { wake: self.ticks.wrapping_add(ticks) };
        arch::pend_switch();
        Outcome::Return(0)
    }

    pub fn sys_exit(&mut self, slot: usize, code: usize) -> Outcome {
        let t = &self.tasks[slot];
        let mut line = Line::new();
        if t.flags & TASK_EXPECT_FAULT != 0 {
            line.bytes(b"  - ").bytes(t.name).bytes(b" exited instead of faulting: FAILED\n");
            self.log_line(&line);
        } else if code != 0 {
            line.bytes(b"  - ").bytes(t.name).bytes(b" exited with code ").dec(code as u32).bytes(b": FAILED\n");
            self.log_line(&line);
        }
        self.exited += 1;
        self.retire(slot);
        Outcome::Return(0)
    }

    pub fn sys_send(&mut self, slot: usize, dest_id: usize, buf: usize, len: usize) -> Result<Outcome, usize> {
        if len > MSG_SIZE {
            return Err(ERROR_INVALID_ARGUMENT);
        }
        self.check_readable(slot, buf, len)?;
        let dest = self.find(dest_id).ok_or(ERROR_NO_TASK)?;
        let console = self.console();
        if len == 0 && console == Some(dest) {
            // An empty message to the console means "read request"
            return Err(ERROR_INVALID_ARGUMENT);
        }
        if console == Some(slot) && self.tasks[dest].reading {
            // The console answering a SYS_READ: straight into the reader
            // SAFETY: `check_readable` proved the range is the console's
            // own slot or flash, both valid memory for the whole call.
            let bytes = unsafe { core::slice::from_raw_parts(buf as *const u8, len) };
            let msg = Message::new(self.tasks[slot].id, bytes);
            let reader = &mut self.tasks[dest];
            reader.input = Some(msg);
            reader.reading = false;
            if reader.state == State::Receiving {
                reader.state = State::Ready;
            }
            return Ok(Outcome::Return(len as isize));
        }
        if self.tasks[dest].mailbox.is_full() {
            self.tasks[slot].state = State::Sending { dest };
            arch::pend_switch();
            return Ok(Outcome::Retry);
        }
        let bytes: &[u8] = if len == 0 {
            &[]
        } else {
            // SAFETY: `check_readable` proved the range is the caller's own
            // slot or flash, both valid memory for the whole call.
            unsafe { core::slice::from_raw_parts(buf as *const u8, len) }
        };
        let from = self.tasks[slot].id;
        self.tasks[dest].mailbox.push(from, bytes);
        if self.tasks[dest].state == State::Receiving {
            self.tasks[dest].state = State::Ready;
        }
        Ok(Outcome::Return(len as isize))
    }

    pub fn sys_recv(&mut self, slot: usize, buf: usize, max: usize, from_out: usize) -> Result<Outcome, usize> {
        if max == 0 {
            return Err(ERROR_INVALID_ARGUMENT);
        }
        self.check_writable(slot, buf, max)?;
        if from_out != 0 && (from_out % 4 != 0 || self.check_writable(slot, from_out, 4).is_err()) {
            return Err(ERROR_BAD_ADDRESS);
        }

        // SAFETY: `check_writable` proved the range lies in the caller's
        // slot, which only the caller uses and which outlives the call.
        let out = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, max) };
        let is_console = self.tasks[slot].flags & TASK_CONSOLE != 0;

        // The console asking again means it has read the input it was
        // told about, so the UART interrupt may fire again
        if is_console && self.input_masked && !self.input_pending {
            self.input_masked = false;
            arch::irq_unmask(arch::UART0_IRQ);
        }

        let (from, n) = if is_console && self.input_pending {
            self.input_pending = false;
            (IRQ_SENDER, 0)
        } else if is_console && !self.log.is_empty() {
            let limit = max.min(MSG_SIZE);
            let n = self.log.take(&mut out[..limit]);
            // Writers blocked on a full stream can try again
            self.wake_senders(slot);
            (KERNEL_SENDER, n)
        } else if let Some(msg) = self.tasks[slot].mailbox.pop() {
            let n = msg.len.min(max);
            out[..n].copy_from_slice(&msg.data[..n]);
            self.wake_senders(slot);
            (msg.from, n)
        } else {
            self.tasks[slot].state = State::Receiving;
            arch::pend_switch();
            return Ok(Outcome::Retry);
        };

        if from_out != 0 {
            // SAFETY: checked writable and 4-byte aligned above
            unsafe { (from_out as *mut u32).write(from as u32) };
        }
        Ok(Outcome::Return(n as isize))
    }

    /// SYS_READ from a task: console input, one line (or up to one
    /// message) per call. The console server owns the UART, so this asks
    /// it with an empty message and blocks until its answer arrives.
    pub fn sys_read(&mut self, slot: usize, fd: usize, buf: usize, len: usize) -> Result<Outcome, usize> {
        if fd != STDIN || len == 0 {
            return Err(ERROR_INVALID_ARGUMENT);
        }
        self.check_writable(slot, buf, len)?;
        let console = self.console().ok_or(ERROR_NO_TASK)?;
        if console == slot {
            // The console would wait on itself forever
            return Err(ERROR_INVALID_ARGUMENT);
        }
        if let Some(msg) = self.tasks[slot].input.take() {
            let n = msg.len.min(len);
            // SAFETY: `check_writable` proved the range lies in the
            // caller's slot, which only the caller uses.
            let out = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, n) };
            out.copy_from_slice(&msg.data[..n]);
            return Ok(Outcome::Return(n as isize));
        }
        if !self.tasks[slot].reading {
            if self.tasks[console].mailbox.is_full() {
                self.tasks[slot].state = State::Sending { dest: console };
                arch::pend_switch();
                return Ok(Outcome::Retry);
            }
            let id = self.tasks[slot].id;
            self.tasks[console].mailbox.push(id, &[]);
            self.tasks[slot].reading = true;
            self.wake_console();
        }
        self.tasks[slot].state = State::Receiving;
        arch::pend_switch();
        Ok(Outcome::Retry)
    }

    /// UART interrupt: mask it and tell the console, which reads the UART
    pub fn input_irq(&mut self) {
        arch::irq_mask(arch::UART0_IRQ);
        self.input_masked = true;
        self.input_pending = true;
        self.wake_console();
    }

    /// SYS_WRITE from a task: append up to one message's worth of bytes to
    /// the console stream. The stream is shared with the kernel log, so the
    /// console prints everything in the order it happened. Returns how many
    /// bytes were taken; callers loop for the rest.
    pub fn sys_write(&mut self, slot: usize, buf: usize, len: usize) -> Result<Outcome, usize> {
        let console = self.console().ok_or(ERROR_NO_TASK)?;
        if console == slot {
            // The console would wait on its own stream forever
            return Err(ERROR_INVALID_ARGUMENT);
        }
        let n = len.min(MSG_SIZE);
        self.check_readable(slot, buf, n)?;
        if n == 0 {
            return Ok(Outcome::Return(0));
        }
        if self.log.space() < n {
            // Wait for the console to drain the stream
            self.tasks[slot].state = State::Sending { dest: console };
            arch::pend_switch();
            return Ok(Outcome::Retry);
        }
        // SAFETY: `check_readable` proved the range is the caller's own
        // slot or flash
        let bytes = unsafe { core::slice::from_raw_parts(buf as *const u8, n) };
        self.log.push(bytes);
        self.wake_console();
        Ok(Outcome::Return(n as isize))
    }

    pub fn sys_alloc(&mut self, slot: usize, size: usize) -> Result<usize, usize> {
        let ptr = self.tasks[slot].heap.allocate(size);
        if ptr.is_null() {
            Err(ERROR_OUT_OF_MEMORY)
        } else {
            Ok(ptr as usize)
        }
    }

    pub fn sys_free(&mut self, slot: usize, ptr: usize) -> Result<usize, usize> {
        // SAFETY: the task heap was initialised over the task's own slot,
        // which stays valid memory while the task exists.
        match unsafe { self.tasks[slot].heap.deallocate(ptr as *mut u8) } {
            Ok(()) => Ok(0),
            Err(e) => Err(crate::syscall::handler::free_error_code(e)),
        }
    }

    fn wake_senders(&mut self, dest: usize) {
        for t in self.tasks.iter_mut() {
            if t.state == (State::Sending { dest }) {
                t.state = State::Ready;
            }
        }
    }

    // ---- task death ------------------------------------------------------

    /// A task faulted. Logs it, kills the task and schedules another.
    pub fn task_fault(&mut self, slot: usize, exception: usize, cfsr: u32, addr: Option<usize>) {
        let (guard, guard_end) = self.guard_range(slot);
        let overflow = cfsr & CFSR_MSTKERR != 0 || addr.is_some_and(|a| guard <= a && a < guard_end);
        let t = &self.tasks[slot];
        let kind: &[u8] = match exception {
            3 => b"HardFault",
            4 => b"MemManage",
            5 => b"BusFault",
            6 => b"UsageFault",
            _ => b"fault",
        };
        let mut line = Line::new();
        line.bytes(b"  [fault] ").bytes(t.name).bytes(b": ").bytes(kind);
        if let Some(a) = addr {
            line.bytes(b" at ").hex(a as u32);
        }
        if overflow {
            line.bytes(b" (stack overflow)");
        }
        line.bytes(b", task killed\n");
        let expected = t.flags & TASK_EXPECT_FAULT != 0;
        let mut verdict = Line::new();
        verdict.bytes(b"  - ").bytes(t.name);
        verdict.bytes(if expected { b" stopped by fault: OK\n" } else { b" crashed: FAILED\n" });
        self.log_line(&line);
        self.log_line(&verdict);
        self.faulted += 1;
        self.retire(slot);
    }

    fn retire(&mut self, slot: usize) {
        let t = &mut self.tasks[slot];
        t.state = State::Free;
        t.mailbox.clear();
        t.reading = false;
        t.input = None;
        // Senders waiting on it retry and get ERROR_NO_TASK
        self.wake_senders(slot);
        if self.boosted == Some(slot) {
            self.boosted = None;
        }
        if self.current == Some(slot) {
            self.current = None;
            arch::set_psp(scratch_stack_top());
        }
        arch::pend_switch();
        self.spawn_pending();
        self.check_finished();
    }

    /// Once every test task is gone, report the totals and confirm the
    /// kernel never wrote to the UART itself since the scheduler started
    fn check_finished(&mut self) {
        if self.finished || self.pending[0].is_some() {
            return;
        }
        let busy = self
            .tasks
            .iter()
            .any(|t| t.state != State::Free && t.flags & (TASK_IDLE | TASK_CONSOLE | TASK_SERVICE) == 0);
        if busy {
            return;
        }
        self.finished = true;
        let mut line = Line::new();
        line.bytes(b"[TEST] Task tests complete: ")
            .dec(self.exited)
            .bytes(b" exited, ")
            .dec(self.faulted)
            .bytes(b" killed by faults\n");
        self.log_line(&line);
        let writes = uart::privileged_writes();
        let mut line = Line::new();
        line.bytes(b"  - Kernel UART writes since scheduler start = ").dec(writes);
        line.bytes(if writes == 0 { b": OK\n" } else { b": FAILED\n" });
        self.log_line(&line);
    }

    /// Queue a line for the console server and wake it
    pub fn log_line(&mut self, line: &Line) {
        self.log.commit(line);
        self.wake_console();
    }

    fn wake_console(&mut self) {
        if let Some(c) = self.console() {
            if self.tasks[c].state == State::Receiving {
                self.tasks[c].state = State::Ready;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::arch::imp::{IRQ_UNMASKED, MPU, SWITCH_PENDING};
    use std::boxed::Box;

    #[repr(C, align(2048))]
    struct Slots([u8; SLOT_SIZE * MAX_TASKS]);

    static FLASH: [u8; 64] = [7; 64];

    struct Fixture {
        k: Box<Kernel>,
        /// Raw so that moving the fixture does not invalidate the
        /// addresses the kernel holds (Miri checks this)
        mem: *mut Slots,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            // SAFETY: created by `Box::into_raw` in `kernel_with`
            drop(unsafe { Box::from_raw(self.mem) });
        }
    }

    impl Fixture {
        fn base(&self) -> usize {
            self.mem as usize
        }
        fn slot_addr(&self, slot: usize, offset: usize) -> usize {
            self.base() + slot * SLOT_SIZE + offset
        }
    }

    const ENTRY: usize = 0x1001;
    const TRAMPOLINE: usize = 0x2001;

    fn kernel_with(tasks: &[(&'static [u8], u32)]) -> Fixture {
        let mem = Box::into_raw(Box::new(Slots([0; SLOT_SIZE * MAX_TASKS])));
        let mut k = Box::new(Kernel::new());
        k.register(b"idle", ENTRY, 0, TASK_IDLE).unwrap();
        for &(name, flags) in tasks {
            k.register(name, ENTRY, 0, flags).unwrap();
        }
        let cfg = Config {
            slot_base: mem as usize,
            flash_start: FLASH.as_ptr() as usize,
            flash_end: FLASH.as_ptr() as usize + FLASH.len(),
            exit_trampoline: TRAMPOLINE,
        };
        k.start(cfg).unwrap();
        Fixture { k, mem }
    }

    fn take_pending() -> bool {
        SWITCH_PENDING.with(|p| p.replace(false))
    }

    fn drain_log(k: &mut Kernel) -> std::string::String {
        let mut out = [0u8; ipc::LOG_SIZE];
        let n = k.log.take(&mut out);
        std::string::String::from_utf8_lossy(&out[..n]).into_owned()
    }

    #[test]
    fn spawn_builds_initial_frame() {
        let f = kernel_with(&[(b"a", 0)]);
        let t = &f.k.tasks[1];
        assert_eq!(t.state, State::Ready);
        assert_eq!(t.id, 2);
        assert_eq!(t.sp, f.slot_addr(1, SLOT_SIZE - 64));
        assert_eq!(t.sp % 8, 0);
        let word = |i: usize| unsafe { ((t.sp + i * 4) as *const u32).read() as usize };
        assert_eq!(word(13), TRAMPOLINE);
        assert_eq!(word(14), ENTRY & !1);
        assert_eq!(word(15), INITIAL_XPSR);
    }

    #[test]
    fn round_robin_skips_idle_until_nothing_else_is_ready() {
        let mut f = kernel_with(&[(b"a", 0), (b"b", 0)]);
        let k = &mut f.k;
        let mut order = std::vec::Vec::new();
        for _ in 0..4 {
            k.switch(0);
            order.push(k.current.unwrap());
        }
        assert_eq!(order, [1, 2, 1, 2]);
        k.tasks[1].state = State::Receiving;
        k.tasks[2].state = State::Receiving;
        k.switch(0);
        assert_eq!(k.current, Some(0), "idle runs when everyone is blocked");
    }

    #[test]
    fn switch_maps_only_the_task_slot_and_uart_for_console() {
        let mut f = kernel_with(&[(b"console", TASK_CONSOLE), (b"a", 0)]);
        let base = f.base();
        f.k.switch(0);
        assert_eq!(f.k.current, Some(1));
        let regions = MPU.with(|m| *m.borrow());
        assert_eq!(regions[0].0, (base + SLOT_SIZE) as u32 | 0x10 | 1);
        assert_eq!(regions[1].0 & !0x1F, mpu::UART0_BASE);
        assert_eq!(regions[1].1 & 1, 1);
        let guard = (base + SLOT_SIZE + TASK_HEAP_SIZE) as u32;
        assert_eq!(regions[2].0, guard | 0x10 | 3, "stack guard above the heap");
        assert_eq!(regions[2].1 & 1, 1);
        f.k.switch(0);
        let regions = MPU.with(|m| *m.borrow());
        assert_eq!(regions[1].1, 0, "only the console gets the UART");
    }

    #[test]
    fn sleep_wakes_after_the_right_number_of_ticks() {
        let mut f = kernel_with(&[(b"sleeper", 0), (b"spinner", 0)]);
        let k = &mut f.k;
        k.switch(0);
        assert_eq!(k.current, Some(1));
        assert_eq!(k.sys_sleep(1, 100), Outcome::Return(0));
        assert!(take_pending());
        for _ in 0..9 {
            k.tick();
            assert!(matches!(k.tasks[1].state, State::Sleeping { .. }));
        }
        k.tick();
        assert_eq!(k.tasks[1].state, State::Ready);
        k.current = Some(2);
        k.switch(0);
        assert_eq!(k.current, Some(1), "woken task runs first");
    }

    #[test]
    fn send_and_recv_copy_and_block() {
        let mut f = kernel_with(&[(b"a", 0), (b"b", 0)]);
        let a_buf = f.slot_addr(1, 1100);
        let b_buf = f.slot_addr(2, 1100);
        let b_from = f.slot_addr(2, 1200);
        let k = &mut f.k;
        let b_id = k.tasks[2].id;

        // Receiver blocks on an empty mailbox and retries later
        assert_eq!(k.sys_recv(2, b_buf, 16, b_from), Ok(Outcome::Retry));
        assert_eq!(k.tasks[2].state, State::Receiving);

        unsafe { (a_buf as *mut [u8; 4]).write(*b"ping") };
        assert_eq!(k.sys_send(1, b_id, a_buf, 4), Ok(Outcome::Return(4)));
        assert_eq!(k.tasks[2].state, State::Ready, "send wakes the receiver");

        assert_eq!(k.sys_recv(2, b_buf, 16, b_from), Ok(Outcome::Return(4)));
        assert_eq!(unsafe { (b_buf as *const [u8; 4]).read() }, *b"ping");
        assert_eq!(unsafe { (b_from as *const u32).read() } as usize, k.tasks[1].id);

        // Fill the mailbox, then the sender blocks until the receiver drains one
        for _ in 0..ipc::MAILBOX_DEPTH {
            assert_eq!(k.sys_send(1, b_id, a_buf, 4), Ok(Outcome::Return(4)));
        }
        assert_eq!(k.sys_send(1, b_id, a_buf, 4), Ok(Outcome::Retry));
        assert_eq!(k.tasks[1].state, State::Sending { dest: 2 });
        k.sys_recv(2, b_buf, 16, 0).unwrap();
        assert_eq!(k.tasks[1].state, State::Ready);
    }

    #[test]
    fn console_input_round_trip() {
        let mut f = kernel_with(&[(b"console", TASK_CONSOLE), (b"reader", 0)]);
        let (rbuf, cbuf, from_out) = (f.slot_addr(2, 1100), f.slot_addr(1, 1100), f.slot_addr(1, 1200));
        let k = &mut f.k;
        let reader = k.tasks[2].id;
        let from = || unsafe { (from_out as *const u32).read() as usize };

        // The reader asks the console and blocks
        assert_eq!(k.sys_read(2, STDIN, rbuf, 16), Ok(Outcome::Retry));
        assert_eq!(k.tasks[2].state, State::Receiving);
        assert!(k.tasks[2].reading);
        // Asking again while waiting does not send a second request
        assert_eq!(k.sys_read(2, STDIN, rbuf, 16), Ok(Outcome::Retry));
        assert_eq!(k.sys_recv(1, cbuf, 64, from_out), Ok(Outcome::Return(0)));
        assert_eq!(from(), reader, "empty message from the reader = request");
        assert_eq!(k.sys_recv(1, cbuf, 64, from_out), Ok(Outcome::Retry));

        // Input arrives: the line is masked until the console has read it
        IRQ_UNMASKED.with(|u| u.set(true));
        k.tasks[1].state = State::Receiving;
        k.input_irq();
        assert!(!IRQ_UNMASKED.with(|u| u.get()));
        assert_eq!(k.tasks[1].state, State::Ready);
        assert_eq!(k.sys_recv(1, cbuf, 64, from_out), Ok(Outcome::Return(0)));
        assert_eq!(from(), IRQ_SENDER);
        assert!(!IRQ_UNMASKED.with(|u| u.get()), "still masked while the console reads");
        assert_eq!(k.sys_recv(1, cbuf, 64, from_out), Ok(Outcome::Retry));
        assert!(IRQ_UNMASKED.with(|u| u.get()), "unmasked once the console asks again");

        // Fill the reader's mailbox: the answer must still get through
        for _ in 0..ipc::MAILBOX_DEPTH {
            assert!(k.tasks[2].mailbox.push(99, b"x"));
        }
        unsafe { (cbuf as *mut [u8; 3]).write(*b"hi
") };
        assert_eq!(k.sys_send(1, reader, cbuf, 3), Ok(Outcome::Return(3)));
        assert_eq!(k.tasks[2].state, State::Ready);
        assert_eq!(k.sys_read(2, STDIN, rbuf, 16), Ok(Outcome::Return(3)));
        assert_eq!(unsafe { (rbuf as *const [u8; 3]).read() }, *b"hi
");
        assert!(!k.tasks[2].reading);

        // An empty message to the console would look like a request
        let bad: Result<Outcome, usize> = Err(ERROR_INVALID_ARGUMENT);
        let console = k.tasks[1].id;
        assert_eq!(k.sys_send(2, console, rbuf, 0), bad);
        assert_eq!(k.sys_read(1, STDIN, cbuf, 16), bad, "the console cannot read from itself");
        assert_eq!(k.sys_read(2, 1, rbuf, 16), bad, "only stdin");
    }

    #[test]
    fn user_pointers_are_checked() {
        let mut f = kernel_with(&[(b"a", 0), (b"b", 0)]);
        let own = f.slot_addr(1, 1100);
        let other = f.slot_addr(2, 1100);
        let flash = FLASH.as_ptr() as usize;
        let k = &mut f.k;
        let b_id = k.tasks[2].id;
        let bad = Err(ERROR_BAD_ADDRESS);
        let bad_call: Result<Outcome, usize> = Err(ERROR_BAD_ADDRESS);

        assert_eq!(k.check_readable(1, flash, 64), Ok(()));
        assert_eq!(k.check_writable(1, flash, 1), bad);
        assert_eq!(k.check_readable(1, other, 4), bad);
        assert_eq!(k.check_readable(1, own, SLOT_SIZE), bad, "runs past the slot");
        assert_eq!(k.check_readable(1, usize::MAX - 2, 8), bad);
        let guard = own - 1100 + TASK_HEAP_SIZE;
        assert_eq!(k.check_writable(1, guard - 8, 8), Ok(()), "top of the heap");
        assert_eq!(k.check_writable(1, guard + STACK_GUARD, 8), Ok(()), "bottom of the stack");
        assert_eq!(k.check_writable(1, guard + 4, 4), bad, "inside the stack guard");
        assert_eq!(k.check_readable(1, guard - 8, 16), bad, "runs into the stack guard");
        assert_eq!(k.check_writable(1, guard - 8, 64), bad, "spans the stack guard");

        assert_eq!(k.sys_send(1, b_id, other, 4), bad_call);
        assert_eq!(k.sys_recv(1, flash, 4, 0), bad_call);
        assert_eq!(k.sys_recv(1, own, 4, other), bad_call);
        assert_eq!(k.sys_recv(1, own, 4, own + 1), bad_call, "misaligned sender slot");
        assert_eq!(k.sys_send(1, 99, own, 4), Err(ERROR_NO_TASK));
        assert_eq!(k.sys_send(1, b_id, own, MSG_SIZE + 1), Err(ERROR_INVALID_ARGUMENT));
    }

    #[test]
    fn task_heap_is_inside_the_slot() {
        let mut f = kernel_with(&[(b"a", 0)]);
        let (start, end) = (f.slot_addr(1, 0), f.slot_addr(1, TASK_HEAP_SIZE));
        let k = &mut f.k;
        let p = k.sys_alloc(1, 100).unwrap();
        assert!(p >= start && p + 100 <= end);
        assert_eq!(k.sys_free(1, p), Ok(0));
        assert_eq!(k.sys_free(1, p), Err(ERROR_DOUBLE_FREE));
        assert_eq!(k.sys_alloc(1, TASK_HEAP_SIZE), Err(ERROR_OUT_OF_MEMORY));
        assert_eq!(k.sys_free(1, 0x2000_2010), Err(ERROR_INVALID_ARGUMENT));
    }

    #[test]
    fn exit_frees_the_slot_for_queued_tasks() {
        let names: [&'static [u8]; 9] = [b"t1", b"t2", b"t3", b"t4", b"t5", b"t6", b"t7", b"t8", b"t9"];
        let tasks: std::vec::Vec<_> = names.iter().map(|&n| (n, 0)).collect();
        let mut f = kernel_with(&tasks);
        let k = &mut f.k;
        assert!(k.pending[0].is_some(), "two tasks wait for a slot");
        k.current = Some(3);
        k.sys_exit(3, 0);
        assert_eq!(k.current, None);
        assert_eq!(k.tasks[3].name, b"t8");
        k.sys_exit(4, 0);
        assert_eq!(k.tasks[4].name, b"t9");
        assert!(drain_log(k).is_empty(), "clean exits log nothing");
    }

    #[test]
    fn faults_are_logged_and_judged() {
        let mut f = kernel_with(&[(b"console", TASK_CONSOLE), (b"bad", TASK_EXPECT_FAULT), (b"oops", 0)]);
        let k = &mut f.k;
        k.task_fault(2, 4, 0, Some(0x2000_0000));
        k.task_fault(3, 5, 0, None);
        let log = drain_log(k);
        assert!(log.contains("bad: MemManage at 0x20000000"));
        assert!(log.contains("  - bad stopped by fault: OK\n"));
        assert!(log.contains("  - oops crashed: FAILED\n"));
        assert!(log.contains("[TEST] Task tests complete: 0 exited, 2 killed by faults\n"));
        assert!(log.contains("since scheduler start = 0: OK\n"));
    }

    #[test]
    fn console_stream_keeps_task_and_kernel_output_in_order() {
        let mut f = kernel_with(&[(b"console", TASK_CONSOLE), (b"a", 0)]);
        let buf = f.slot_addr(1, 1100);
        let from = f.slot_addr(1, 1200);
        let a_buf = f.slot_addr(2, 1100);
        let k = &mut f.k;
        unsafe { (a_buf as *mut [u8; 3]).write(*b"hi
") };
        assert_eq!(k.sys_write(2, a_buf, 3), Ok(Outcome::Return(3)));
        let mut line = Line::new();
        line.bytes(b"kernel says
");
        k.log_line(&line);
        assert_eq!(k.sys_write(2, a_buf, 3), Ok(Outcome::Return(3)));

        assert_eq!(k.sys_recv(1, buf, 64, from), Ok(Outcome::Return(18)));
        assert_eq!(unsafe { (buf as *const [u8; 18]).read() }, *b"hi
kernel says
hi
");
        assert_eq!(unsafe { (from as *const u32).read() }, KERNEL_SENDER as u32);
        assert_eq!(k.sys_write(1, buf, 2), Err(ERROR_INVALID_ARGUMENT), "console can't write to itself");
    }

    #[test]
    fn writers_block_while_the_console_stream_is_full() {
        let mut f = kernel_with(&[(b"console", TASK_CONSOLE), (b"a", 0)]);
        let buf = f.slot_addr(1, 1100);
        let a_buf = f.slot_addr(2, 1100);
        let k = &mut f.k;
        for _ in 0..ipc::LOG_SIZE / MSG_SIZE {
            assert_eq!(k.sys_write(2, a_buf, MSG_SIZE), Ok(Outcome::Return(MSG_SIZE as isize)));
        }
        assert_eq!(k.sys_write(2, a_buf, 1), Ok(Outcome::Retry));
        assert_eq!(k.tasks[2].state, State::Sending { dest: 1 });
        k.sys_recv(1, buf, 64, 0).unwrap();
        assert_eq!(k.tasks[2].state, State::Ready, "draining wakes the writer");
        assert_eq!(k.sys_write(2, a_buf, 1), Ok(Outcome::Return(1)));
    }
}
