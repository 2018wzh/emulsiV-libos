/* Virgule fixed vectors. No CSR, ECALL, WFI, compressed or atomic instructions. */
.option norvc
.option norelax
.section .vectors,"ax",@progbits
.global __reset
__reset:
    j __start
    j __irq_entry

.section .text.init,"ax",@progbits
__start:
    la gp, __global_pointer$
    la sp, __stack_top
    /* Disable device interrupts before BSS initialization. */
    lui t0, 0xb0000
    sb zero, 0(t0)
    lui t0, 0xd0000
    sw zero, 4(t0)
    la t0, __bss_start
    la t1, __bss_end
1:
    bgeu t0, t1, 2f
    sw zero, 0(t0)
    addi t0, t0, 4
    j 1b
2:
    call __libos_main
3:
    j 3b

/* Rust callbacks follow the C ABI and preserve s0-s11 themselves.
 * Save all caller-saved registers before touching them. 64-byte frame keeps
 * the required 16-byte SP alignment. Virgule prevents nested IRQs until MRET. */
.global __irq_entry
__irq_entry:
    addi sp, sp, -64
    sw ra, 0(sp)
    sw t0, 4(sp)
    sw t1, 8(sp)
    sw t2, 12(sp)
    sw a0, 16(sp)
    sw a1, 20(sp)
    sw a2, 24(sp)
    sw a3, 28(sp)
    sw a4, 32(sp)
    sw a5, 36(sp)
    sw a6, 40(sp)
    sw a7, 44(sp)
    sw t3, 48(sp)
    sw t4, 52(sp)
    sw t5, 56(sp)
    sw t6, 60(sp)
    call __libos_irq_handler
    lw ra, 0(sp)
    lw t0, 4(sp)
    lw t1, 8(sp)
    lw t2, 12(sp)
    lw a0, 16(sp)
    lw a1, 20(sp)
    lw a2, 24(sp)
    lw a3, 28(sp)
    lw a4, 32(sp)
    lw a5, 36(sp)
    lw a6, 40(sp)
    lw a7, 44(sp)
    lw t3, 48(sp)
    lw t4, 52(sp)
    lw t5, 56(sp)
    lw t6, 60(sp)
    addi sp, sp, 64
    mret
