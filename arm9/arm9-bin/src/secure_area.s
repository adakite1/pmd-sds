@ a => "allocatable" and part of the loaded image when running
@ x => "executable" code
@ %progbits => "Program Bits", telling the assembler and linker this section has actual data (as opposed to say .bss)
.section .secure_area, "ax", %progbits

.arm
.align 2

.global __secure_area_start
__secure_area_start:
    .word 0xE7FFDEFF
    .word 0xE7FFDEFF
    .word 0xE7FFDEFF
    @+0x0C

    @@@@ unused
    @@@ .ascii "encryObj" @ this will have to be encrypted later on by the packing script. Fun fact: .asciz appends a null terminator!
    @@@@+0x08

.thumb
.thumb_func
.global SVC_WaitByLoop
SVC_WaitByLoop:
    svc 3
    bx lr

.arm
    .pool

    .equ __secure_used, . - __secure_area_start @ current_assembly_address - __secure_area_start = how many bytes so far
    .equ __secure_padding_words, (0x800 - __secure_used) / 4 @ fill the rest with anything; here, just an undefined instruction so if we accidentally jump here we 'error'

    .fill __secure_padding_words, 4, 0xE7FFDEFF

.global __secure_area_end
__secure_area_end: