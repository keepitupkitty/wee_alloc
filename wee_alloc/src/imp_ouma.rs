use super::AllocErr;
use const_init::ConstInit;
use core::cell::UnsafeCell;
use core::ffi::c_int;
use core::ffi::c_long;
use core::ffi::c_void;
use core::ptr;
use syscalls::*;
use memory_units::{Bytes, Pages};

pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;
pub const MAP_PRIVATE: c_int = 0x0002;
pub const MAP_FAILED: *mut c_void = !0 as *mut c_void;

#[cfg(target_arch = "aarch64")]
pub const MAP_ANON: c_int = 0x0020;

#[cfg(target_arch = "x86_64")]
pub const MAP_ANON: c_int = 0x0020;

unsafe fn internal_mmap(
    addr: *mut c_void,
    len: usize,
    prot: c_int,
    flags: c_int,
    fd: c_int,
    off: c_long,
) -> *mut c_void {
    raw_syscall!(Sysno::mmap, addr, len, prot, flags, fd, off) as *mut c_void
}

pub(crate) fn alloc_pages(pages: Pages) -> Result<ptr::NonNull<u8>, AllocErr> {
    unsafe {
        let bytes: Bytes = pages.into();
        let addr = internal_mmap(
            ptr::null_mut(),
            bytes.0,
            PROT_WRITE | PROT_READ,
            MAP_ANON | MAP_PRIVATE,
            -1,
            0,
        );
        if addr == MAP_FAILED {
            Err(AllocErr)
        } else {
            ptr::NonNull::new(addr as *mut u8).ok_or(AllocErr)
        }
    }
}

// Align to the cache line size on an i7 to prevent false sharing.
#[repr(align(64))]
pub(crate) struct Exclusive<T> {
    inner: UnsafeCell<T>,
}

impl<T: ConstInit> ConstInit for Exclusive<T> {
    const INIT: Self = Exclusive {
        inner: UnsafeCell::new(T::INIT),
    };
}

impl<T> Exclusive<T> {
    /// Get exclusive, mutable access to the inner value.
    ///
    /// # Safety
    ///
    /// Does not assert that `pthread`s calls return OK, unless the
    /// "extra_assertions" feature is enabled. This means that if `f` re-enters
    /// this method for the same `Exclusive` instance, there will be undetected
    /// mutable aliasing, which is UB.
    #[inline]
    pub(crate) unsafe fn with_exclusive_access<F, U>(&self, f: F) -> U
    where
        for<'x> F: FnOnce(&'x mut T) -> U,
    {
        let result = f(&mut *self.inner.get());
        result
    }
}
