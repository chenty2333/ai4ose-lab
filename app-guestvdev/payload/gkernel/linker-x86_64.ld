/* Linker script for bare-metal x86_64 guest payload */
/* Loaded by the hypervisor at guest GPA 0x10000 */

ENTRY(_start)

SECTIONS
{
    . = 0x10000;

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
