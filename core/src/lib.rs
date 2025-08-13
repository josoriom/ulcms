//! ULCMS: Rust core with C ABI exports (no third-party crates).

use core::ffi::{c_char, c_double, c_int};
use std::ffi::{CStr, CString};
use std::panic::{AssertUnwindSafe, catch_unwind};

pub mod utilities;

use utilities::parse_mzml::{SpectrumSummary, parse_mzml};

#[inline]
fn mean(xs: &[f64]) -> f64 {
    let sum: f64 = xs.iter().copied().sum();
    sum / (xs.len() as f64)
}

#[inline]
fn std(xs: &[f64]) -> f64 {
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
        std(xs)
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

#[repr(C)]
pub struct SpectrumSummaryFFI {
    pub index: usize,
    pub id: *mut c_char,
    pub array_length: usize,
    pub ms_level: u32,
    pub scan_type: *mut c_char,
    pub polarity: *mut c_char,
    pub spectrum_type: *mut c_char,
    pub retention_time: f64,
    pub scan_window_lower_limit: f64,
    pub scan_window_upper_limit: f64,
    pub total_ion_current: f64,
    pub base_peak_intensity: f64,
    pub base_peak_mz: f64,
    pub mz_array: *mut f64,
    pub mz_array_len: usize,
    pub intensity_array: *mut f64,
    pub intensity_array_len: usize,
}

fn str_opt_to_c(opt: Option<String>) -> *mut c_char {
    match opt {
        Some(s) => CString::new(s).unwrap().into_raw(),
        None => core::ptr::null_mut(),
    }
}

fn vecf64_opt_to_raw_box(opt: Option<Vec<f64>>) -> (*mut f64, usize) {
    match opt {
        Some(v) => {
            let boxed: Box<[f64]> = v.into_boxed_slice();
            let len = boxed.len();
            let ptr = Box::into_raw(boxed) as *mut f64;
            (ptr, len)
        }
        None => (core::ptr::null_mut(), 0),
    }
}

impl From<SpectrumSummary> for SpectrumSummaryFFI {
    fn from(s: SpectrumSummary) -> Self {
        let (mz_ptr, mz_len) = vecf64_opt_to_raw_box(s.mz_array);
        let (int_ptr, int_len) = vecf64_opt_to_raw_box(s.intensity_array);
        SpectrumSummaryFFI {
            index: s.index,
            id: CString::new(s.id).unwrap().into_raw(),
            array_length: s.array_length,
            ms_level: s.ms_level.unwrap_or(0),
            scan_type: str_opt_to_c(s.scan_type),
            polarity: str_opt_to_c(s.polarity),
            spectrum_type: str_opt_to_c(s.spectrum_type),
            retention_time: s.retention_time.unwrap_or(f64::NAN),
            scan_window_lower_limit: s.scan_window_lower_limit.unwrap_or(f64::NAN),
            scan_window_upper_limit: s.scan_window_upper_limit.unwrap_or(f64::NAN),
            total_ion_current: s.total_ion_current.unwrap_or(f64::NAN),
            base_peak_intensity: s.base_peak_intensity.unwrap_or(f64::NAN),
            base_peak_mz: s.base_peak_mz.unwrap_or(f64::NAN),
            mz_array: mz_ptr,
            mz_array_len: mz_len,
            intensity_array: int_ptr,
            intensity_array_len: int_len,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ulcms_parse_mzml(
    path: *const c_char,
    out_ptr: *mut *mut SpectrumSummaryFFI,
    out_len: *mut usize,
) -> c_int {
    if path.is_null() || out_ptr.is_null() || out_len.is_null() {
        return 1;
    }

    let res = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
        let cstr = unsafe { CStr::from_ptr(path) };
        let path_str = cstr.to_str().map_err(|_| "invalid UTF-8".to_string())?;

        let spectra = parse_mzml(path_str)?; // Vec<SpectrumSummary>

        let buf: Box<[SpectrumSummaryFFI]> = spectra
            .into_iter()
            .map(SpectrumSummaryFFI::from)
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let len = buf.len();
        let ptr = Box::into_raw(buf) as *mut SpectrumSummaryFFI;

        unsafe {
            *out_ptr = ptr;
            *out_len = len;
        }
        Ok(())
    }));

    match res {
        Ok(Ok(())) => 0,
        Ok(Err(_msg)) => 4,
        Err(_) => 2,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ulcms_free_spectra(ptr: *mut SpectrumSummaryFFI, len: usize) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let slice = std::slice::from_raw_parts_mut(ptr, len);

        for it in slice.iter_mut() {
            if !it.id.is_null() {
                let _ = CString::from_raw(it.id);
                it.id = core::ptr::null_mut();
            }
            if !it.scan_type.is_null() {
                let _ = CString::from_raw(it.scan_type);
                it.scan_type = core::ptr::null_mut();
            }
            if !it.polarity.is_null() {
                let _ = CString::from_raw(it.polarity);
                it.polarity = core::ptr::null_mut();
            }
            if !it.spectrum_type.is_null() {
                let _ = CString::from_raw(it.spectrum_type);
                it.spectrum_type = core::ptr::null_mut();
            }
            if !it.mz_array.is_null() {
                let _ = Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                    it.mz_array,
                    it.mz_array_len,
                ));
                it.mz_array = core::ptr::null_mut();
                it.mz_array_len = 0;
            }
            if !it.intensity_array.is_null() {
                let _ = Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                    it.intensity_array,
                    it.intensity_array_len,
                ));
                it.intensity_array = core::ptr::null_mut();
                it.intensity_array_len = 0;
            }
        }

        let _ = Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ulcms_alloc(size: usize) -> *mut u8 {
    if size == 0 {
        return core::ptr::null_mut();
    }
    let mut v: Vec<u8> = Vec::with_capacity(size);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
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
