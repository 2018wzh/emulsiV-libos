.option norvc
.section .text,"ax",@progbits
.global __libos_irq_handler
__libos_irq_handler:
    lui t0, 0xb0000
    sb zero, 0(t0)
    lui t0, 0xc0000
    li t1, 33
    sb t1, 0(t0)
    ret
