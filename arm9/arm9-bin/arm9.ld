/* Program begins at the function called _start */
ENTRY(_start)

MEMORY {
    /* NDS main RAM location (4 megabytes) */
    MAIN_RAM (rwx) : ORIGIN = 0x02000000, LENGTH = 4M

    ITCM (rwx) : ORIGIN = 0x01FF8000, LENGTH = 16K
    /* The full hardware ITCM is 32K, but here we're using 16K only since the upper half is used as a runtime arena (heap) */

    DTCM (rw) : ORIGIN = 0x027E0000, LENGTH = 16K
}

SECTIONS {
    __arm7_wram_base = 0x037F8000 /* same as ORIGIN(ARM7_WRAM) from arm7.ld */;
    /* In this game, ARM7 is built with the assumption that it will have all the WRAM given to it (WRAMCNT = 3),
        which can be seen in ARM7_WRAM in arm7.ld where you see ARM7_WRAM is the full 96K, with the whole 32K part
        inside. WRAMCNT = 1 would make a big gap where ARM7 places code and data that is just gone (16 KiB arm7,
        then that same 16 KiB gets mirrored from arm7's perspective, then arm7 WRAM). WRAMCNT = 2 has the same issue,
        ARM9 gets the first 16 KiB, then ARM7 sees a mirror of it's upper 16 KiB of compiled data in the lower 16 KiB
        address space instead. Either way, the built code would completely break.
        This seems to be the sdk default, MI_Init calls MI_SetWramBank(MI_WRAM_ARM7_ALL) unconditionally on the ARM9,
        inside OS_Init, and then nothing else in the game writes to WRAMCNT again.
        Nothing else touches 0x04000247 directly, and no one else calls MI_SetWramBank either. Which makes
        sense since again, that would break the ARM7 code's build assumptions.
        
        This also means that Arena 6 is empty. */

    __main_ram_base = ORIGIN(MAIN_RAM);
    __itcm_base = ORIGIN(ITCM);
    __itcm_heap_start = __itcm_base + LENGTH(ITCM); /* read as + 0x4000 */
    __itcm_heap_end = __itcm_base + 0x8000;
    __dtcm_base = ORIGIN(DTCM);

    __main_ram_top = ORIGIN(MAIN_RAM) + LENGTH(MAIN_RAM); /* 0x02400000 */
    __main_ram_top_arm9 = __main_ram_top - 0x20000; /* 0x023E0000 */
    /* The top 0x20000 bytes of the lower Main RAM mirror consist of 0x4000 bytes of
        ARM7's private Main RAM (shadowed by DTCM on the upper mirror), 0x1b000 bytes of more of ARM7's private Main RAM, and 0x1000 bytes
        of System and SDK memory, all of which are not owned by the ARM9 game code. */

    /* First 0x800 bytes are the secure area. At this point has "encryObj" marker,
        then packing script encrypts it later, and BIOS decrypts it again at boot. */
    .secure_area 0x02000000 : {
        KEEP(*(.secure_area))
    } > MAIN_RAM
    ASSERT(. == 0x02000800, "secure area must be exactly 0x800 bytes!")

    /* .text, SDK order should be: crt0, game code, libs */
    .text 0x02000800 : {
        *(SORT_BY_NAME(.text.origaddr.*))
        KEEP(*(.text.crt0))
        *(.text .text.[!i]* .text.i .text.i[!t]* .text.it .text.it[!c]* .text.itc .text.itc[!m]*
          .text.itcm[!.]*)
        *(.glue_7 .glue_7t) /* ARM-to-Thumb and Thumb-to-ARM interworking helper code */
    } > MAIN_RAM
    ASSERT(_start == 0x02000800, "_start must be at 0x02000800")

    /* Empty sentinel to avoid a linker error for .ARM.exidx specifically
        if no unwinding is used (panic = "abort") the section should stay empty. */
    .ARM.exidx : {
        __exidx_start = .;
        *(.ARM.exidx*)
        __exidx_end = .;
    } > MAIN_RAM

    .init : {
        *(.init)
    } > MAIN_RAM

    .rodata : ALIGN(4) {
        *(SORT_BY_NAME(.rodata.origaddr.*))
        *(.rodata .rodata.[!di]* .rodata.i .rodata.i[!t]* .rodata.it .rodata.it[!c]* .rodata.itc
          .rodata.itc[!m]* .rodata.itcm[!.]* .rodata.d .rodata.d[!t]* .rodata.dt .rodata.dt[!c]* .rodata.dtc
          .rodata.dtc[!m]* .rodata.dtcm[!.]*)
    } > MAIN_RAM

    /* This needs to be manually handled by crt0 like .bss (it's an array of function pointers that crt0 has to call one by one),
        though no_std Rustlang usually doesn't use this (no CPP-style static constructors). */
    .init_array : ALIGN(4) {
        __init_array_start = .;
        KEEP (*(SORT_BY_INIT_PRIORITY(.init_array.*) SORT_BY_INIT_PRIORITY(.ctors.*)))
        KEEP (*(.init_array EXCLUDE_FILE (*crtbegin*.o *crtbegin?.o *crtend*.o *crtend?.o ) .ctors))
        __init_array_end = .;
    } > MAIN_RAM

    /* initialized statics */
    .data : ALIGN(4) {
        *(SORT_BY_NAME(.data.origaddr.*))
        *(.data .data.[!di]* .data.i .data.i[!t]* .data.it .data.it[!c]* .data.itc .data.itc[!m]*
          .data.itcm[!.]* .data.d .data.d[!t]* .data.dt .data.dt[!c]* .data.dtc .data.dtc[!m]* .data.dtcm[!.]*)
        *(.sdata .sdata.*)
    } > MAIN_RAM

    .build_info : ALIGN(4) {
        __build_info = .;
        LONG(__autoload_table_addr);                         /* autoload_infos_start */
        LONG(__autoload_table_addr + __autoload_table_size); /* autoload_infos_end */
        LONG(__autoload_data);                               /* autoload_blocks (LMA) */
        LONG(__static_data_end);                             /* bss_start, but this is not entirely the same as __bss_start below, this bss_start is the SDK one that means the start of the module's post-static-data area. */
        LONG(__bss_end);                                     /* bss_end */
        LONG(0);                                             /* compressed_code_ptr (nothing is compressed) */
        LONG(0x04027534);                                    /* sdk_version */
        LONG(0xDEC00621);                                    /* nitrocode */
        LONG(0x2106C0DE);                                    /* nitrocode_rev */
    } > MAIN_RAM

    /* End of statically stored image */
    __static_data_end = .;

    /* Start of autoloaded blobs */

    /* For ease of understanding:
        VMA (Virtual Memory Address) is where the code will actually run.
        LMA (Load Memory Address) is where the data remains *before* it gets run.
        For the code to run, something has to copy it from the LMA to the VMA.
        "> ITCM AT> MAIN_RAM" says that the code executes in the ITCM section (VMA),
        but the initial bytes are stored in the main RAM at first before crt0 (C RunTime zero)
        copies it over for running.
    */

    .text.itcm : ALIGN(4) {
        /* crt0 should copy the range from LMA... */
        __itcm_start = .; /* ...to __itcm_start..__itcm_end (VMA), before calling any ITCM-resident code. */
        *(SORT_BY_NAME(.text.itcm.origaddr.*))
        *(.itcm .text.itcm .text.itcm.*)
    } > ITCM AT> MAIN_RAM

    .rodata.itcm : ALIGN(4) {
        *(SORT_BY_NAME(.rodata.itcm.origaddr.*))
        *(.rodata.itcm .rodata.itcm.*)
    } > ITCM AT> MAIN_RAM

    .data.itcm : ALIGN(4) {
        *(SORT_BY_NAME(.data.itcm.origaddr.*))
        *(.data.itcm .data.itcm.*)
        . = ALIGN(4);
        __itcm_end = .;
    } > ITCM AT> MAIN_RAM

    .bss.itcm (NOLOAD) : ALIGN(4) {
        __itcm_bss_start = .;
        *(SORT_BY_NAME(.bss.itcm.origaddr.*))
        *(.bss.itcm .bss.itcm.*)
        . = ALIGN(4);
        __itcm_bss_end = .;
    } > ITCM

    .rodata.dtcm : ALIGN(4) {
        __dtcm_start = .;
        *(SORT_BY_NAME(.rodata.dtcm.origaddr.*))
        *(.rodata.dtcm .rodata.dtcm.*)
    } > DTCM AT> MAIN_RAM

    .data.dtcm : ALIGN(4) {
        *(SORT_BY_NAME(.data.dtcm.origaddr.*))
        *(.dtcm .data.dtcm .data.dtcm.*)
        . = ALIGN(4);
        __dtcm_end = .;
    } > DTCM AT> MAIN_RAM

    .bss.dtcm (NOLOAD) : ALIGN(4) {
        __dtcm_bss_start = .;
        *(SORT_BY_NAME(.bss.dtcm.origaddr.*))
        *(.bss.dtcm .bss.dtcm.*)
        . = ALIGN(4);
        __dtcm_bss_end = .;
    } > DTCM

    /* THE AUTOLOAD TABLE
        Each entry is composed of: { dest_addr, stored_size, bss_size }
    */
    .autoload_table : ALIGN(4) {
        /* Entry 0: ITCM */
        LONG(__itcm_start);
        LONG(__itcm_end - __itcm_start);
        LONG(__itcm_bss_end - __itcm_bss_start);

        /* Entry 1: DTCM */
        LONG(__dtcm_start);
        LONG(__dtcm_end - __dtcm_start);
        LONG(__dtcm_bss_end - __dtcm_bss_start);
    } > MAIN_RAM AT> MAIN_RAM /* force both to use MAIN_RAM, AT> above is sticky */

    __autoload_data = LOADADDR(.text.itcm); /* where does the first source blob start (LMA start) */
    __autoload_table_addr = LOADADDR(.autoload_table); /* where is the table */
    __autoload_table_size = SIZEOF(.autoload_table);

    __reg_base = 0x04000000;
    __palette_ram_base = 0x05000000;
    __vram_base = 0x06000000;
    __oam_base = 0x07000000;

    __dtcm_clear_size = 0x4000;
    __palette_ram_clear_size = 0x400;
    __oam_fill_size = 0x400;

    __dtcm_top = ORIGIN(DTCM) + LENGTH(DTCM); /* 0x027E4000 */
    /* layout: [reserved 0x40][SVC: 0x40 bytes][IRQ: 0x800 bytes][System: stack grows further downward] */
    __svc_stack_top = __dtcm_top - 0x40; /* 0x027E4000 - 0x40 = 0x27E3FC0 */
    __svc_irq_stack_boundary = __dtcm_top - 0x80; /* 0x027E4000 - 0x80 = 0x27E3F80 */
    __irq_stack_top = __svc_irq_stack_boundary - 4; /* 0x027E4000 - 0x80 - 4 = 0x27E3F7C, importantly: pre-compensate for AAPCS too */
    __irq_stack_size = 0x800;
    __system_stack_pointer = __svc_irq_stack_boundary - __irq_stack_size - 4; /* 0x27E3F80 - 0x800 - 4 = 0x27E377C */
    /* alignment: [svc align 8, irq align 4 only, sys align 4 only] */
    __irq_handler_pointer = __dtcm_top - 4; /* 0x027E4000 - 4 = 0x27E3FFC */
    ASSERT(__dtcm_bss_end <= __system_stack_pointer, "DTCM data/bss overlaps the stacks")

    /* This "multifunctional" size parameter lets you either set the share of space given to
        the System stack or the extra heap arena, with a signed value. The system stack grows
        downward, but it might not grow down *so* much, so some of that space from the stack
        bottom, down to the end of DTCM's whole autoloaded fixed block (including after bss), in between
        those two, you can shove a heap in there.
        > 0: the stack gets exactly that many bytes, and the arena gets the leftovers.
        < 0: reversed, where the arena gets |multifunctional_size| bytes, and the stack gets the leftovers.
        == 0: give everything to the stack (clamped so it's at least __dtcm_base, which does nothing, and I don't know
          why the sdk code does this).
         */
    __system_stack_size_multifunctional = 0;

    /* Bottom of the IRQ stack = top of the system stack region */
    __irq_stack_bottom = __svc_irq_stack_boundary - __irq_stack_size; /* 0x27E3F80 - 0x800 = 0x27E3780 */
    __system_stack_top_real_without_anti_aapcs_align = __irq_stack_bottom;

    /* rust-lld evaluates expressions as unsigned 64-bit values, meaning x < 0 = false always at all times. Instead we
        test bit 31 instead and separate out the sign and magnitude of the multifunctional size. */
    __sys_stack_sz_neg = (__system_stack_size_multifunctional & 0x80000000) != 0;
    __sys_stack_sz_abs = (__sys_stack_sz_neg
                            ? (0 - __system_stack_size_multifunctional)
                            : __system_stack_size_multifunctional) & 0x7FFFFFFF;

    /* Arena 4 (DTCM heap) settings generated from above */

    __additional_heap_space_in_dtcm_lo = __dtcm_bss_end;
    /* The lowest you can go is the autoloaded DTCM data's end point
        because the heap should not accidentally overwrite autoloaded DTCM
        data. */

    __additional_heap_space_in_dtcm_hi =
        (__system_stack_size_multifunctional == 0)
            ? MAX(__additional_heap_space_in_dtcm_lo, __dtcm_base) /* == 0 */
            : (__sys_stack_sz_neg
                ? (__additional_heap_space_in_dtcm_lo + __sys_stack_sz_abs)                 /* < 0 */
                : (__system_stack_top_real_without_anti_aapcs_align - __sys_stack_sz_abs)); /* > 0 */

    __system_stack_bottom = __additional_heap_space_in_dtcm_hi;
    __system_stack_size = __system_stack_top_real_without_anti_aapcs_align - __system_stack_bottom;
    __additional_heap_space_in_dtcm = __additional_heap_space_in_dtcm_hi - __additional_heap_space_in_dtcm_lo;

    ASSERT(__additional_heap_space_in_dtcm_hi >= __additional_heap_space_in_dtcm_lo,
        "system stack size too large, overlaps DTCM fixed region")
    ASSERT(__additional_heap_space_in_dtcm_hi <= __irq_stack_bottom,
        "additional heap arena in DTCM has a size set too large, overlaps IRQ stack")

    .static_footer : ALIGN(4) { /* when building the ROM, arm9_size should be set to exclude this footer (image_len - 12 bytes) */
        LONG(0xDEC00621);
        LONG(__build_info - 0x02000000);
        LONG(0);
    } > MAIN_RAM AT> MAIN_RAM

    /* .bss (no data, should be zeroed by crt0); at the end since the bss sections should all be at
        the end of their memory sections to avoid having a big gap in the final flat image. The
        per-memory-section LMA is advanced no matter what, but bss at the end means that this can't
        shift anything after it and create that empty space. */
    /* implementing autoload like the original crt0 means a gap the size of the LMA part that remain unused
        by any code for .bss, but since I'd like to avoid -Clink-arg=--no-check-sections which is needed to let the bss
        actually overlap the LMA section, keeping it like this for now. */
    .bss (NOLOAD) : ALIGN(4) {
        __bss_start = .;
        *(SORT_BY_NAME(.bss.origaddr.*))
        *(.bss .bss.* COMMON)
        *(.sbss .sbss.*)
        . = ALIGN(4);
        __bss_end = .;
    } > MAIN_RAM

    /DISCARD/ : {
        *(.ARM.extab*)
        *(.eh_frame*)
        *(.comment)
        *(.note*)
    }
}