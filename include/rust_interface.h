#ifndef RUST_INTERFACE_H
#define RUST_INTERFACE_H

#include <stdint.h>
#include <stddef.h>

// Memory management functions provided by Rust
void rust_init_heap(uintptr_t heap_start, size_t heap_size);
void *rust_heap_alloc(size_t size);
void rust_heap_free(void *ptr);

#endif // RUST_INTERFACE_H
