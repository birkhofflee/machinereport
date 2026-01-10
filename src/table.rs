use std::fmt::Write;

use crate::colors::COLORS;

/// Configuration for table layout
pub struct TableConfig {
  pub name_width: usize,
  pub data_width: usize,
}

impl TableConfig {
  #[must_use] 
  pub const fn new(name_width: usize, data_width: usize) -> Self {
    Self {
      name_width,
      data_width,
    }
  }

  /// Total width including borders and padding: "│ NAME │ DATA │"
  #[must_use] 
  pub const fn total_width(&self) -> usize {
    // │ + space + name + space + │ + space + data + space + │
    1 + 1 + self.name_width + 1 + 1 + 1 + self.data_width + 1 + 1
  }

  /// Position of the middle column separator (after "│ NAME ")
  #[must_use] 
  pub const fn middle_pos(&self) -> usize {
    1 + 1 + self.name_width + 1
  }
}

/// Default table configuration matching the TR-100 style
pub const TABLE_CONFIG: TableConfig = TableConfig::new(13, 32);

/// Writes the header with the decorative top border (with gradient colors)
pub fn write_header(buf: &mut String, title: &str, subtitle: &str) {
  let total = TABLE_CONFIG.total_width();
  let c = &*COLORS;
  let inner_width = total - 2; // excluding ┌ and ┐

  // Top border: ┌┬┬┬...┬┐ with gradient
  buf.push_str(c.dim);
  buf.push('┌');

  if c.reset.is_empty() {
    for _ in 0..inner_width {
      buf.push('┬');
    }
  } else {
    write_gradient_chars(buf, '┬', inner_width);
    buf.push_str(c.dim);
  }

  buf.push_str("┐\n");

  // Border below top: ├┴┴┴...┴┤ (also gradient)
  buf.push('├');

  if c.reset.is_empty() {
    for _ in 0..inner_width {
      buf.push('┴');
    }
  } else {
    write_gradient_chars(buf, '┴', inner_width);
    buf.push_str(c.dim);
  }

  buf.push('┤');
  buf.push_str(c.reset);
  buf.push('\n');

  // Centered title
  write_centered_colored(buf, title, c.bold_cyan);

  // Centered subtitle
  write_centered_colored(buf, subtitle, c.cyan);
}

/// Write characters with terminal colors arranged in gradient order
fn write_gradient_chars(buf: &mut String, ch: char, width: usize) {
  let c = &*COLORS;

  // Terminal colors arranged in a gradient-like order:
  // red -> yellow -> green -> cyan -> blue -> magenta
  // Using both normal (30-37) and bright (90-97) variants for smoother
  // transition
  const GRADIENT_COLORS: &[u8] = &[
    1,  // red
    9,  // bright red
    3,  // yellow
    11, // bright yellow
    2,  // green
    10, // bright green
    6,  // cyan
    14, // bright cyan
    4,  // blue
    12, // bright blue
    5,  // magenta
    13, // bright magenta
  ];

  let num_colors = GRADIENT_COLORS.len();
  let chars_per_color = width / num_colors;
  let remainder = width % num_colors;

  for (idx, &color) in GRADIENT_COLORS.iter().enumerate() {
    let extra = usize::from(idx < remainder);
    let segment_width = chars_per_color + extra;

    // Use foreground color codes (30-37 for normal, 90-97 for bright)
    if color < 8 {
      buf.push_str(&format!("\x1b[3{color}m"));
    } else {
      buf.push_str(&format!("\x1b[9{}m", color - 8));
    }

    for _ in 0..segment_width {
      buf.push(ch);
    }
  }
  buf.push_str(c.reset);
}

/// Writes centered text within the table with color
fn write_centered_colored(buf: &mut String, text: &str, color: &str) {
  let c = &*COLORS;
  let content_width = TABLE_CONFIG.total_width() - 2; // minus the two │ borders
  let text_len = text.chars().count();
  let padding_left = (content_width.saturating_sub(text_len)) / 2;
  let padding_right = content_width
    .saturating_sub(text_len)
    .saturating_sub(padding_left);

  buf.push_str(c.dim);
  buf.push('│');
  buf.push_str(c.reset);
  for _ in 0..padding_left {
    buf.push(' ');
  }
  buf.push_str(color);
  buf.push_str(text);
  buf.push_str(c.reset);
  for _ in 0..padding_right {
    buf.push(' ');
  }
  buf.push_str(c.dim);
  buf.push('│');
  buf.push_str(c.reset);
  buf.push('\n');
}

/// Writes a divider line
pub fn write_divider(buf: &mut String, style: DividerStyle) {
  let c = &*COLORS;
  let total = TABLE_CONFIG.total_width();

  // FullWidth styles don't have a middle column
  if matches!(
    style,
    DividerStyle::FullWidth | DividerStyle::FullWidthBottom
  ) {
    let (left, right) = match style {
      DividerStyle::FullWidth => ('├', '┤'),
      DividerStyle::FullWidthBottom => ('└', '┘'),
      _ => unreachable!(),
    };
    buf.push_str(c.dim);
    buf.push(left);
    for _ in 1..total - 1 {
      buf.push('─');
    }
    buf.push(right);
    buf.push_str(c.reset);
    buf.push('\n');
    return;
  }

  let (left, mid, right) = match style {
    DividerStyle::Top => ('├', '┬', '┤'),
    DividerStyle::Middle => ('├', '┼', '┤'),
    DividerStyle::Bottom => ('└', '┴', '┘'),
    DividerStyle::FullWidth | DividerStyle::FullWidthBottom => unreachable!(),
  };

  let middle_pos = TABLE_CONFIG.middle_pos();

  buf.push_str(c.dim);
  buf.push(left);
  for i in 1..total - 1 {
    if i == middle_pos {
      buf.push(mid);
    } else {
      buf.push('─');
    }
  }
  buf.push(right);
  buf.push_str(c.reset);
  buf.push('\n');
}

#[allow(dead_code)]
pub enum DividerStyle {
  Top,
  Middle,
  Bottom,
  FullWidth,       // No middle column, just ├───...───┤
  FullWidthBottom, // No middle column, just └───...───┘
}

/// Writes a data row: │ NAME          │ DATA                             │
pub fn write_row(buf: &mut String, name: &str, data: &str) {
  let c = &*COLORS;
  let name_width = TABLE_CONFIG.name_width;
  let data_width = TABLE_CONFIG.data_width;

  // Truncate name if needed
  let name_display: String = if name.chars().count() > name_width {
    let mut s: String = name.chars().take(name_width - 3).collect();
    s.push_str("...");
    s
  } else {
    name.to_string()
  };

  // Truncate data if needed (but preserve ANSI codes)
  let data_display: String = truncate_with_ansi(data, data_width);

  // Count visible characters (excluding ANSI codes)
  let name_visible_len = name_display.chars().count();
  let data_visible_len = visible_len(&data_display);

  // Border
  buf.push_str(c.dim);
  buf.push_str("│ ");
  buf.push_str(c.reset);

  // Label (colored)
  buf.push_str(c.bold_blue);
  buf.push_str(&name_display);
  buf.push_str(c.reset);
  for _ in name_visible_len..name_width {
    buf.push(' ');
  }

  // Middle border
  buf.push_str(c.dim);
  buf.push_str(" │ ");
  buf.push_str(c.reset);

  // Data
  buf.push_str(&data_display);
  for _ in data_visible_len..data_width {
    buf.push(' ');
  }

  // Right border
  buf.push_str(c.dim);
  buf.push_str(" │");
  buf.push_str(c.reset);
  buf.push('\n');
}

/// Calculate visible length excluding ANSI escape codes
fn visible_len(s: &str) -> usize {
  let mut len = 0;
  let mut in_escape = false;

  for c in s.chars() {
    if c == '\x1b' {
      in_escape = true;
    } else if in_escape {
      if c == 'm' {
        in_escape = false;
      }
    } else {
      len += 1;
    }
  }

  len
}

/// Truncate string while preserving ANSI codes
fn truncate_with_ansi(s: &str, max_visible: usize) -> String {
  let total_visible = visible_len(s);

  // If it fits, return as-is
  if total_visible <= max_visible {
    return s.to_string();
  }

  let mut result = String::with_capacity(s.len());
  let mut visible_count = 0;
  let mut in_escape = false;
  let target = max_visible.saturating_sub(3);

  for c in s.chars() {
    if c == '\x1b' {
      in_escape = true;
      result.push(c);
    } else if in_escape {
      result.push(c);
      if c == 'm' {
        in_escape = false;
      }
    } else if visible_count < target {
      result.push(c);
      visible_count += 1;
    }
  }

  result.push_str("...");
  result
}

/// Generate a bar graph with colors based on usage percentage
/// Low usage: green, medium: yellow, high: red
#[must_use] 
pub fn bar_graph(used: f64, total: f64, width: usize) -> String {
  let c = &*COLORS;
  let percent = if total > 0.0 { used / total } else { 0.0 };
  let filled = ((percent * width as f64).round() as usize).min(width);

  // Choose color based on percentage
  let bar_color = if percent < 0.5 {
    c.green
  } else if percent < 0.75 {
    c.yellow
  } else {
    c.red
  };

  let mut result = String::with_capacity(width * 3 + 20);
  result.push_str(bar_color);
  for _ in 0..filled {
    result.push('█');
  }
  result.push_str(c.reset);
  result.push_str(c.dim);
  for _ in filled..width {
    result.push('░');
  }
  result.push_str(c.reset);
  result
}

/// Compact uptime format like: 1d 2h 30m
#[must_use] 
pub fn format_uptime_compact(seconds: u64) -> String {
  let days = seconds / 86400;
  let hours = (seconds / 3600) % 24;
  let minutes = (seconds / 60) % 60;

  let mut result = String::with_capacity(16);

  if days > 0 {
    let _ = write!(result, "{days}d ");
  }
  if hours > 0 || days > 0 {
    let _ = write!(result, "{hours}h ");
  }
  let _ = write!(result, "{minutes}m");

  if result.is_empty() || result == "0m" {
    result = "<1m".to_string();
  }

  result
}
