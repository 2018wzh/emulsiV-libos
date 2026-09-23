.option norvc
.option norelax
.section .text,"ax",@progbits
.global __libos_main
__libos_main:
    lui t0, 0xc0000
    li t1, 82
    sb t1, 0(t0)
.global fixture_idle
fixture_idle:
    j fixture_idle
.global __libos_default_irq
__libos_default_irq:
    lui t0, 0xb0000
    lbu t1, 1(t0)
    sb zero, 0(t0)
    lui t0, 0xc0000
    sb t1, 0(t0)
    /* Deliberately clobber all remaining caller-saved registers. */
    li t2, 77
    li t3, 77
    li t4, 77
    li t5, 77
    li t6, 77
    li a0, 77
    li a1, 77
    li a2, 77
    li a3, 77
    li a4, 77
    li a5, 77
    li a6, 77
    li a7, 77
    ret
.section .bss,"aw",@nobits
.global fixture_bss
fixture_bss:
    .space 16
