/* Byte-wise C ABI fallbacks, independently sectioned for linker GC.
 * Weak symbols permit a toolchain's own implementation to override them. */
.option norvc
.option norelax
.section .text.memcpy,"ax",@progbits
.weak memcpy
.type memcpy,@function
memcpy:
    mv t0, a0
    beqz a2, 2f
1:
    lbu t1, 0(a1)
    sb t1, 0(t0)
    addi a1, a1, 1
    addi t0, t0, 1
    addi a2, a2, -1
    bnez a2, 1b
2:
    ret
.size memcpy, .-memcpy
.section .text.memset,"ax",@progbits
.weak memset
.type memset,@function
memset:
    mv t0, a0
    beqz a2, 2f
1:
    sb a1, 0(t0)
    addi t0, t0, 1
    addi a2, a2, -1
    bnez a2, 1b
2:
    ret
.size memset, .-memset
.section .text.memmove,"ax",@progbits
.weak memmove
.type memmove,@function
memmove:
    bgeu a1, a0, memcpy
    sub t0, a0, a1
    bgeu t0, a2, memcpy
    add t0, a0, a2
    add t1, a1, a2
1:
    addi t0, t0, -1
    addi t1, t1, -1
    lbu t2, 0(t1)
    sb t2, 0(t0)
    addi a2, a2, -1
    bnez a2, 1b
    ret
.size memmove, .-memmove
.section .text.memcmp,"ax",@progbits
.weak memcmp
.type memcmp,@function
memcmp:
    mv t0, a0
    li a0, 0
    beqz a2, 3f
1:
    lbu t1, 0(t0)
    lbu t2, 0(a1)
    bne t1, t2, 2f
    addi t0, t0, 1
    addi a1, a1, 1
    addi a2, a2, -1
    bnez a2, 1b
    ret
2:
    sub a0, t1, t2
3:
    ret
.size memcmp, .-memcmp
