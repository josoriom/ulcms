#' @export
ulcms_mean <- function(x, na.rm = FALSE) {
  x <- as.double(x); if (na.rm) x <- x[!is.na(x)]
  if (!length(x)) return(NA_real_)
  .C("ulcms_mean_f64_r", x, as.integer(length(x)), out = as.double(0))$out
}

#' @export
ulcms_std <- function(x, na.rm = FALSE) {
  x <- as.double(x); if (na.rm) x <- x[!is.na(x)]
  if (!length(x)) return(NA_real_)
  .C("ulcms_std_f64_r", x, as.integer(length(x)), out = as.double(0))$out
}

#' @export
ulcms_median <- function(x, na.rm = FALSE) {
  x <- as.double(x); if (na.rm) x <- x[!is.na(x)]
  if (!length(x)) return(NA_real_)
  .C("ulcms_median_f64_r", x, as.integer(length(x)), out = as.double(0))$out
}
