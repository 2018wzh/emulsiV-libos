/* Stock emulsiV: code + data + BSS + stack share 3072 bytes.
 * The HEX loader initializes .data in place (VMA == LMA); there is no ROM.
 * Reload HEX before restarting if initialized mutable data must be reset. */
OUTPUT_ARCH(riscv)
ENTRY(__reset)
/* A linker fallback, not an assembler alias to an undefined external symbol. */
PROVIDE(__libos_irq_handler = __libos_default_irq);
MEMORY {
    RAM (rwx) : ORIGIN = 0x00000000, LENGTH = 3K
    BITMAP (rw) : ORIGIN = 0x00000C00, LENGTH = 1K
}
__stack_size = DEFINED(__stack_size_override) ? __stack_size_override : 512;
__stack_top = ORIGIN(RAM) + LENGTH(RAM);
__stack_bottom = __stack_top - __stack_size;
SECTIONS {
    .vectors ORIGIN(RAM) : ALIGN(4) {
        KEEP(*(.vectors))
    } > RAM
    .text : ALIGN(4) {
        *(.text.init)
        *(.text .text.*)
    } > RAM
    .rodata : ALIGN(4) {
        *(.srodata .srodata.* .rodata .rodata.*)
    } > RAM
    .data : ALIGN(4) {
        __data_start = .;
        PROVIDE(__global_pointer$ = . + 0x800);
        *(.sdata .sdata.* .data .data.*)
        . = ALIGN(4);
        __data_end = .;
    } > RAM
    .bss (NOLOAD) : ALIGN(4) {
        __bss_start = .;
        *(.sbss .sbss.* .bss .bss.* COMMON)
        . = ALIGN(4);
        __bss_end = .;
    } > RAM
    __image_end = .;
    __heap_start = ALIGN(__image_end, 16);
    __heap_end = __stack_bottom;
    .stack __stack_bottom (NOLOAD) : {
        . += __stack_size;
    } > RAM
    /DISCARD/ : { *(.eh_frame .eh_frame_hdr) }
}
ASSERT(SIZEOF(.vectors) == 8, "reset and IRQ vectors must be exactly 8 bytes")
ASSERT(__image_end <= __stack_bottom, "firmware exceeds stock 3 KiB RAM budget")
ASSERT(__stack_size >= 128 && (__stack_size & 15) == 0, "stack needs >=128 bytes and 16-byte alignment")
ASSERT(__stack_top <= ORIGIN(BITMAP), "stack overlaps framebuffer")
ASSERT(__heap_start <= __heap_end, "no aligned heap extent before stack")
