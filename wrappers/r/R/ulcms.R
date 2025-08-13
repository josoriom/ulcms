# R/ulcms.R — call native functions via cached addresses

#' @export
ulcms_mean <- function(x, na.rm = FALSE) {
  x <- as.double(x); if (na.rm) x <- x[!is.na(x)]
  if (!length(x)) return(NA_real_)
  .C(.ulcms_state$addr_mean, x, as.integer(length(x)), out = double(1))$out
}

#' @export
ulcms_std <- function(x, na.rm = FALSE) {
  x <- as.double(x); if (na.rm) x <- x[!is.na(x)]
  if (!length(x)) return(NA_real_)
  .C(.ulcms_state$addr_std, x, as.integer(length(x)), out = double(1))$out
}

#' @export
ulcms_median <- function(x, na.rm = FALSE) {
  x <- as.double(x); if (na.rm) x <- x[!is.na(x)]
  if (!length(x)) return(NA_real_)
  .C(.ulcms_state$addr_median, x, as.integer(length(x)), out = double(1))$out
}
