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
bootloader/   - Boot code responsible for loading the OS
docs/         - Documentation and specifications
include/      - Header files (shared definitions)
init/         - System initialization (before kernel runs)
kernel/       - Core kernel logic
lib/          - Utility libraries (if needed)
tests/        - Test cases and debugging tools
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

## Memory Management

CPOS uses a hybrid approach to memory management, combining C and Rust:

### Architecture

- **RAM Layout**: 32KB total (0x20000000 - 0x20008000)
  - Boot Data: 0x20000000 - 0x20001000
  - Kernel Heap: 0x20001000 - 0x20007000 (24KB)
  - Kernel Stack: 0x20007000 - 0x20008000

### Implementation

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
