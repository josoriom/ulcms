use std::fs;
use std::time::Instant;
use ulcms::utilities::parse_mzml::{SpectrumSummary, parse_mzml};

fn main() {
    let path = "/Users/josoriom/github/josoriom/ulcms/core/data/iron_ultrairon_SER_MS-AI-RPNEG@fNMR_IROr26_IROp011_LTR_30.mzML";

    let data = fs::read(path).expect("read file");
    let start = Instant::now();
    match parse_mzml(&data) {
        Ok(spectra) => {
            let elapsed = start.elapsed();
            println!("Processing took: {:.3?}", elapsed);

            if spectra.is_empty() {
                println!("No spectra found.");
                return;
            }
            let s = &spectra[0];
            println!("{}", to_json(s, 10));
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}

fn to_json(s: &SpectrumSummary, preview: usize) -> String {
    fn fmt_vec(v: &Option<Vec<f64>>, preview: usize) -> String {
        match v {
            None => "null".to_string(),
            Some(arr) => {
                let shown = arr
                    .iter()
                    .take(preview)
                    .map(|x| format!("{}", x))
                    .collect::<Vec<_>>()
                    .join(", ");
                if arr.len() > preview {
                    format!("({}) [{} ...]", arr.len(), shown)
                } else {
                    format!("({}) [{}]", arr.len(), shown)
                }
            }
        }
    }
    fn opt_f64(v: Option<f64>) -> String {
        v.map(|x| {
            if (x.fract()).abs() < 1e-12 {
                format!("{}", x.round() as i64)
            } else {
                format!("{}", x)
            }
        })
        .unwrap_or_else(|| "null".into())
    }
    fn opt_str(s: Option<&str>) -> String {
        match s {
            None => "null".into(),
            Some(v) => format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")),
        }
    }

    format!(
        r#"{{
  "arrayLength": {a},
  "basePeakIntensity": {bpi},
  "basePeakMZ": {bpmz},
  "id": "{id}",
  "index": {i},
  "intensityArray": {int},
  "msLevel": {ms},
  "mzArray": {mz},
  "polarity": {pol},
  "retentionTime": {rt},
  "scanType": {st},
  "scanWindowLowerLimit": {lw},
  "scanWindowUpperLimit": {up},
  "totalIonCurrent": {tic},
  "type": {ty}
}}"#,
        a = s.array_length,
        bpi = opt_f64(s.base_peak_intensity),
        bpmz = opt_f64(s.base_peak_mz),
        id = s.id.replace('\\', "\\\\").replace('"', "\\\""),
        i = s.index,
        int = fmt_vec(&s.intensity_array, preview),
        ms = s
            .ms_level
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".into()),
        mz = fmt_vec(&s.mz_array, preview),
        pol = opt_str(s.polarity.as_deref()),
        rt = opt_f64(s.retention_time),
        st = opt_str(s.scan_type.as_deref()),
        lw = opt_f64(s.scan_window_lower_limit),
        up = opt_f64(s.scan_window_upper_limit),
        tic = opt_f64(s.total_ion_current),
        ty = opt_str(s.spectrum_type.as_deref()),
    )
}
