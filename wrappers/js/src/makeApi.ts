// src/makeApi.ts
export interface RawApi {
  utilities: {
    mean: (xs: number[]) => number;
    std: (xs: number[]) => number;
    median: (xs: number[]) => number;
  };
}

type Fn = (ptr: number, len: number, out: number) => number;

export function makeApi(instance: WebAssembly.Instance): RawApi {
  const exp = instance.exports as Record<string, any>;
  const memory = exp.memory as WebAssembly.Memory;
  const alloc = exp.ulcms_alloc as (size: number) => number;
  const free_ = exp.ulcms_free as (ptr: number, size: number) => void;
  const fMean = exp.ulcms_mean_f64 as Fn;
  const fStd = exp.ulcms_std_f64 as Fn;
  const fMed = exp.ulcms_median_f64 as Fn;

  const call1 = (xs: number[], f: Fn): number => {
    const n = xs.length;
    if (n === 0) throw new Error("ULCMS: empty array");
    const inBytes = n * 8;
    const inPtr = alloc(inBytes);
    const outPtr = alloc(8);
    new Float64Array(memory.buffer, inPtr, n).set(xs);
    const rc = f(inPtr, n, outPtr);
    try {
      if (rc !== 0) throw new Error(`ULCMS FFI error (rc=${rc})`);
      return new Float64Array(memory.buffer, outPtr, 1)[0];
    } finally {
      free_(inPtr, inBytes);
      free_(outPtr, 8);
    }
  };

  return {
    utilities: {
      mean: (xs) => call1(xs, fMean),
      std: (xs) => call1(xs, fStd),
      median: (xs) => call1(xs, fMed),
    },
  };
}
