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

/*(asm-corpus-filename: arm9/main/SVC_SoftReset, objdump-symbol-name: arm9_bin/SVC_SoftReset)DONE('svc')*/
.thumb_func
.global SVC_SoftReset
SVC_SoftReset:
    svc 0
    bx lr
.size SVC_SoftReset, . - SVC_SoftReset

/* RegisterRamReset and HaltGba are GBA-only. Skipped. */

/*(asm-corpus-filename: arm9/main/SVC_WaitByLoop, objdump-symbol-name: arm9_bin/SVC_WaitByLoop)DONE('svc')*/
.thumb_func
.global SVC_WaitByLoop
SVC_WaitByLoop:
    svc 3
    bx lr
.size SVC_WaitByLoop, . - SVC_WaitByLoop

/*(asm-corpus-filename: arm9/main/sub_02000472, objdump-symbol-name: arm9_bin/SVC_IntrWait)DONE('svc')*/
.thumb_func
.global SVC_IntrWait
SVC_IntrWait:
    svc 4
    bx lr
.size SVC_IntrWait, . - SVC_IntrWait

/*(asm-corpus-filename: arm9/main/sub_02000716, objdump-symbol-name: arm9_bin/SVC_VBlankIntrWait)DONE('svc')*/
.thumb_func
.global SVC_VBlankIntrWait
SVC_VBlankIntrWait:
    svc 5
    bx lr
.size SVC_VBlankIntrWait, . - SVC_VBlankIntrWait

/*(asm-corpus-filename: arm9/main/sub_020002E6, objdump-symbol-name: arm9_bin/SVC_Halt)DONE('svc')*/
.thumb_func
.global SVC_Halt
SVC_Halt:
    svc 6
    bx lr
.size SVC_Halt, . - SVC_Halt

/*(asm-corpus-filename: arm9/main/sub_0200013E, objdump-symbol-name: arm9_bin/SVC_Div)DONE('svc')*/
.thumb_func
.global SVC_Div
SVC_Div:
    svc 9
    bx lr
.size SVC_Div, . - SVC_Div

/*(asm-corpus-filename: arm9/main/sub_020004DA, objdump-symbol-name: arm9_bin/SVC_Mod)DONE('svc')*/
.thumb_func
.global SVC_Mod
SVC_Mod:
    svc 9
    adds r0, r1, #0
    bx lr
.size SVC_Mod, . - SVC_Mod

/*(asm-corpus-filename: arm9/main/SVC_CpuSet, objdump-symbol-name: arm9_bin/SVC_CpuSet)DONE('svc')*/
.thumb_func
.global SVC_CpuSet
SVC_CpuSet:
    svc 11
    bx lr
.size SVC_CpuSet, . - SVC_CpuSet

/*(asm-corpus-filename: arm9/main/sub_020000EA, objdump-symbol-name: arm9_bin/SVC_CpuFastSet)DONE('svc')*/
.thumb_func
.global SVC_CpuFastSet
SVC_CpuFastSet:
    svc 12
    bx lr
.size SVC_CpuFastSet, . - SVC_CpuFastSet

/*(asm-corpus-filename: arm9/main/sub_02000386, objdump-symbol-name: arm9_bin/SVC_Sqrt)DONE('svc')*/
.thumb_func
.global SVC_Sqrt
SVC_Sqrt:
    svc 13
    bx lr
.size SVC_Sqrt, . - SVC_Sqrt

/*(asm-corpus-filename: arm9/main/sub_020003F0, objdump-symbol-name: arm9_bin/SVC_CRC16)DONE('svc')*/
.thumb_func
.global SVC_CRC16
SVC_CRC16:
    svc 14
    bx lr
.size SVC_CRC16, . - SVC_CRC16

/*(asm-corpus-filename: arm9/main/sub_0200020C, objdump-symbol-name: arm9_bin/SVC_IsMainMemoryExtended)DONE('svc')*/
.thumb_func
.global SVC_IsMainMemoryExtended
SVC_IsMainMemoryExtended:
    svc 15
    bx lr
.size SVC_IsMainMemoryExtended, . - SVC_IsMainMemoryExtended

/*(asm-corpus-filename: arm9/main/sub_020006A6, objdump-symbol-name: arm9_bin/SVC_BitUnPack)DONE('svc')*/
.thumb_func
.global SVC_BitUnPack
SVC_BitUnPack:
    svc 16
    bx lr
.size SVC_BitUnPack, . - SVC_BitUnPack

/* name is about cb vs direct ldr from memory, the read used is `ldrb` */
/*(asm-corpus-filename: arm9/main/sub_020005D8, objdump-symbol-name: arm9_bin/SVC_LZ77UnCompLdrReadWriteBytes)DONE('svc')*/
.thumb_func
.global SVC_LZ77UnCompLdrReadWriteBytes
SVC_LZ77UnCompLdrReadWriteBytes:
    svc 17
    bx lr
.size SVC_LZ77UnCompLdrReadWriteBytes, . - SVC_LZ77UnCompLdrReadWriteBytes

/*(asm-corpus-filename: arm9/main/sub_02000194, objdump-symbol-name: arm9_bin/SVC_LZ77UnCompCbReadWriteHalfwords)DONE('svc')*/
.thumb_func
.global SVC_LZ77UnCompCbReadWriteHalfwords
SVC_LZ77UnCompCbReadWriteHalfwords:
    svc 18
    bx lr
.size SVC_LZ77UnCompCbReadWriteHalfwords, . - SVC_LZ77UnCompCbReadWriteHalfwords

/*(asm-corpus-filename: arm9/main/sub_02000274, objdump-symbol-name: arm9_bin/SVC_HuffmanUnCompCbReadWriteWords)DONE('svc')*/
.thumb_func
.global SVC_HuffmanUnCompCbReadWriteWords
SVC_HuffmanUnCompCbReadWriteWords:
    svc 19
    bx lr
.size SVC_HuffmanUnCompCbReadWriteWords, . - SVC_HuffmanUnCompCbReadWriteWords

/* name is about cb vs direct ldr from memory, the read used is `ldrb` */
/*(asm-corpus-filename: arm9/main/sub_02000324, objdump-symbol-name: arm9_bin/SVC_RLUnCompLdrReadWriteBytes)DONE('svc')*/
.thumb_func
.global SVC_RLUnCompLdrReadWriteBytes
SVC_RLUnCompLdrReadWriteBytes:
    svc 20
    bx lr
.size SVC_RLUnCompLdrReadWriteBytes, . - SVC_RLUnCompLdrReadWriteBytes

/*(asm-corpus-filename: arm9/main/sub_0200064A, objdump-symbol-name: arm9_bin/SVC_RLUnCompCbReadWriteHalfwords)DONE('svc')*/
.thumb_func
.global SVC_RLUnCompCbReadWriteHalfwords
SVC_RLUnCompCbReadWriteHalfwords:
    svc 21
    bx lr
.size SVC_RLUnCompCbReadWriteHalfwords, . - SVC_RLUnCompCbReadWriteHalfwords

.arm
.align 2 /* on ARM as, = 2^2 bytes = 4 byte align */
    .pool

    .equ __secure_used, . - __secure_area_start @ current_assembly_address - __secure_area_start = how many bytes so far
    .equ __secure_padding_words, (0x800 - __secure_used) / 4 @ fill the rest with anything; here, just an undefined instruction so if we accidentally jump here we 'error'

    .fill __secure_padding_words, 4, 0xE7FFDEFF

.global __secure_area_end
__secure_area_end: