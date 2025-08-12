export interface RawApi {
  utilities: {
    mean: (xs: number[]) => number;
    std: (xs: number[]) => number;
    median: (xs: number[]) => number;
  };
}

type Fn = (ptr: number, len: number, out: number) => number;

async function loadInstance(): Promise<WebAssembly.Instance> {
  const wasmUrl = new URL("./ulcms.wasm", import.meta.url);
  const isFile = wasmUrl.protocol === "file:";
  if (!isFile && typeof fetch === "function") {
    try {
      if ("instantiateStreaming" in WebAssembly) {
        const res = await fetch(wasmUrl);
        if (!res.ok)
          throw new Error(`Failed to fetch ulcms.wasm: ${res.status}`);
        const { instance } = await WebAssembly.instantiateStreaming(res, {});
        return instance;
      }
    } catch {}
    const res = await fetch(wasmUrl);
    if (!res.ok) throw new Error(`Failed to fetch ulcms.wasm: ${res.status}`);
    const bytes = await res.arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return instance;
  } else {
    const { readFile } = await import("fs/promises");
    const { fileURLToPath } = await import("url");
    const path = fileURLToPath(wasmUrl);
    const buf = await readFile(path);
    const { instance } = await WebAssembly.instantiate(buf, {});
    return instance;
  }
}

export async function loader(): Promise<RawApi> {
  const instance = await loadInstance();
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
