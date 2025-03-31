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

- **Offset 0x00**: Initial Stack Pointer - Stack location for exceptions
- **Offset 0x04**: Reset_Handler - System reset entry point
- **Offset 0x08**: NMI_Handler - Non-maskable interrupt
- **Offset 0x0C**: HardFault_Handler - All classes of faults
- **Offset 0x2C**: SVC_Handler - Supervisor call (system calls)

### Implementation

- **Default Handlers**: All exceptions initially point to a default handler
- **Weak Symbols**: Handlers are declared with `__attribute__((weak))`
- **Override Mechanism**: Specific handlers can be implemented where needed
- **Vector Positioning**: Linker script places vectors at the correct memory address

### Exception Types

- **System Exceptions**: Reset, NMI, HardFault, etc.
- **SVC (Supervisor Call)**: Used for system calls from user to kernel mode
- **Peripheral Interrupts**: For device-specific interrupt handling

### Usage Example

#### Implementing a Custom SVC Handler

In CPOS, the `SVC_Handler` is implemented in assembly and delegates system call handling to a Rust function (`rust_handle_svc`). The handler determines the stack pointer (MSP or PSP) and passes the system call number and arguments to the Rust handler.

```c
void __attribute__((naked)) SVC_Handler(void)
{
    __asm volatile(
        "tst lr, #4\n" // Test bit 2 of EXC_RETURN to determine stack used
        "ite eq\n" // If-Then-Else block
        "mrseq r1, msp\n" // If bit 2 is clear, use MSP
        "mrsne r1, psp\n" // If bit 2 is set, use PSP
        "push {lr}\n" // Save link register
        "bl rust_handle_svc\n" // Call Rust handler
        "pop {pc}\n" // Return
    );
}
```

#### Triggering a system call

```c
// Generate a supervisor call (SVC) with immediate value #0
__asm volatile("svc #0");
```

This will invoke the `SVC_Handler`, which processes the system call.

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
  - First-fit allocation strategy
  - Block splitting to reduce fragmentation
  - Size tracking for proper deallocation

### C-Rust Integration

C code can access the memory allocator through simple FFI functions:

```c
// Initialize heap
rust_init_heap(HEAP_START, HEAP_SIZE);

// Allocate memory
void* ptr = rust_heap_alloc(size);

// Free memory
rust_heap_free(ptr);
```

## System Call Interface

CPOS provides a robust system call interface allowing user programs to securely interact with kernel services. The system call mechanism follows ARM EABI conventions and leverages the hardware's SVC (Supervisor Call) instruction.

### Syscall Architecture

- Dual Interface: System calls can be invoked via C functions or direct SVC instructions
- Language: Core implementation in Rust for memory safety and robust error handling
- Stack-Based Arguments: Follows ARM EABI calling conventions

### Available System Calls

| Number | Name      | Description                      | Arguments          |
|--------|-----------|----------------------------------|--------------------|
| 1      | SYS_WRITE | Write data to output device      | fd, buffer, length |
| 2      | SYS_READ  | Read data from input device      | fd, buffer, length |
| 10     | SYS_EXIT  | Terminate current process        | exit_code          |
| 11     | SYS_SLEEP | Sleep for specified milliseconds | ms                 |
| 20     | SYS_ALLOC | Allocate memory                  | size               |
| 21     | SYS_FREE  | Free allocated memory            | pointer            |

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

Using SVC instruction directly:

```c
const char *message = "Hello from user space!";
__asm volatile(
    "mov r0, #1\n"        // SYS_WRITE syscall number
    "mov r1, #1\n"        // fd = 1 (stdout)
    "ldr r2, %[msg]\n"    // buffer address
    "mov r3, #21\n"       // length of message
    "svc #0\n"            // SVC instruction
    :
    : [msg] "m"(message)
    : "r0", "r1", "r2", "r3", "memory"
);
```
