/* Program begins at the function called _start */
ENTRY(_start)

MEMORY {
    /* Tiny ARM7 static region. crt0 lives here permanently, but other than that,
        after the embedded autoloaded source data is copied by crt0 to the VMA, ARM9
        overlays can overwrite and reuse this 0x0238A140+ region. */
    ARM7_STATIC (rx) : ORIGIN = 0x02380000, LENGTH = 0x27080

    /* 96K contiguous block: 32K shared WRAM mirror (0x037F8000-0x037FFFFF)
        + 64K ARM7-private WRAM (0x03800000-0x0380FFFF).
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
    __arm7_static_base = ORIGIN(ARM7_STATIC);
    __arm7_wram_base = ORIGIN(ARM7_WRAM);
    __arm7_main_base = ORIGIN(ARM7_MAIN);

    .crt0 0x02380000 : {
        *(SORT_BY_NAME(.text.origaddr.0238*))
        KEEP(*(.text.crt0))
    } > ARM7_STATIC
    ASSERT(_start == 0x02380000, "_start must be at 0x02380000")

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

    /* The SDK's section order: .text, exception table, .init, .rodata,
       .ctor/.sinit, .data, .bss */
    
    .text.wram : ALIGN(4) {
        __wram_start = .;
        *(SORT_BY_NAME(.text.wram.origaddr.*))
        *(.text.wram .text.wram.*)
        *(.text._ZN4core* .text.unlikely._ZN4core*)
        *libcompiler_builtins-*.rlib:*(.text .text.*)
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

    .rodata.wram : ALIGN(4) {
        *(SORT_BY_NAME(.rodata.wram.origaddr.*))
        *(.rodata.wram .rodata.wram.*)
        *(.rodata._ZN4core*)
        *libcompiler_builtins-*.rlib:*(.rodata .rodata.*)
        *(.rodata..L* .rodata.str* .rodata.cst*)
    } > ARM7_WRAM AT> ARM7_STATIC

    /* This needs to be manually handled by crt0 like .bss (it's an array of function pointers that crt0 has to call one by one),
        though no_std Rustlang usually doesn't use this (no CPP-style static constructors).
        Assuming that we'll be sticking to Rustlang for this section I'm gonna just not add the separate ".*.wram." prefixed options here. */
    .init_array : ALIGN(4) {
        __init_array_start = .;
        KEEP (*(SORT_BY_INIT_PRIORITY(.init_array.*) SORT_BY_INIT_PRIORITY(.ctors.*)))
        KEEP (*(.init_array EXCLUDE_FILE (*crtbegin*.o *crtbegin?.o *crtend*.o *crtend?.o ) .ctors))
        __init_array_end = .;
    } > ARM7_WRAM AT> ARM7_STATIC

    .data.wram : ALIGN(4) {
        *(SORT_BY_NAME(.data.wram.origaddr.*))
        *(.data.wram .data.wram.*)
        *(.data._ZN4core*)
        *libcompiler_builtins-*.rlib:*(.data .data.*)
        . = ALIGN(4);
        __wram_end = .;
    } > ARM7_WRAM AT> ARM7_STATIC

    /* .bss (no data, should be zeroed by crt0) */
    .bss.wram (NOLOAD) : ALIGN(4) {
        __wram_bss_start = .;
        *(SORT_BY_NAME(.bss.wram.origaddr.*))
        *(.bss.wram .bss.wram.*)
        *(.bss._ZN4core*)
        *libcompiler_builtins-*.rlib:*(.bss .bss.*)
        . = ALIGN(4);
        __wram_bss_end = .;
    } > ARM7_WRAM

    /* MAIN AUTOLOAD */

    .text.main : ALIGN(4) {
        __main_start = .;
        *(SORT_BY_NAME(.text.main.origaddr.*))
        *(.text.main .text.main.*)
    } > ARM7_MAIN AT> ARM7_STATIC

    .rodata.main : ALIGN(4) {
        *(SORT_BY_NAME(.rodata.main.origaddr.*))
        *(.rodata.main .rodata.main.*)
    } > ARM7_MAIN AT> ARM7_STATIC

    .data.main : ALIGN(4) {
        *(SORT_BY_NAME(.data.main.origaddr.*))
        *(.data.main .data.main.*)
        . = ALIGN(4);
        __main_end = .;
    } > ARM7_MAIN AT> ARM7_STATIC

    .bss.main (NOLOAD) : ALIGN(4) {
        __main_bss_start = .;
        *(SORT_BY_NAME(.bss.main.origaddr.*))
        *(.bss.main .bss.main.*)
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

    __autoload_data = LOADADDR(.text.wram); /* first source blob byte (LMA) */
    __autoload_table_addr = LOADADDR(.autoload_table); /* where is the table */
    __autoload_table_size = SIZEOF(.autoload_table);

    __reg_base = 0x04000000;

    __arm7_wram_clear_start = MAX(__autoload_data, 0x03800000);
    __arm7_wram_clear_end = 0x0380ff00; /* 0x100 bytes are reserved as headroom at the top of the WRAM area where there are some IRQ-related and other contents */

    __wram_top = ORIGIN(ARM7_WRAM) + LENGTH(ARM7_WRAM); /* 0x03810000 */
    /* layout: [reserved 0x40][SVC: 0x40 bytes][IRQ: 0x400 bytes][System: stack grows further downward] */
    __svc_stack_top = __wram_top - 0x40; /* 0x03810000 - 0x40 = 0x380FFC0 */
    __irq_stack_top = __wram_top - 0x80; /* 0x03810000 - 0x80 = 0x380FF80 */
    __irq_stack_size = 0x400;
    __sys_stack_size = 0x400;
    __system_stack_pointer = __irq_stack_top - __irq_stack_size - 4; /* 0x380FF80 - 0x400 - 4 = 0x380FB7C */
    /* alignment: [svc align 8, irq align 8, sys align 4 only] */
    __irq_handler_pointer = __wram_top - 4; /* 0x03810000 - 4 = 0x380FFFC */
    ASSERT(__wram_bss_end <= __system_stack_pointer - __sys_stack_size, "WRAM data/bss overlaps the stacks")

    .debug_abbrev 0 : { *(.debug_abbrev) }
    .debug_addr 0 : { *(.debug_addr) }
    .debug_aranges 0 : { *(.debug_aranges) }
    .debug_frame 0 : { *(.debug_frame) }
    .debug_info 0 : { *(.debug_info) }
    .debug_line 0 : { *(.debug_line) }
    .debug_line_str 0 : { *(.debug_line_str) }
    .debug_loc 0 : { *(.debug_loc) }
    .debug_loclists 0 : { *(.debug_loclists) }
    .debug_macinfo 0 : { *(.debug_macinfo) }
    .debug_macro 0 : { *(.debug_macro) }
    .debug_names 0 : { *(.debug_names) }
    .debug_pubnames 0 : { *(.debug_pubnames) }
    .debug_pubtypes 0 : { *(.debug_pubtypes) }
    .debug_ranges 0 : { *(.debug_ranges) }
    .debug_rnglists 0 : { *(.debug_rnglists) }
    .debug_str 0 : { *(.debug_str) }
    .debug_str_offsets 0 : { *(.debug_str_offsets) }
    .debug_types 0 : { *(.debug_types) }
    .ARM.attributes 0 : { KEEP(*(.ARM.attributes)) }
    .symtab 0 : { *(.symtab) }
    .strtab 0 : { *(.strtab) }
    .shstrtab 0 : { *(.shstrtab) }

    .code_labels 0 : { KEEP(*(.code_labels)) }

    /DISCARD/ : {
        *(.ARM.extab*)
        *(.eh_frame*)
        *(.comment)
        *(.note*)
    }
}