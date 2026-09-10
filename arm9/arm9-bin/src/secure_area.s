@ a => "allocatable" and part of the loaded image when running
@ x => "executable" code
@ %progbits => "Program Bits", telling the assembler and linker this section has actual data (as opposed to say .bss)
.section .secure_area, "ax", %progbits

.arm
.align 2
@ arm rather than thumb for code here, aligning to 2^2=4 bytes

.global __secure_area_start
__secure_area_start:
    .ascii "encryObj" @ this will have to be encrypted later on by the packing script. Fun fact: .asciz appends a null terminator!
    @+0x08

.global SVC_WaitByLoop
SVC_WaitByLoop:
    subs r0, r0, #1 @ r0 is loop counter, also compares so we can check if r0 is still > 0
    bgt SVC_WaitByLoop
    bx lr

    .pool

    .equ __secure_used, . - __secure_area_start @ current_assembly_address - __secure_area_start = how many bytes so far
    .rept (0x800 - __secure_used) / 4
    .word 0xE7FFDEFF @ fill the rest with anything; here, just an undefined instruction so if we accidentally jump here we 'error'
    .endr

.global __secure_area_end
__secure_area_end: