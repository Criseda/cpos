# CPOS - ARM Cortex-M3 Operating System

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
  └─ lib.rs       - Rust entry point and FFI interface
tests/            - Test cases and debugging tools
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
- **SVC (Supervisor Call)**: Used for system calls from user to kernel mode
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

## License

[MIT](LICENSE)
