# wrappers/r/test.R

# --- locate repo root (has "artifacts") ---
args <- commandArgs(trailingOnly = FALSE)
file <- sub("^--file=", "", grep("^--file=", args, value = TRUE))
this_dir <- if (length(file)) dirname(normalizePath(file)) else getwd()

find_root <- function(start) {
  cur <- normalizePath(start, mustWork = FALSE)
  for (i in 1:6) {
    if (file.exists(file.path(cur, "artifacts"))) return(cur)
    parent <- normalizePath(file.path(cur, ".."), mustWork = FALSE)
    if (parent == cur) break
    cur <- parent
  }
  stop("Could not locate repo root containing 'artifacts' from: ", start)
}
root <- find_root(this_dir)

# --- resolve native lib path for this OS/arch ---
sys  <- Sys.info()[["sysname"]]
arch <- tolower(Sys.info()[["machine"]]); if (arch == "aarch64") arch <- "arm64"
label <- switch(sys, "Darwin"="macos", "Linux"="linux", "Windows"="windows", stop("Unsupported OS: ", sys))
fname <- switch(sys, "Darwin"="libulcms.dylib", "Linux"="libulcms.so", "Windows"="ulcms.dll")
libpath <- file.path(root, "artifacts", paste0(label, "-", arch), fname)
if (!file.exists(libpath)) stop("Build first: ", libpath, "\nRun: make ", label, "-", arch)

# --- load native lib and API ---
dyn.load(libpath)
on.exit(try(dyn.unload(libpath), silent = TRUE), add = TRUE)
source(file.path(root, "wrappers", "r", "R", "ulcms.R"))

# --- data + compute ---
x <- as.double(c(1, 2, 3, 4))
m <- ulcms_mean(x)
s <- ulcms_std(x)       # population σ
d <- ulcms_median(x)

# --- show results ---
cat(sprintf("x:        %s\n", paste(x, collapse = ", ")))
cat(sprintf("mean:     %.12g\n", m))
cat(sprintf("std (pop):%.12g\n", s))
cat(sprintf("median:   %.12g\n", d))

# --- checks (all.equal) ---
ok_mean   <- isTRUE(all.equal(m, 2.5))
pop_sd    <- sd(x) * sqrt((length(x)-1)/length(x))
ok_std    <- isTRUE(all.equal(s, pop_sd))
ok_median <- isTRUE(all.equal(d, median(x)))

cat("\nChecks:\n")
cat(sprintf("  mean   : %s\n", if (ok_mean) "OK ✓" else "FAIL ✗"))
cat(sprintf("  std    : %s\n", if (ok_std) "OK ✓" else "FAIL ✗"))
cat(sprintf("  median : %s\n", if (ok_median) "OK ✓" else "FAIL ✗"))

if (!(ok_mean && ok_std && ok_median)) quit(status = 1) else cat("\nALL TESTS PASSED ✅\n")
