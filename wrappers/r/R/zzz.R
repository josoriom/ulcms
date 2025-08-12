.ulcms_state <- new.env(parent = emptyenv())

.normalize_arch <- function(mach) {
  m <- tolower(mach)
  if (m %in% c("aarch64", "arm64")) return("arm64")
  if (m %in% c("x86_64", "amd64"))  return("x86_64")
  m
}

.os_label <- function(sys) {
  switch(sys,
    "Darwin"  = "macos",
    "Linux"   = "linux",
    "Windows" = "windows",
    stop("Unsupported OS: ", sys)
  )
}

.lib_filename <- function(sys) {
  switch(sys,
    "Darwin"  = "libulcms.dylib",
    "Linux"   = "libulcms.so",
    "Windows" = "ulcms.dll",
    stop("Unsupported OS: ", sys)
  )
}

.find_ulcms_binary <- function(pkgname) {
  sys  <- Sys.info()[["sysname"]]
  arch <- .normalize_arch(Sys.info()[["machine"]])
  base <- system.file("libs", package = pkgname)
  fname <- .lib_filename(sys)
  label <- .os_label(sys)

  cand <- file.path(base, paste0(label, "-", arch), fname)

  if (!file.exists(cand)) {
    hits <- list.files(
      base,
      pattern = "(^|/)ulcms\\.(dll|so)$|(^|/)libulcms\\.dylib$",
      recursive = TRUE, full.names = TRUE, ignore.case = TRUE
    )
    if (length(hits)) cand <- hits[[1]]
  }

  if (!file.exists(cand)) {
    stop("ULCMS native library not found under: ", base,
         "\nExpected: ", file.path(base, paste0(label, "-", arch), fname),
         call. = FALSE)
  }
  cand
}

.onLoad <- function(libname, pkgname) {
  dll <- .find_ulcms_binary(pkgname)
  if (.Platform$OS.type == "windows") {
    Sys.setenv(PATH = paste(unique(c(dirname(dll),
      strsplit(Sys.getenv("PATH"), .Platform$path.sep)[[1]])),
      collapse = .Platform$path.sep))
  }
  dyn.load(dll, local = FALSE)
  assign("dll_path", dll, envir = .ulcms_state)
}

.onUnload <- function(libpath) {
  dll <- get0("dll_path", envir = .ulcms_state, ifnotfound = NA_character_)
  if (is.character(dll) && nzchar(dll) && file.exists(dll)) {
    try(dyn.unload(dll), silent = TRUE)
  }
}
