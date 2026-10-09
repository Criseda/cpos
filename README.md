# CPOS - ARM Cortex-M3 Operating System

## Version: 0.2.0

Bare-metal ARM OS designed for embedded systems with ARM Cortex-M3 processors.

A small microkernel-style OS: the kernel (C startup plus a Rust core)
provides preemptive scheduling, MPU memory isolation, message passing and
memory allocation, while tasks, including the UART console driver, run
unprivileged. See [Verified claims](#verified-claims) for what is measured
and how to reproduce it.


## Getting Started

### Prerequisites

Before building and running CPOS, you'll need to install several tools:

- ARM GCC Toolchain
- QEMU System Emulator
- Make

See [INSTALLATION.md](docs/INSTALLATION.md) for detailed instructions on installing these prerequisites on various operating systems.

## 📂 Project Structure

```plaintext
bootloader/       - Boot code responsible for loading the OS
docs/             - Documentation and specifications
include/          - Header files (shared definitions)
init/             - System initialization (before kernel runs)
kernel/           - Core kernel logic, exception entry (sched.c)
lib/              - Utility libraries for C components
user/             - Unprivileged tasks: idle, console server, tests
rust_kernel/      - Rust kernel components
  ├─ memory/      - Memory management implementation
  ├─ syscall/     - System calls implementation
  ├─ task/        - Scheduler, MPU regions, message passing
  ├─ fuzz/        - cargo-fuzz targets
  └─ lib.rs       - Rust entry point and FFI interface
scripts/          - unsafe_audit.py
linker.ld         - Defines memory layout for program execution
Makefile          - Automates building and cleaning the project.
```

## Quick Start

```bash
# Clone the repository
git clone https://github.com/criseda/cpos.git
cd cpos

# Build the project
make

# Run in QEMU
make qemu
```

## Usage

For detailed instructions on building, running and extending CPOS, see [USAGE.md](docs/USAGE.md)

## License

[MIT](LICENSE)

## Interrupt Handling

CPOS uses the standard ARM Cortex-M3 vector table (`kernel/vectors.c`,
placed at the start of flash by `linker.ld`). Unused entries fall back to a
weak default handler.

| Exception | Handler | Purpose |
|-----------|---------|---------|
| Reset | `Reset_Handler` | Copy `.data`, zero `.bss`, run `init` |
| HardFault, MemManage, BusFault, UsageFault | shared fault entry | Kill the faulting task; halt if the kernel faulted |
| SVCall | `SVC_Handler` | System calls |
| PendSV | `PendSV_Handler` | Context switch (lowest priority) |
| SysTick | `SysTick_Handler` | 10 ms scheduler tick |

SVC, SysTick and the configurable faults share one priority so they never
preempt each other while holding kernel state; PendSV runs below them.

## Tasks and Scheduling

- **Preemptive round robin**: SysTick fires every 10 ms and ends the running
  task's slice; a task that wakes from sleep runs next.
- **Context switch** in PendSV: the hardware stacks r0-r3, r12, lr, pc and
  xPSR, the kernel saves r4-r11 on the task's process stack (PSP).
- **Unprivileged tasks**: tasks run in thread mode with `CONTROL.nPRIV = 1`
  on PSP. They cannot raise their own privilege.
- **Blocking calls** (receive on an empty mailbox, send to a full one) block
  the task and rewind its PC to the `svc`, so the call simply runs again when
  the task is woken.
- **Task lifecycle**: up to 8 tasks live at once; more are queued and start
  when a slot frees up. Returning from a task's entry function exits it.

### Memory protection

Each task owns one 2 KB, size-aligned RAM slot: a private 1 KB heap at the
bottom, a 32-byte stack guard, and its stack (992 bytes) above. On every
switch the kernel programs the MPU so the task can reach only:

| Region | Memory | Task access |
|--------|--------|-------------|
| 0 | Flash (code, constants) | Read, execute |
| 1 | The task's own slot | Read, write, no execute |
| 2 | UART0 registers | Read, write; console server only |
| 3 | Stack guard, between the heap and the stack | None |

Kernel data, the kernel heap, the kernel stack, other tasks' slots and all
other peripherals fault. Region 3 overlaps region 1 and wins, so a stack
that grows past its limit faults on the guard, and the kernel reports
`(stack overflow)`, instead of silently overwriting the heap. The kernel also checks every pointer a task passes
in a system call against the memory that task owns (its slot, or flash for
read-only buffers) and returns `-6` (bad address) otherwise.

### Message passing and the console server

Each task has a mailbox of four 64-byte messages (`SYS_SEND`/`SYS_RECV`).
The UART driver runs as an unprivileged **console server** task, the only
task the MPU lets reach the UART. Once the scheduler starts the kernel never
touches the UART: `SYS_WRITE` from a task, and the kernel's own log lines
(faults, test results), go into one ordered stream that the console server
drains and prints. A counter in the C UART driver records any byte written
by privileged code after that point; the boot test checks it stays 0.

## Memory Management

CPOS uses a hybrid approach to memory management, combining C and Rust:

### Architecture

- **RAM Layout**: 32KB total (0x20000000 - 0x20008000)
  - Kernel `.data`/`.bss`: 0x20000000 - 0x20002000 (8KB, checked at link time)
  - Kernel Heap: 0x20002000 - 0x20003000 (4KB)
  - Task slots: 0x20003000 - 0x20007000 (8 x 2KB)
  - Kernel Stack: 0x20007000 - 0x20008000

### Memory Implementation

- **Allocator Type**: Linked List Allocator
- **Language**: Implemented in Rust for memory safety
- **Features**:
  - Thread-safe (mutex-protected)
  - First-fit allocation strategy, 8-byte aligned blocks
  - Block splitting and coalescing of neighbouring free blocks
  - Rejects double frees and pointers it did not hand out
  - Validates every free-list header before use, so even a heap whose
    headers were overwritten (a task can write its own heap) never makes the
    kernel write outside that heap
  - One kernel heap, plus a private heap per task (`SYS_ALLOC` in a task)
  - Usage counters (`rust_heap_free_bytes`, `rust_heap_free_blocks`)

### C-Rust Integration

C code can access the memory allocator through simple FFI functions:

```c
// Initialize heap
rust_init_heap(HEAP_START, HEAP_SIZE);

// Allocate memory
void* ptr = rust_heap_alloc(size);

// Free memory (0 on success, -1 invalid pointer, -2 double free)
rust_heap_free(ptr);
```

## System Call Interface

System calls use the SVC instruction: number in r0, arguments in r1-r3,
result back in r0. Errors are negative. The core is implemented in Rust.

Calls come from one of two places. Before the scheduler starts, the kernel
can call them itself (the boot tests do); after that they come from
unprivileged tasks, and every pointer is checked.

### Available System Calls

| Number | Name       | Description | Arguments |
|--------|------------|-------------|-----------|
| 1      | SYS_WRITE  | Write to the console (fd 1); from a task, returns the bytes taken, at most 64 per call | fd, buffer, length |
| 2      | SYS_READ   | Read from the UART (fd 0) up to length or a newline; kernel context only, tasks get `-3` | fd, buffer, length |
| 10     | SYS_EXIT   | End the calling task | exit_code |
| 11     | SYS_SLEEP  | Sleep for at least the given milliseconds (10 ms ticks) | ms |
| 12     | SYS_YIELD  | Give up the rest of the time slice | - |
| 13     | SYS_TICKS  | Ticks since the scheduler started | - |
| 20     | SYS_ALLOC  | Allocate memory (in a task: from its own heap) | size |
| 21     | SYS_FREE   | Free allocated memory | pointer |
| 30     | SYS_SEND   | Send up to 64 bytes to a task; blocks while its mailbox is full | task id, buffer, length |
| 31     | SYS_RECV   | Receive a message; blocks while the mailbox is empty | buffer, max length, sender id out (may be 0) |

Errors: `-1` invalid syscall, `-2` invalid argument, `-3` not implemented,
`-4` out of memory, `-5` double free, `-6` bad address (pointer outside the
caller's memory), `-7` no such task (or a task-only call made outside a
task).

### Usage Example

From a task (wrappers in `include/user.h`):

```c
char buf[64];
uint32_t from;

sys_write("hello\n", 6);                 /* printed by the console server */
sys_sleep(100);                          /* about 10 ticks */
int32_t p = sys_alloc(100);              /* inside this task's slot */
sys_free(p);
sys_send(other_task, "ping", 4);
int32_t n = sys_recv(buf, sizeof(buf), &from);
```

## Testing

```bash
# Host unit tests: allocator, syscalls, scheduler, IPC, MPU encoding
cd rust_kernel && cargo test

# Same tests under Miri (undefined-behaviour checker). Addresses arrive as
# integers from the linker and C, hence permissive provenance.
MIRIFLAGS=-Zmiri-permissive-provenance cargo +nightly miri test

# Fuzzing (nightly + cargo-fuzz)
cd fuzz
cargo +nightly fuzz run allocator -- -max_total_time=120
cargo +nightly fuzz run allocator_hostile -- -max_total_time=120
cargo +nightly fuzz run syscalls -- -max_total_time=120

# Unsafe audit: every unsafe needs a SAFETY justification
python3 scripts/unsafe_audit.py rust_kernel

# On-target tests: boot in QEMU, every check prints OK or FAILED
make && make qemu
```

The boot run first exercises the allocator and syscalls from the kernel,
then starts the scheduler and runs unprivileged test tasks: preemption,
sleep timing, privilege, MPU faults, pointer checks, per-task heaps, IPC and
exit. Expected faults are reported as `stopped by fault: OK`.

## Verified claims

Measured with Rust 1.99 (host tests), nightly 1.101 (Miri, cargo-fuzz) and
QEMU 7.2 (`lm3s6965evb`) in a Debian bookworm container. The QEMU boot run
reports 41 checks OK and 0 FAILED.

| Claim | Evidence |
|-------|----------|
| 10 system calls, including process control (exit, sleep, yield) and message passing | Syscall table above; QEMU boot tests and host tests |
| Preemptive multitasking | QEMU: two busy-looping tasks are each preempted 13-14 times in 30 ticks; a 100 ms sleep under load wakes after exactly 10 ticks |
| Tasks run unprivileged; system calls switch to privileged handler mode | QEMU: `CONTROL.nPRIV = 1` in tasks, a task cannot clear it, and its write to the SysTick control register has no effect |
| MPU isolation between tasks and from the kernel | QEMU: tasks touching kernel data, another task's stack or the UART are killed by MemManage faults; the kernel rejects out-of-bounds pointers with `-6` |
| Stack overflow detection | QEMU: a runaway recursive task is killed by a MemManage fault inside its stack guard, before it reaches its heap |
| Microkernel-style: the UART driver is an unprivileged server task | QEMU: all task output goes through the console server; privileged UART writes after scheduler start = 0 |
| No leaks in the allocator | Host tests: 200,000 randomized operations with exact byte accounting and invariant checks after every step; boot test: the heap returns to one block of its original size after 1,000 mixed cycles; Miri: no undefined behaviour in the test suite; fuzzing: 2.4M random alloc/free sequences end fully coalesced |
| Memory-safety discipline in the Rust core | 31 `unsafe` sites in kernel code (24 blocks, 6 functions, 1 impl), all with a written `SAFETY` justification (`scripts/unsafe_audit.py`); fuzzing: 2.75M random syscall sequences with hostile pointers and 5.7M allocator runs with corrupted headers, with no crash, hang or out-of-bounds write |

Known limits:

- QEMU 7.2 lets an unprivileged task *read* System Control Space registers;
  real Cortex-M3 hardware raises a BusFault. Tasks still cannot change them.
- `SYS_READ` works only before the scheduler starts; console input for tasks
  would be a request to the console server and is not implemented.
- The stack guard is 32 bytes. A single function that reserves more stack
  than that at once can step over it; the allocator's header checks still
  keep a corrupted heap from leading the kernel outside it.
- The allocator detects double frees of free memory, but freeing a stale
  pointer into a block that has since been reused (use after free) cannot be
  told apart from a valid free. Even then it never writes outside the heap.
- No comparative security figure against C is claimed: the measurements
  above are what is checked.
