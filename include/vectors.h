/*
 * SPDX-License-Identifier: MIT
 * Copyright (C) 2025 Laurentiu Cristian Preda <laurentiu.cristian.preda@gmail.com>
 *
 * Author: Laurentiu Cristian Preda (criseda)
 * GitHub: https://github.com/criseda
 *
 * Interrupt Vector Interface for CPOS
 * 
 * This header file declares the interrupt vector table functions
 * and related functionality.
 */

#ifndef VECTORS_H
#define VECTORS_H

/* Weak handler declarations that can be overridden */
void NMI_Handler(void);
void HardFault_Handler(void);
void MemManage_Handler(void);
void BusFault_Handler(void);
void UsageFault_Handler(void);
void SVC_Handler(void);
void DebugMon_Handler(void);
void PendSV_Handler(void);
void SysTick_Handler(void);

/* Default handler for all interrupts */
void Default_Handler(void);

#endif /* VECTORS_H */
