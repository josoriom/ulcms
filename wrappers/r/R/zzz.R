# R/zzz.R — load prebuilt shared library from inst/libs/<platform>/ and cache addresses

.ulcms_state <- new.env(parent = emptyenv())

.ulcms_platform <- function() {
  sys  <- tolower(Sys.info()[["sysname"]])
  mach <- tolower(Sys.info()[["machine"]])

  # Optional override for testing:
  plat_override <- Sys.getenv("ULCMS_PLATFORM", unset = "")
  if (nzchar(plat_override)) return(plat_override)

  if (sys == "darwin") {
    if (mach %in% c("arm64", "aarch64")) "macos-arm64" else "macos-x86_64"
  } else if (sys == "linux") {
    if (mach %in% c("arm64", "aarch64")) "linux-arm64" else "linux-x86_64"
  } else if (sys == "windows") {
    "windows-x86_64"  # add "windows-arm64" when you ship it
  } else {
    stop(sprintf("Unsupported platform: %s-%s", sys, mach), call. = FALSE)
  }
}

.ulcms_ext <- function() {
  sys <- tolower(Sys.info()[["sysname"]])
  if (sys == "darwin") "dylib"
  else if (sys == "linux") "so"
  else if (sys == "windows") "dll"
  else stop(sprintf("Unsupported OS: %s", sys), call. = FALSE)
}

.ulcms_lib_filename <- function() {
  ext <- .ulcms_ext()
  if (.Platform$OS.type == "windows") "ulcms.dll" else sprintf("libulcms.%s", ext)
}

.ulcms_find_binary <- function(pkgname) {
  # Optional absolute override for dev:
  override <- Sys.getenv("ULCMS_LIB_PATH", unset = "")
  if (nzchar(override)) {
    if (!file.exists(override))
      stop(sprintf("ULCMS_LIB_PATH set but file not found: %s", override), call. = FALSE)
    return(normalizePath(override, winslash = "/", mustWork = TRUE))
  }

  plat    <- .ulcms_platform()
  libfile <- .ulcms_lib_filename()
  full <- system.file("libs", plat, libfile, package = pkgname)
  if (!nzchar(full) || !file.exists(full)) {
    stop(sprintf(
      "Missing prebuilt native library at inst/libs/%s/%s (installed as: %s).",
      plat, libfile, full
    ), call. = FALSE)
  }
  normalizePath(full, winslash = "/", mustWork = TRUE)
}

.onLoad <- function(libname, pkgname) {
  path <- .ulcms_find_binary(pkgname)

  # Keep symbols local; we’ll resolve from this DLL explicitly
  dll <- dyn.load(path, local = TRUE, now = TRUE)

  # Save DLL info for diagnostics
  .ulcms_state$dll      <- dll
  .ulcms_state$dll_name <- dll[["name"]]
  .ulcms_state$libpath  <- path

  # Cache function addresses (robust against symbol visibility issues)
  .ulcms_state$addr_mean   <- getNativeSymbolInfo("ulcms_mean_f64_r",  PACKAGE = dll)$address
  .ulcms_state$addr_std    <- getNativeSymbolInfo("ulcms_std_f64_r",   PACKAGE = dll)$address
  .ulcms_state$addr_median <- getNativeSymbolInfo("ulcms_median_f64_r", PACKAGE = dll)$address
}

.onUnload <- function(libpath) {
  p <- tryCatch(.ulcms_state$libpath, error = function(e) NULL)
  if (!is.null(p)) try(dyn.unload(p), silent = TRUE)
}
