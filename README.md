# CPOS - ARM Cortex-M3 Operating System

## Version: 0.2.0

Bare-metal ARM OS designed for embedded systems with ARM Cortex-M3 processors.


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
kernel/           - Core kernel logic
lib/              - Utility libraries for C components
rust_kernel/      - Rust kernel components
  ├─ memory/      - Memory management implementation
  ├─ syscall/     - System calls implementation
  └─ lib.rs       - Rust entry point and FFI interface
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

CPOS uses the standard ARM Cortex-M3 interrupt vector system for handling exceptions and hardware interrupts.

### Vector Table

- Located at the beginning of Flash memory
- Contains addresses of exception handlers
- Implemented in `vectors.c` and placed using the `.vectors` section

The key vector entries include:

- **0x00000000**: Initial Stack Pointer - Stack location for exceptions
- **0x00000004**: Reset_Handler - System reset entry point
- **0x00000008**: NMI_Handler - Non-maskable interrupt
- **0x0000000C**: HardFault_Handler - All classes of faults
- **0x0000002C**: SVC_Handler - Supervisor call (system calls)

### Implementation

- **Default Handlers**: All exceptions initially point to a default handler
- **Weak Symbols**: Handlers are declared with `__attribute__((weak))`
- **Override Mechanism**: Specific handlers can be implemented where needed
- **Vector Positioning**: Linker script places vectors at the correct memory address

### Exception Types

- **System Exceptions**: Reset, NMI, HardFault, etc.
- **SVC (Supervisor Call)**: Entry point for system calls
- **Peripheral Interrupts**: For device-specific interrupt handling

### Usage Example

Implementing a custom SVC handler:

```c
void SVC_Handler(void)
{
    // Identify which system call was requested
    // Handle the system call
    // Return to user mode
    uart_send_string("System call processed\n");
}
```

Triggering a system call:

```c
// Generate a supervisor call (SVC) with immediate value #0
__asm volatile("svc #0");
```

## Memory Management

CPOS uses a hybrid approach to memory management, combining C and Rust:

### Architecture

- **RAM Layout**: 32KB total (0x20000000 - 0x20008000)
  - Boot Data: 0x20000000 - 0x20001000
  - Kernel Heap: 0x20001000 - 0x20007000 (24KB)
  - Kernel Stack: 0x20007000 - 0x20008000

### Memory Implementation

- **Allocator Type**: Linked List Allocator
- **Language**: Implemented in Rust for memory safety
- **Features**:
  - Thread-safe (mutex-protected)
  - First-fit allocation strategy, 8-byte aligned blocks
  - Block splitting and coalescing of neighbouring free blocks
  - Rejects double frees and pointers it did not hand out
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

### Testing

The allocator and syscall dispatcher have host-side unit tests, including a
randomized stress test that checks after every operation that no bytes are
lost and the free list stays sorted and coalesced:

```bash
cd rust_kernel
cargo test
```

The kernel also runs a leak check at boot (visible with `make qemu`): after
thousands of mixed alloc/free cycles the heap must be back to a single free
block of its original size.

## System Call Interface

CPOS provides a robust system call interface allowing user programs to securely interact with kernel services. The system call mechanism follows ARM EABI conventions and leverages the hardware's SVC (Supervisor Call) instruction.

### Syscall Architecture

- Dual Interface: System calls can be invoked via C functions or direct SVC instructions
- Language: Core implementation in Rust for memory safety and robust error handling
- Stack-Based Arguments: Follows ARM EABI calling conventions

### Available System Calls

| Number | Name      | Description                                     | Arguments          | Status                    |
|--------|-----------|-------------------------------------------------|--------------------|---------------------------|
| 1      | SYS_WRITE | Write bytes to UART (fd 1)                      | fd, buffer, length | Implemented               |
| 2      | SYS_READ  | Read from UART (fd 0) up to length or a newline | fd, buffer, length | Implemented               |
| 10     | SYS_EXIT  | Terminate current process                       | exit_code          | Planned (needs scheduler) |
| 11     | SYS_SLEEP | Sleep for specified milliseconds                | ms                 | Planned (needs scheduler) |
| 20     | SYS_ALLOC | Allocate memory                                 | size               | Implemented               |
| 21     | SYS_FREE  | Free allocated memory                           | pointer            | Implemented               |

Errors are returned as negative values: `-1` invalid syscall, `-2` invalid
argument, `-3` not implemented, `-4` out of memory, `-5` double free.

### Usage Examples

From C Code:

```c
// Write to standard output
const char *message = "Hello, World!";
int result = rust_syscall(SYS_WRITE, 1, (uint32_t)message, 13);

// Allocate memory
uint32_t ptr = rust_syscall(SYS_ALLOC, 1024, 0, 0);
if (ptr > 0) {
    // Use allocated memory
    rust_syscall(SYS_FREE, ptr, 0, 0);
}
```

Using the SVC instruction directly (number in r0, arguments in r1-r3, result
back in r0):

```c
register uint32_t r0 __asm__("r0") = SYS_WRITE;
register uint32_t r1 __asm__("r1") = 1;              // fd = 1 (stdout)
register uint32_t r2 __asm__("r2") = (uint32_t)message;
register uint32_t r3 __asm__("r3") = length;
__asm volatile("svc #0" : "+r"(r0) : "r"(r1), "r"(r2), "r"(r3) : "memory");
int32_t result = (int32_t)r0;
```

The kernel does not drop to unprivileged mode yet, so SVC currently goes
from privileged thread mode to handler mode.
