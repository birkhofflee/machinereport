mod colors;
mod release;
mod syscall;
mod system;
mod table;

use std::io::{self, Write};

pub use microfetch_lib::UtsName;

use crate::{
  release::{get_os_pretty_name, get_system_info},
  system::{
    get_cpu_cores,
    get_cpu_model,
    get_disk_info,
    get_gpu,
    get_host,
    get_hostname,
    get_load_averages,
    get_local_ip,
    get_memory_info,
    get_shell,
    get_swap_info,
    get_terminal,
    get_uptime_seconds,
    get_username,
    get_wm,
  },
  table::{
    DividerStyle,
    bar_graph,
    format_uptime_compact,
    write_color_row,
    write_divider,
    write_header,
    write_row,
  },
};

const REPORT_TITLE: &str = "MICROFETCH";
const REPORT_SUBTITLE: &str = "SYSTEM REPORT";

#[cfg_attr(feature = "hotpath", hotpath::main)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
  if Some("--version") == std::env::args().nth(1).as_deref() {
    println!("Microfetch {}", env!("CARGO_PKG_VERSION"));
  } else {
    let utsname = UtsName::uname()?;
    print_system_report(&utsname)?;
  }

  Ok(())
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
#[allow(clippy::cast_precision_loss)]
fn print_system_report(
  utsname: &UtsName,
) -> Result<(), Box<dyn std::error::Error>> {
  // Gather all system information
  let os_name = get_os_pretty_name()?;
  let host = get_host();
  let kernel = get_system_info(utsname);
  let hostname = get_hostname(utsname);
  let machine_ip = get_local_ip();
  let username = get_username();
  let shell = get_shell();
  let terminal = get_terminal();
  let wm = get_wm();
  let cpu_model = get_cpu_model();
  let cpu_cores = get_cpu_cores();
  let gpu = get_gpu();
  let (load1, load5, load15) = get_load_averages();
  let uptime_secs = get_uptime_seconds();
  let uptime_str = format_uptime_compact(uptime_secs);
  let (mem_used_kb, mem_total_kb) = get_memory_info();
  let (swap_used_kb, swap_total_kb) = get_swap_info();
  let (disk_used, disk_total) = get_disk_info();

  // Calculate memory values
  let mem_used_gb = mem_used_kb as f64 / 1024.0 / 1024.0;
  let mem_total_gb = mem_total_kb as f64 / 1024.0 / 1024.0;
  let mem_percent = if mem_total_kb > 0 {
    (mem_used_kb as f64 / mem_total_kb as f64 * 100.0) as u32
  } else {
    0
  };

  // Calculate swap values
  let swap_used_gb = swap_used_kb as f64 / 1024.0 / 1024.0;
  let swap_total_gb = swap_total_kb as f64 / 1024.0 / 1024.0;
  let swap_percent = if swap_total_kb > 0 {
    (swap_used_kb as f64 / swap_total_kb as f64 * 100.0) as u32
  } else {
    0
  };

  // Calculate disk values
  let disk_used_gb = disk_used as f64 / 1024.0 / 1024.0 / 1024.0;
  let disk_total_gb = disk_total as f64 / 1024.0 / 1024.0 / 1024.0;
  let disk_percent = if disk_total > 0 {
    (disk_used as f64 / disk_total as f64 * 100.0) as u32
  } else {
    0
  };

  // Build output string
  let mut output = String::with_capacity(4096);

  // Header
  write_header(&mut output, REPORT_TITLE, REPORT_SUBTITLE);

  // OS section
  write_divider(&mut output, DividerStyle::Top);
  write_row(&mut output, "OS", &os_name);
  write_row(&mut output, "HOST", &host);
  write_row(&mut output, "KERNEL", &kernel);

  // User/Environment section
  write_divider(&mut output, DividerStyle::Middle);
  write_row(&mut output, "HOSTNAME", &hostname);
  write_row(&mut output, "USER", &username);
  write_row(&mut output, "SHELL", &shell);
  write_row(&mut output, "TERMINAL", &terminal);
  write_row(&mut output, "WM", &wm);

  // Hardware section
  write_divider(&mut output, DividerStyle::Middle);
  write_row(
    &mut output,
    "CPU",
    &format!("{} ({})", cpu_model, cpu_cores),
  );
  write_row(&mut output, "GPU", &gpu);

  // Load averages with bar graphs
  let load_1m_bar = bar_graph(load1, cpu_cores as f64, 32);
  let load_5m_bar = bar_graph(load5, cpu_cores as f64, 32);
  let load_15m_bar = bar_graph(load15, cpu_cores as f64, 32);
  write_row(&mut output, "LOAD 1m", &load_1m_bar);
  write_row(&mut output, "LOAD 5m", &load_5m_bar);
  write_row(&mut output, "LOAD 15m", &load_15m_bar);

  // Network section
  write_divider(&mut output, DividerStyle::Middle);
  write_row(&mut output, "LOCAL IP", &machine_ip);
  write_row(&mut output, "UPTIME", &uptime_str);

  // Memory section
  write_divider(&mut output, DividerStyle::Middle);
  let mem_info = format!(
    "{:.2}/{:.2} GiB [{}%]",
    mem_used_gb, mem_total_gb, mem_percent
  );
  let mem_bar = bar_graph(mem_used_kb as f64, mem_total_kb as f64, 32);
  write_row(&mut output, "MEMORY", &mem_info);
  write_row(&mut output, "MEM USAGE", &mem_bar);

  // Swap section (only if swap exists)
  if swap_total_kb > 0 {
    let swap_info = format!(
      "{:.2}/{:.2} GiB [{}%]",
      swap_used_gb, swap_total_gb, swap_percent
    );
    let swap_bar = bar_graph(swap_used_kb as f64, swap_total_kb as f64, 32);
    write_row(&mut output, "SWAP", &swap_info);
    write_row(&mut output, "SWAP USAGE", &swap_bar);
  }

  // Disk section
  write_divider(&mut output, DividerStyle::Middle);
  let disk_info = format!(
    "{:.2}/{:.2} GB [{}%]",
    disk_used_gb, disk_total_gb, disk_percent
  );
  let disk_bar = bar_graph(disk_used as f64, disk_total as f64, 32);
  write_row(&mut output, "DISK (/)", &disk_info);
  write_row(&mut output, "DISK USAGE", &disk_bar);

  // Color palette section
  write_divider(&mut output, DividerStyle::FullWidth);
  write_color_row(&mut output);

  // Footer (full width since color row has no columns)
  write_divider(&mut output, DividerStyle::FullWidthBottom);

  // Write to stdout
  let stdout = io::stdout();
  let mut handle = stdout.lock();
  handle.write_all(output.as_bytes())?;

  Ok(())
}
