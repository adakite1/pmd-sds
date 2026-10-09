//! Top 0x40 bytes of DTCM reserved for SDK and BIOS uses. The arm7 version is in its WRAM.
//!
//! Most constants here are ordered top-down by address within that 0x40 bytes slice.

pub const DTCM_TOP: u32 = 0x027E4000; //NOTE: This should be the same value as the __dtcm_top value defined in the arm9 linker script, make sure to keep it up to date

pub const IRQ_HANDLER_POINTER_ARM9: u32 = DTCM_TOP - 4;

/// After the handler finishes its work, it sets this. DMA and timer callbacks have this done for them by [`crate::os::OSi_DynamicIrqCallback`], while other handlers do this by themselves after finishing work.
///
/// It's similar to a futex. It is a word in user memory,
/// with a kernel wait queue (a single wait queue here called OSi_IrqThreadQueue) keyed
/// by that word's address on a futex but notably *not* keyed by bit here by the SDK so every interrupt wakes every waiter, who each have to re-check their own bits and usually goes back to sleep,
/// where a waiter checks the value in memory (the word in futexes, if not ready, sleep; while here, test bits & mask, and if clear, OS_SleepThread),
/// and the waker changes the value in memory (the word in futexes, then FUTEX_WAKE; while here, the handler sets its bit, then ReturnFromInterrupt wakes the queue),
/// and finally waiters looping, re-checking on every wakeup (where here, OS_WaitIrq's loop re-tests after every wakeup too).
///
/// A few more differences are that first off, the bit is sticky (goddamn manual reset events), meaning it stays set until it is cleared. An interrupt that happens before the wait still counts.
/// There are also consistently delay bugs:
/// - `SWI IntrWait`'s r0 = 0 ('no discard') argument is supposed to mean that if a selected bit is already set, it should return immediately without waiting. However, this is
/// broken here because on the ARM9 BIOS, that call halts *before* it checks the flags. It only re-checks after an interrupt of any kind wakes it again. So the wake is always delayed by at
/// least one interrupt interval if the bit was already set.
/// - `FUTEX_WAIT` checks the value and enqueues the thread atomically in the kernel (a spinlock on a queue bucket, and setting itself to task-is-sleeping before unlocking the spinlock and calling schedule, so a waker will always see it in the queue after getting the spinlock and immediately set the task to running again)
/// so it always wakes immediately and no wakeups are missed.
/// Here on the NDS, we only have to worry about IRQ interrupting our code, not really massive paralleism. However, for some reason, OS_WaitIrq disables Irqs at first, but then
/// turns it *back on before calling OS_SleepThread to enqueue itself, letting Irqs potentially run before it enqueues itself*.
pub const IRQ_FUTEX_BITS_ARM9: u32 = DTCM_TOP - 8;

