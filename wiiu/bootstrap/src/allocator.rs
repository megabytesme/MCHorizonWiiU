//! WUT's newlib heap is the backing store for Horizon's `alloc` users.
use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
    mem, ptr,
};

unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(pointer: *mut c_void);
}

pub struct CafeAllocator;

unsafe impl GlobalAlloc for CafeAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let alignment = layout.align().max(mem::align_of::<*mut u8>());
        let header_size = mem::size_of::<*mut u8>();
        let Some(allocation_size) = layout
            .size()
            .max(1)
            .checked_add(alignment - 1)
            .and_then(|size| size.checked_add(header_size))
        else {
            return ptr::null_mut();
        };

        let allocation = unsafe { malloc(allocation_size) }.cast::<u8>();
        if allocation.is_null() {
            return ptr::null_mut();
        }

        let Some(aligned) = (allocation as usize)
            .checked_add(header_size)
            .and_then(|start| start.checked_add(alignment - 1))
            .map(|start| start & !(alignment - 1))
        else {
            unsafe { free(allocation.cast::<c_void>()) };
            return ptr::null_mut();
        };
        let result = aligned as *mut u8;
        unsafe { result.sub(header_size).cast::<*mut u8>().write(allocation) };
        result
    }

    unsafe fn dealloc(&self, pointer: *mut u8, _layout: Layout) {
        if !pointer.is_null() {
            let allocation = unsafe {
                pointer
                    .sub(mem::size_of::<*mut u8>())
                    .cast::<*mut u8>()
                    .read()
            };
            unsafe { free(allocation.cast::<c_void>()) };
        }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: CafeAllocator = CafeAllocator;
