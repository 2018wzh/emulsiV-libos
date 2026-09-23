# Heap design and safety / 堆设计与安全

## Memory ownership

The `heap` feature is optional. Without it, the library does not register a global allocator.
On the target, the linker exports a 16-byte-aligned heap start and a fixed heap end at the stack bottom.
The global allocator initializes this extent on its first use.
The allocator state belongs to BSS and is included in the static image budget.

The allocation granule is 16 bytes. A 160-bit bitmap can describe at most 2560 bytes.
The actual heap is smaller because code, constants, and BSS also occupy RAM.
The allocator state is 36 bytes on RV32I. There is no header in an allocated payload.
A request rounds up to whole allocation units.

`Heap<'a>` instead holds an exclusive borrow of a caller-supplied buffer.
Its capacity excludes leading alignment padding and a partial final unit.
It limits the extent to `MAX_CAPACITY`.
Do not let a returned pointer outlive that heap or buffer.

## Allocation and release

Allocation scans the bitmap for the first aligned free run that satisfies the request.
It returns null, or `None` in the borrowed API, when the request cannot fit.
A zero-sized request to the borrowed API returns `None`.
Rust allocation APIs handle zero-sized types according to their own contracts.

Release marks the allocation units free. It does not clear their bytes.
Use `allocate_zeroed` or `alloc_zeroed` when cleared payload bytes are required.
Adjacent free units automatically form a larger free run.
Separated free runs can still cause external fragmentation.

The raw release API is unsafe. Supply the original live pointer and its exact `Layout`.
Do not release a block twice. Do not read or write a block after release.
The allocator does not store a separate object header to diagnose violations of these contracts.

Reallocation first allocates disjoint storage with the original alignment.
It copies the shorter payload length and then releases the old block.
Failure leaves the old block valid and unchanged.
Zero size returns failure in the borrowed API.
Reallocation does not currently grow or shrink in place.
Its temporary space requirement can cause failure even when an in-place operation could succeed.

## Failure and diagnostics

Use fallible container reservation when a program must recover from OOM.
`Vec::try_reserve_exact` and `String::try_reserve_exact` report allocation failure.
A raw allocation reports failure with a null pointer.
An infallible allocation can enter the panic handler, which prints `panic` and halts.

The statistics report capacity, used space, free space, the largest free run, and the peak used space.
Used and free bytes are granule-based quantities. They are not exact user payload lengths.
The largest free run does not guarantee that every alignment can use that whole run.
Statistics are snapshots, not reservations.

## Interrupt serialization

Stock Virgule has one hart and two supported device interrupt sources.
The global allocator masks both sources before it borrows mutable allocator state.
It restores the source masks after the borrow ends.
No user callback, allocation, or unwinding occurs inside that state operation.
An interrupt before both masks are stored runs before the mutable state borrow starts.

The allocator does not use atomics, CSR instructions, or a spinlock.
It is not portable to a multi-hart target or a target with additional unmasked interrupt sources.
Allocation, release, and statistics scan at most 160 units, but reallocation also copies the payload.
Interrupts remain masked during this work.
Avoid heap operations in interrupt callbacks when bounded service latency is important.
Device events can still be lost at the hardware latch while interrupts are masked.

## Tested behaviors and remaining limits

The host tests cover alignment, zeroing, exhaustion, fragmentation, adjacent reuse, and failed reallocation.
They also verify 10000 reuse cycles and 5000 pseudo-random allocation operations with live-block canaries.
The five target examples cover Box, Vec growth, String, raw allocation, release, and OOM recovery.
Both CPU engines run those target examples.

These tests are not a formal Rust memory-safety proof or an exhaustive interrupt-interleaving proof.
There is no hardware stack guard, virtual memory, or process isolation.
Keep the default 512-byte stack unless a separate worst-case analysis justifies another partition.

## 中文要点

`heap` 默认关闭。目标端全局堆位于静态镜像之后、预留栈之前，首次使用时初始化。
分配以 16 字节为单位，位图状态占 36 字节。代码和 BSS 会减少实际堆容量。
释放后的单位可以复用，相邻空闲单位可以合并满足更大的请求，但碎片仍然可能导致失败。

`Heap<'a>` 独占借用用户缓冲区。返回的原始指针不能超过堆或缓冲区的生命周期。
释放必须提供原始有效指针和准确的 `Layout`，不能重复释放，也不能释放后继续访问。
普通释放不清零内容。需要清零时，应使用清零分配接口。

重新分配采用“先分配、再复制、最后释放”的方式，需要额外临时空间。
失败时原块和内容保持不变，不支持原地扩缩容。
可恢复 OOM 应通过 `try_reserve_exact` 或检查原始分配返回值处理。
不可恢复的分配失败可能触发 panic 并停止程序。

全局堆操作在借用内部状态前屏蔽两个已知设备中断源，借用结束后恢复掩码。
此方法只针对原版单 hart 平台，不是通用的多核锁，也不保证输入事件无损。
现有测试不等于穷举中断交错或形式化内存安全证明。
