//! ULCMS: Rust core with C ABI exports (no third-party crates).
//! Exposes mean, std dev (population), and median for f64 slices.

use core::ffi::{c_double, c_int};
use std::panic::{catch_unwind, AssertUnwindSafe};

#[inline]
fn mean(xs: &[f64]) -> f64 {
    let sum: f64 = xs.iter().copied().sum();
    sum / (xs.len() as f64)
}

#[inline]
fn std_population(xs: &[f64]) -> f64 {
    let mu = mean(xs);
    let mut acc = 0.0f64;
    for &x in xs {
        let d = x - mu;
        acc += d * d;
    }
    (acc / (xs.len() as f64)).sqrt()
}

#[inline]
fn median(xs: &[f64]) -> f64 {
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        let a = v[n / 2 - 1];
        let b = v[n / 2];
        (a + b) * 0.5
    }
}

// ---- C ABI: mean ----

#[unsafe(no_mangle)]
pub extern "C" fn ulcms_mean_f64(ptr: *const c_double, len: usize, out: *mut c_double) -> c_int {
    if ptr.is_null() || out.is_null() {
        return 1;
    }
    if len == 0 {
        return 3;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let xs = unsafe { core::slice::from_raw_parts(ptr, len) };
        mean(xs)
    }));
    match res {
        Ok(m) => {
            unsafe {
                *out = m;
            }
            0
        }
        Err(_) => 2,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ulcms_mean_f64_r(
    ptr: *const c_double,
    len_ptr: *const c_int,
    out: *mut c_double,
) -> c_int {
    if ptr.is_null() || out.is_null() || len_ptr.is_null() {
        return 1;
    }
    let len = unsafe { *len_ptr } as usize;
    ulcms_mean_f64(ptr, len, out)
}

// ---- C ABI: std dev (population) ----

#[unsafe(no_mangle)]
pub extern "C" fn ulcms_std_f64(ptr: *const c_double, len: usize, out: *mut c_double) -> c_int {
    if ptr.is_null() || out.is_null() {
        return 1;
    }
    if len == 0 {
        return 3;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let xs = unsafe { core::slice::from_raw_parts(ptr, len) };
        std_population(xs)
    }));
    match res {
        Ok(s) => {
            unsafe {
                *out = s;
            }
            0
        }
        Err(_) => 2,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ulcms_std_f64_r(
    ptr: *const c_double,
    len_ptr: *const c_int,
    out: *mut c_double,
) -> c_int {
    if ptr.is_null() || out.is_null() || len_ptr.is_null() {
        return 1;
    }
    let len = unsafe { *len_ptr } as usize;
    ulcms_std_f64(ptr, len, out)
}

// ---- C ABI: median ----

#[unsafe(no_mangle)]
pub extern "C" fn ulcms_median_f64(ptr: *const c_double, len: usize, out: *mut c_double) -> c_int {
    if ptr.is_null() || out.is_null() {
        return 1;
    }
    if len == 0 {
        return 3;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let xs = unsafe { core::slice::from_raw_parts(ptr, len) };
        median(xs)
    }));
    match res {
        Ok(med) => {
            unsafe {
                *out = med;
            }
            0
        }
        Err(_) => 2,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ulcms_median_f64_r(
    ptr: *const c_double,
    len_ptr: *const c_int,
    out: *mut c_double,
) -> c_int {
    if ptr.is_null() || out.is_null() || len_ptr.is_null() {
        return 1;
    }
    let len = unsafe { *len_ptr } as usize;
    ulcms_median_f64(ptr, len, out)
}

/* ---------------------------
   Helpers for WASM interop
----------------------------*/

#[unsafe(no_mangle)]
pub extern "C" fn ulcms_alloc(size: usize) -> *mut u8 {
    if size == 0 {
        return core::ptr::null_mut();
    }
    let mut vec: Vec<u8> = Vec::with_capacity(size);
    let ptr = vec.as_mut_ptr();
    core::mem::forget(vec);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn ulcms_free(ptr: *mut u8, size: usize) {
    if ptr.is_null() || size == 0 {
        return;
    }
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, size);
    }
}
