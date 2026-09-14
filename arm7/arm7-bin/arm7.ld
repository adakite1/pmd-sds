/* Program begins at the function called _start */
ENTRY(_start)

MEMORY {
    /* Tiny ARM7 static region. crt0 lives here permanently, but other than that,
        after the embedded autoloaded source data is copied by crt0 to the VMA, ARM9
        overlays can overwrite and reuse this 0x0238A140+ region. */
    ARM7_STATIC (rx) : ORIGIN = 0x02380000, LENGTH = 0x27080

    /* 96K contiguous block: 32K shared WRAM mirror (0x037F8000-0x037FFFFF)
        + 64K ARM7-private WRAM (0x03800000-0x0380FFFF).
        That shared lower half only takes effect once ARM9 sets WRAMCNT to share 32K of the WRAM.
        Most of ARM7 is here.
        Also the stack lives at the top and is set by crt0:
        SVC `0x0380FFC0`, IRQ `0x0380FF80`, SYS `0x0380FB7C`
        and the hardcoded IRQ handler pointer is at 0x0380FFFC. */
    ARM7_WRAM (rwx) : ORIGIN = 0x037F8000, LENGTH = 96K

    /* Note that ARM7 does not have any TCM so while 0x027E0000 is
        mapped to TCM in ARM9, here in ARM7 it's just mapped to main RAM (physical 0x023E0000, top 128K of the 4MB main RAM).
        We stop at 0x027FF800 because BIOS boot data is beyond there. */
    ARM7_MAIN (rwx) : ORIGIN = 0x027E0000, LENGTH = 0x1F800
}

SECTIONS {
    .crt0 0x02380000 : {
        KEEP(*(.text.crt0))
    } > ARM7_STATIC

    .module_params : ALIGN(4) {
        __module_params = .;
        LONG(__autoload_table_addr);                         /* autoload_infos_start */
        LONG(__autoload_table_addr + __autoload_table_size); /* autoload_infos_end */
        LONG(__autoload_data);                               /* autoload_blocks (LMA) */
        LONG(__static_data_end);                             /* bss_start (empty) */
        LONG(__static_data_end);                             /* bss_end */
    } > ARM7_STATIC

    /* End of statically stored image (it's not a lot for arm7) */
    __static_data_end = .;
    /* Start of autoloaded blobs */

    /* WRAM AUTOLOAD */

    /* Use #[link_section = "section label like .main.text"] to explicitly place code in the MAIN region instead,
        but otherwise, everything (.text, .rodata, .data, .bss) goes by default into WRAM. */

    /* The SDK's section order: .text, exception table, .init, .rodata,
       .ctor/.sinit, .data, .bss */
    
    .wram.text : ALIGN(4) {
        __wram_start = .;
        *(.wram.text .wram.text.*)
        *(.text .text.*)            /* unattributed code defaults to WRAM */
        *(.glue_7 .glue_7t)
    } > ARM7_WRAM AT> ARM7_STATIC

    /* Empty sentinel to avoid a linker error for .ARM.exidx specifically
        if no unwinding is used (panic = "abort") the section should stay empty. */
    .ARM.exidx : {
        __exidx_start = .;
        *(.ARM.exidx*)
        __exidx_end = .;
    } > ARM7_WRAM AT> ARM7_STATIC

    .init : {
        *(.init)
    } > ARM7_WRAM AT> ARM7_STATIC

    .wram.rodata : ALIGN(4) {
        *(.wram.rodata .wram.rodata.*)
        *(.rodata .rodata.*)        /* unattributed rodata defaults to WRAM */
    } > ARM7_WRAM AT> ARM7_STATIC

    /* This needs to be manually handled by crt0 like .bss (it's an array of function pointers that crt0 has to call one by one),
        though no_std Rustlang usually doesn't use this (no CPP-style static constructors).
        Assuming that we'll be sticking to Rustlang for this section I'm gonna just not add the separate ".wram." prefixed options here. */
    .init_array : ALIGN(4) {
        __init_array_start = .;
        KEEP (*(SORT_BY_INIT_PRIORITY(.init_array.*) SORT_BY_INIT_PRIORITY(.ctors.*)))
        KEEP (*(.init_array EXCLUDE_FILE (*crtbegin*.o *crtbegin?.o *crtend*.o *crtend?.o ) .ctors))
        __init_array_end = .;
    } > ARM7_WRAM AT> ARM7_STATIC

    .wram.data : ALIGN(4) {
        *(.wram.data .wram.data.*)
        *(.data .data.*)            /* unattributed data defaults to WRAM */
        *(.sdata .sdata.*)
        . = ALIGN(4);
        __wram_end = .;
    } > ARM7_WRAM AT> ARM7_STATIC

    /* .bss (no data, should be zeroed by crt0) */
    .wram.bss (NOLOAD) : ALIGN(4) {
        __wram_bss_start = .;
        *(.wram.bss .wram.bss.*)
        *(.bss .bss.* COMMON)      /* unattributed bss defaults to WRAM */
        *(.sbss .sbss.*)
        . = ALIGN(4);
        __wram_bss_end = .;
    } > ARM7_WRAM

    /* MAIN AUTOLOAD */

    .main.text : ALIGN(4) {
        __main_start = .;
        *(.main.text .main.text.*)
    } > ARM7_MAIN AT> ARM7_STATIC

    .main.rodata : ALIGN(4) {
        *(.main.rodata .main.rodata.*)
    } > ARM7_MAIN AT> ARM7_STATIC

    .main.data : ALIGN(4) {
        *(.main.data .main.data.*)
        . = ALIGN(4);
        __main_end = .;
    } > ARM7_MAIN AT> ARM7_STATIC

    .main.bss (NOLOAD) : ALIGN(4) {
        __main_bss_start = .;
        *(.main.bss .main.bss.*)
        . = ALIGN(4);
        __main_bss_end = .;
    } > ARM7_MAIN

    /* THE AUTOLOAD TABLE
        Each entry is composed of: { dest_addr, stored_size, bss_size }
        Order: Do WRAM first, and then MAIN.
    */
    .autoload_table : ALIGN(4) {
        /* Entry 0: WRAM */
        LONG(__wram_start);
        LONG(__wram_end - __wram_start);
        LONG(__wram_bss_end - __wram_bss_start);

        /* Entry 1: MAIN */
        LONG(__main_start);
        LONG(__main_end - __main_start);
        LONG(__main_bss_end - __main_bss_start);
    } > ARM7_STATIC AT> ARM7_STATIC  /* force explicit, AT> above is sticky */

    __autoload_data = LOADADDR(.wram.text); /* first source blob byte (LMA) */
    __autoload_table_addr = LOADADDR(.autoload_table); /* where is the table */
    __autoload_table_size = SIZEOF(.autoload_table);

    __arm7_wram_clear_start = MAX(__static_data_end, 0x03800000);
    __arm7_wram_clear_end = 0x0380ff00; /* 0x100 bytes are reserved as headroom at the top of the WRAM area where there are some IRQ-related and other contents */

    /DISCARD/ : {
        *(.ARM.extab*)
        *(.eh_frame*)
        *(.comment)
        *(.note*)
    }
}