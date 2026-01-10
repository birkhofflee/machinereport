use std::sync::LazyLock;

#[allow(dead_code)]
pub struct Colors {
  pub reset:       &'static str,
  pub bold:        &'static str,
  pub dim:         &'static str,
  pub blue:        &'static str,
  pub cyan:        &'static str,
  pub green:       &'static str,
  pub yellow:      &'static str,
  pub red:         &'static str,
  pub magenta:     &'static str,
  pub white:       &'static str,
  pub bold_blue:   &'static str,
  pub bold_cyan:   &'static str,
  pub bold_green:  &'static str,
  pub bold_yellow: &'static str,
  pub bold_white:  &'static str,
}

impl Colors {
  const fn new(is_no_color: bool) -> Self {
    if is_no_color {
      Self {
        reset:       "",
        bold:        "",
        dim:         "",
        blue:        "",
        cyan:        "",
        green:       "",
        yellow:      "",
        red:         "",
        magenta:     "",
        white:       "",
        bold_blue:   "",
        bold_cyan:   "",
        bold_green:  "",
        bold_yellow: "",
        bold_white:  "",
      }
    } else {
      Self {
        reset:       "\x1b[0m",
        bold:        "\x1b[1m",
        dim:         "\x1b[2m",
        blue:        "\x1b[34m",
        cyan:        "\x1b[36m",
        green:       "\x1b[32m",
        yellow:      "\x1b[33m",
        red:         "\x1b[31m",
        magenta:     "\x1b[35m",
        white:       "\x1b[37m",
        bold_blue:   "\x1b[1;34m",
        bold_cyan:   "\x1b[1;36m",
        bold_green:  "\x1b[1;32m",
        bold_yellow: "\x1b[1;33m",
        bold_white:  "\x1b[1;37m",
      }
    }
  }
}

pub static COLORS: LazyLock<Colors> = LazyLock::new(|| {
  const NO_COLOR: *const libc::c_char = c"NO_COLOR".as_ptr();
  let is_no_color = unsafe { !libc::getenv(NO_COLOR).is_null() };
  Colors::new(is_no_color)
});
