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
