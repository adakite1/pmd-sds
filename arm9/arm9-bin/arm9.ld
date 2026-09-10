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
    /* First 0x800 bytes are the secure area. At this point has "encryObj" marker,
        then packing script encrypts it later, and BIOS decrypts it again at boot. */
    .secure_area 0x02000000 : {
        KEEP(*(.secure_area))
    } > MAIN_RAM
    ASSERT(. == 0x02000800, "secure area must be exactly 0x800 bytes!")

    /* .text, SDK order should be: crt0, game code, libs */
    .text 0x02000800 : {
        KEEP(*(.text.crt0))
        *(.text .text.*)
        *(.glue_7 .glue_7t) /* ARM-to-Thumb and Thumb-to-ARM interworking helper code */
    } > MAIN_RAM

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
        *(.rodata .rodata.*)
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
        *(.data .data.*)
        *(.sdata .sdata.*)
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

    .itcm : ALIGN(4) {
        /* crt0 should copy the range from LMA... */
        __itcm_start = .; /* ...to __itcm_start..__itcm_end (VMA), before calling any ITCM-resident code. */
        *(.itcm .itcm.text .itcm.text.*)
    } > ITCM AT> MAIN_RAM

    .itcm.rodata : ALIGN(4) {
        *(.itcm.rodata .itcm.rodata.*)
    } > ITCM AT> MAIN_RAM

    .itcm.data : ALIGN(4) {
        *(.itcm.data .itcm.data.*)
        . = ALIGN(4);
        __itcm_end = .;
    } > ITCM AT> MAIN_RAM

    .itcm_bss (NOLOAD) : ALIGN(4) {
        __itcm_bss_start = .;
        *(.itcm.bss .itcm.bss.*)
        . = ALIGN(4);
        __itcm_bss_end = .;
    } > ITCM

    .dtcm : ALIGN(4) {
        __dtcm_start = .;
        *(.dtcm .dtcm.text .dtcm.text.*)
    } > DTCM AT> MAIN_RAM

    .dtcm.rodata : ALIGN(4) {
        *(.dtcm.rodata .dtcm.rodata.*)
    } > DTCM AT> MAIN_RAM

    .dtcm.data : ALIGN(4) {
        *(.dtcm.data .dtcm.data.*)
        . = ALIGN(4);
        __dtcm_end = .;
    } > DTCM AT> MAIN_RAM

    .dtcm_bss (NOLOAD) : ALIGN(4) {
        __dtcm_bss_start = .;
        *(.dtcm.bss .dtcm.bss.*)
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

    __autoload_data = LOADADDR(.itcm); /* where does the first source blob start (LMA start) */
    __autoload_table_addr = LOADADDR(.autoload_table); /* where is the table */
    __autoload_table_size = SIZEOF(.autoload_table);

    /* .bss (no data, should be zeroed by crt0); at the end since the bss sections should all be at
        the end of their memory sections to avoid having a big gap in the final flat image. The
        per-memory-section LMA is advanced no matter what, but bss at the end means that this can't
        shift anything after it and create that empty space. */
    .bss (NOLOAD) : ALIGN(4) {
        __bss_start = .;
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