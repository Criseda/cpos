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

## License

[MIT](LICENSE)
