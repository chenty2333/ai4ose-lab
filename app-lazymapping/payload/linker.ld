ENTRY(_start)
SECTIONS
{
    . = 0;
    .text : {
        KEEP(*(.text._start))
        *(.text .text.*)
    }
    /DISCARD/ : {
        *(.eh_frame*)
        *(.note*)
        *(.comment)
        *(.gnu*)
        *(.ARM.*)
        *(.dynsym)
        *(.dynstr)
        *(.hash)
        *(.dynamic)
        *(.interp)
        *(.got*)
        *(.plt*)
        *(.rela*)
        *(.data*)
        *(.rodata*)
        *(.bss*)
    }
}
