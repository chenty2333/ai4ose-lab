/* Linker script for bare-metal aarch64 guest payload */
/* Loaded at EL0 by the hypervisor at guest VA 0x40200000 */

ENTRY(_start)

SECTIONS
{
    . = 0x40200000;

    .text : {
        *(.text._start)
        *(.text .text.*)
    }

    .rodata : ALIGN(8) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(8) {
        *(.data .data.*)
    }

    .bss : ALIGN(8) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
        *(.note*)
    }
}
