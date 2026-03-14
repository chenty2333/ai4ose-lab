#![no_std]

use axallocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator};
use core::alloc::Layout;
use core::ptr::NonNull;

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const SIZE: usize> {
    start: usize,
    end: usize,
    b_pos: usize,
    p_pos: usize,
    count: usize,
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
            count: 0,
        }
    }

    const fn align_down(pos: usize, align: usize) -> usize {
        pos & !(align - 1)
    }

    const fn align_up(pos: usize, align: usize) -> usize {
        (pos + align - 1) & !(align - 1)
    }

    fn page_lower_bound(&self) -> usize {
        Self::align_up(self.b_pos, SIZE)
    }

    fn page_upper_bound(&self) -> usize {
        Self::align_down(self.p_pos, SIZE)
    }
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        assert!(SIZE.is_power_of_two());
        self.start = start;
        self.end = start + size;
        self.b_pos = start;
        self.p_pos = start + size;
        self.count = 0;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        if size == 0 {
            return Ok(());
        }
        if self.start == self.end {
            self.init(start, size);
            return Ok(());
        }
        if start == self.end && self.p_pos == self.end {
            self.end += size;
            self.p_pos = self.end;
            return Ok(());
        }
        Err(AllocError::InvalidParam)
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let start = Self::align_up(self.b_pos, layout.align());
        let end = start
            .checked_add(layout.size())
            .ok_or(AllocError::InvalidParam)?;
        if end > self.p_pos {
            return Err(AllocError::NoMemory);
        }
        self.b_pos = end;
        self.count += 1;
        Ok(NonNull::new(start as *mut u8).unwrap_or_else(NonNull::dangling))
    }

    fn dealloc(&mut self, _pos: NonNull<u8>, _layout: Layout) {
        if self.count == 0 {
            return;
        }
        self.count -= 1;
        if self.count == 0 {
            self.b_pos = self.start;
        }
    }

    fn total_bytes(&self) -> usize {
        self.p_pos.saturating_sub(self.start)
    }

    fn used_bytes(&self) -> usize {
        self.b_pos.saturating_sub(self.start)
    }

    fn available_bytes(&self) -> usize {
        self.p_pos.saturating_sub(self.b_pos)
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;

    fn alloc_pages(
        &mut self,
        num_pages: usize,
        align_pow2: usize,
    ) -> AllocResult<usize> {
        if num_pages == 0
            || !align_pow2.is_power_of_two()
            || align_pow2 < SIZE
            || align_pow2 % SIZE != 0
        {
            return Err(AllocError::InvalidParam);
        }

        let size = num_pages
            .checked_mul(SIZE)
            .ok_or(AllocError::InvalidParam)?;
        let lower = self.page_lower_bound();
        let upper = self.page_upper_bound();
        if upper < lower + size {
            return Err(AllocError::NoMemory);
        }

        let candidate = Self::align_down(upper - size, align_pow2);
        if candidate < lower {
            return Err(AllocError::NoMemory);
        }
        self.p_pos = candidate;
        Ok(candidate)
    }

    fn alloc_pages_at(
        &mut self,
        base: usize,
        num_pages: usize,
        align_pow2: usize,
    ) -> AllocResult<usize> {
        if num_pages == 0
            || !align_pow2.is_power_of_two()
            || align_pow2 < SIZE
            || align_pow2 % SIZE != 0
            || base % align_pow2 != 0
        {
            return Err(AllocError::InvalidParam);
        }

        let size = num_pages
            .checked_mul(SIZE)
            .ok_or(AllocError::InvalidParam)?;
        let lower = self.page_lower_bound();
        let upper = self.page_upper_bound();
        let candidate = Self::align_down(
            upper.checked_sub(size).ok_or(AllocError::NoMemory)?,
            align_pow2,
        );
        if base != candidate || base < lower {
            return Err(AllocError::NoMemory);
        }
        self.p_pos = base;
        Ok(base)
    }

    fn dealloc_pages(&mut self, pos: usize, num_pages: usize) {
        let _ = (pos, num_pages);
    }

    fn total_pages(&self) -> usize {
        Self::align_down(self.end, SIZE)
            .saturating_sub(Self::align_up(self.start, SIZE))
            / SIZE
    }

    fn used_pages(&self) -> usize {
        Self::align_down(self.end, SIZE).saturating_sub(Self::align_down(self.p_pos, SIZE)) / SIZE
    }

    fn available_pages(&self) -> usize {
        self.page_upper_bound()
            .saturating_sub(self.page_lower_bound())
            / SIZE
    }
}
