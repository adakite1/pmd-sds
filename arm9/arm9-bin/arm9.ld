/* Program begins at the function called _start */
ENTRY(_start)

MEMORY {
    /* NDS main RAM location (4 megabytes) */
    MAIN_RAM (rwx) : ORIGIN = 0x02000000, LENGTH = 4M

    ITCM (rwx) : ORIGIN = 0x01FF8000, LENGTH = 32K

    DTCM (rw) : ORIGIN = 0x027E0000, LENGTH = 16K
}

/* For ease of understanding:
    VMA (Virtual Memory Address) is where the code will actually run.
    LMA (Load Memory Address) is where the data remains *before* it gets run.
    For the code to run, something has to copy it from the LMA to the VMA.
    "> ITCM AT> MAIN_RAM" says that the code executes in the ITCM section (VMA),
    but the initial bytes are stored in the main RAM at first before crt0 (C RunTime zero)
    copies it over for running.
 */
SECTIONS {
    .text : { *(.text*) } > MAIN_RAM          /* VMA = LMA = 0x02000000+ */
    .data : { *(.data*) } > MAIN_RAM

    .itcm : {
        __itcm_lma = LOADADDR(.itcm); /* crt0 should copy the range from __itcm_lma (LMA)... */
        __itcm_start = .; /* ...to __itcm_start..__itcm_end (VMA), before calling any ITCM-resident code. */
        *(.itcm*)
        __itcm_end = .;
    } > ITCM AT> MAIN_RAM

    .dtcm : {
        __dtcm_lma = LOADADDR(.dtcm);
        __dtcm_start = .;
        *(.dtcm*)
        __dtcm_end = .;
    } > DTCM AT> MAIN_RAM

    /* Empty sentinel to avoid a linker error for .ARM.exidx specifically
        if no unwinding is used (panic = "abort") the section should stay empty. */
    .ARM.exidx : {
        __exidx_start = .;
        *(.ARM.exidx*)
        __exidx_end = .;
    } > MAIN_RAM

    /DISCARD/ {
        *(.ARM.extab*)
        *(.eh_frame*)
        *(.comment)
        *(.note*)
    }
}
