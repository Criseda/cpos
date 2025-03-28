/*
 * SPDX-License-Identifier: MIT
 * Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
 *
 * Author: Laurentiu Cristian Preda (criseda)
 * GitHub: https://github.com/criseda
 *
 * UART Interface for CPOS
 * 
 * This header file declares UART functionality for serial communication.
 */

#ifndef UART_H
#define UART_H

#include <stdint.h>

/* Function prototypes for UART */
void uart_init(uint32_t baudrate);
void uart_send_char(char c);
void uart_send_string(const char *str);
char uart_receive_char(void);

#endif /* UART_H */
