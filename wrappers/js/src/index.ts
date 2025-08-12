import { makeApi } from "./makeApi.js";

declare const __INLINE__: boolean;
const INLINE = typeof __INLINE__ !== "undefined" && __INLINE__;
declare const __WASM_DATA_URL__: string | undefined;
declare var require: any;

type Api = ReturnType<typeof makeApi>;
export let utilities: Api["utilities"];

function dataUrlToBytes(dataUrl: string): Uint8Array {
  const b64 = dataUrl.split(",")[1] ?? "";
  if (typeof atob === "function") {
    const bin = atob(b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bytes.length; i++) bytes[i] = bin.charCodeAt(i);
    return bytes;
  }
  return Uint8Array.from(Buffer.from(b64, "base64"));
}

function instantiateSync(bytes: Uint8Array, imports: WebAssembly.Imports = {}) {
  const ab = bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength
  );
  const mod = new WebAssembly.Module(ab as ArrayBuffer);
  const instance = new WebAssembly.Instance(mod, imports);
  const api = makeApi(instance);
  utilities = api.utilities;
}

(function main() {
  if (INLINE) {
    const dataUrl = __WASM_DATA_URL__!;
    const bytes = dataUrlToBytes(dataUrl);
    instantiateSync(bytes, {});
    return;
  }

  if (typeof process !== "undefined" && process.versions?.node) {
    if (typeof __dirname !== "undefined") {
      const fs = require("node:fs") as typeof import("node:fs");
      const path = require("node:path") as typeof import("node:path");
      const p = path.join(__dirname, "ulcms.wasm");
      const buf: Buffer = fs.readFileSync(p);
      instantiateSync(new Uint8Array(buf), {});
      return;
    }

    const fs = require("node:fs") as typeof import("node:fs");
    const url = require("node:url") as typeof import("node:url");
    const path = require("node:path") as typeof import("node:path");
    const here = path.dirname(url.fileURLToPath((0, eval)("import.meta").url));
    const p = path.join(here, "ulcms.wasm");
    const buf: Buffer = fs.readFileSync(p);
    instantiateSync(new Uint8Array(buf), {});
    return;
  }

  throw new Error("Browser ESM requires the UMD inline build.");
})();
