//! ULCMS: Rust core with C ABI exports for mzML parsing.

use core::ffi::{c_char, c_int};
use std::ffi::{CStr, CString};
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub mod utilities;

use utilities::parse_mzml::{SpectrumSummary, parse_mzml};

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
        let data = fs::read(path_str).map_err(|e| format!("open/read: {e}"))?;

        let spectra = parse_mzml(&data)?;

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
        Ok(Err(_)) => 4,
        Err(_) => 2,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ulcms_parse_mzml_from_bytes(
    data_ptr: *const u8,
    data_len: usize,
    out_ptr: *mut *mut SpectrumSummaryFFI,
    out_len: *mut usize,
) -> c_int {
    if data_ptr.is_null() || out_ptr.is_null() || out_len.is_null() {
        return 1;
    }

    let res = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
        let data = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
        let spectra = parse_mzml(data)?;

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
        Ok(Err(_)) => 4,
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
