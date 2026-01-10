use std::{ffi::CStr, mem::MaybeUninit};

use crate::{UtsName, syscall::read_file_fast};

/// Gets the current username
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_username() -> String {
  unsafe {
    let ptr = libc::getenv(c"USER".as_ptr());
    if ptr.is_null() {
      "unknown".into()
    } else {
      CStr::from_ptr(ptr)
        .to_str()
        .unwrap_or("unknown")
        .to_string()
    }
  }
}

/// Gets the hostname
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_hostname(utsname: &UtsName) -> String {
  utsname.nodename().to_str().unwrap_or("unknown").to_string()
}

/// Gets the local IP address by finding the default route interface
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_local_ip() -> String {
  let mut buffer = [0u8; 4096];

  // First, find the default interface from /proc/net/route
  // The default route has destination 00000000
  let default_iface =
    if let Ok(bytes_read) = read_file_fast("/proc/net/route", &mut buffer) {
      let content = std::str::from_utf8(&buffer[..bytes_read]).unwrap_or("");
      let mut iface = None;
      for line in content.lines().skip(1) {
        // Skip header
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == "00000000" {
          iface = Some(parts[0].to_string());
          break;
        }
      }
      iface
    } else {
      None
    };

  // Now read the interface's IP from /sys/class/net/{iface}/...
  // Or use getifaddrs via libc
  if let Some(iface) = default_iface {
    if let Some(ip) = get_interface_ip(&iface) {
      return ip;
    }
  }

  // Fallback: try to find any non-loopback IP from fib_trie Local section
  if let Ok(bytes_read) = read_file_fast("/proc/net/fib_trie", &mut buffer) {
    let content = std::str::from_utf8(&buffer[..bytes_read]).unwrap_or("");

    // Look for IPs in the "Local:" section with "/32 host LOCAL"
    let mut in_local_section = false;
    let mut last_ip: Option<&str> = None;

    for line in content.lines() {
      let trimmed = line.trim();

      if trimmed == "Local:" {
        in_local_section = true;
        continue;
      }

      if in_local_section {
        // Check if this is an IP line
        if trimmed.starts_with("|-- ") {
          if let Some(ip) = trimmed.strip_prefix("|-- ") {
            let ip = ip.trim();
            // Basic validation and skip loopback/link-local
            if ip.chars().filter(|&c| c == '.').count() == 3
              && ip.chars().all(|c| c.is_ascii_digit() || c == '.')
              && !ip.starts_with("127.")
              && !ip.starts_with("169.254.")
            {
              last_ip = Some(ip);
            }
          }
        }
        // Check if the previous IP is a host LOCAL (not broadcast)
        if trimmed == "/32 host LOCAL" {
          if let Some(ip) = last_ip {
            // Prefer private IPs (192.168.x.x, 10.x.x.x, 172.16-31.x.x)
            if ip.starts_with("192.168.")
              || ip.starts_with("10.")
              || (ip.starts_with("172.")
                && ip
                  .split('.')
                  .nth(1)
                  .and_then(|s| s.parse::<u8>().ok())
                  .is_some_and(|n| (16..=31).contains(&n)))
            {
              return ip.to_string();
            }
          }
        }
      }
    }
  }

  "No IP found".to_string()
}

/// Get IP address for a specific interface using ioctl
fn get_interface_ip(iface: &str) -> Option<String> {
  use std::mem::zeroed;

  // Use SIOCGIFADDR ioctl to get the IP
  const SIOCGIFADDR: libc::c_ulong = 0x8915;

  #[repr(C)]
  struct Ifreq {
    ifr_name: [libc::c_char; 16],
    ifr_addr: libc::sockaddr,
  }

  unsafe {
    let sock = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
    if sock < 0 {
      return None;
    }

    let mut req: Ifreq = zeroed();

    // Copy interface name
    let name_bytes = iface.as_bytes();
    let copy_len = name_bytes.len().min(15);
    for (i, &byte) in name_bytes.iter().take(copy_len).enumerate() {
      req.ifr_name[i] = byte as libc::c_char;
    }

    let ret = libc::ioctl(sock, SIOCGIFADDR, &mut req);
    libc::close(sock);

    if ret < 0 {
      return None;
    }

    // Extract IP from sockaddr_in
    let addr: *const libc::sockaddr_in = &req.ifr_addr as *const _ as *const _;
    let ip = (*addr).sin_addr.s_addr;

    // Convert to dotted decimal (already in network byte order)
    let a = (ip & 0xFF) as u8;
    let b = ((ip >> 8) & 0xFF) as u8;
    let c = ((ip >> 16) & 0xFF) as u8;
    let d = ((ip >> 24) & 0xFF) as u8;

    Some(format!("{}.{}.{}.{}", a, b, c, d))
  }
}

/// Gets CPU model name
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_cpu_model() -> String {
  let mut buffer = [0u8; 4096];

  if let Ok(bytes_read) = read_file_fast("/proc/cpuinfo", &mut buffer) {
    let content = &buffer[..bytes_read];
    let content_str = std::str::from_utf8(content).unwrap_or("");

    for line in content_str.lines() {
      if line.starts_with("model name") {
        if let Some(pos) = line.find(':') {
          let model = line[pos + 1..].trim();
          // Shorten the model name to first 4 words
          let words: Vec<&str> = model.split_whitespace().take(4).collect();
          return words.join(" ");
        }
      }
    }
  }

  "Unknown".to_string()
}

/// Gets number of CPU cores
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_cpu_cores() -> usize {
  // Use sysconf for CPU count
  unsafe {
    let count = libc::sysconf(libc::_SC_NPROCESSORS_ONLN);
    if count > 0 {
      #[allow(clippy::cast_sign_loss)]
      return count as usize;
    }
  }

  1
}

/// Gets load averages
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_load_averages() -> (f64, f64, f64) {
  let mut buffer = [0u8; 128];

  if let Ok(bytes_read) = read_file_fast("/proc/loadavg", &mut buffer) {
    let content = std::str::from_utf8(&buffer[..bytes_read]).unwrap_or("");
    let parts: Vec<&str> = content.split_whitespace().collect();

    if parts.len() >= 3 {
      let load1 = parts[0].parse().unwrap_or(0.0);
      let load5 = parts[1].parse().unwrap_or(0.0);
      let load15 = parts[2].parse().unwrap_or(0.0);
      return (load1, load5, load15);
    }
  }

  (0.0, 0.0, 0.0)
}

/// Gets uptime in seconds
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_uptime_seconds() -> u64 {
  let mut info = MaybeUninit::uninit();
  if unsafe { libc::sysinfo(info.as_mut_ptr()) } == 0 {
    #[allow(clippy::cast_sign_loss)]
    unsafe {
      info.assume_init().uptime as u64
    }
  } else {
    0
  }
}

/// Gets memory info as (used_kb, total_kb)
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_memory_info() -> (u64, u64) {
  let mut total_kb = 0u64;
  let mut available_kb = 0u64;
  let mut buffer = [0u8; 1024];

  if let Ok(bytes_read) = read_file_fast("/proc/meminfo", &mut buffer) {
    let content = &buffer[..bytes_read];

    let mut offset = 0;
    while offset < content.len() {
      let remaining = &content[offset..];
      let line_end = remaining
        .iter()
        .position(|&b| b == b'\n')
        .unwrap_or(remaining.len());
      let line = &remaining[..line_end];

      if line.starts_with(b"MemTotal:") {
        let mut pos = 9;
        while pos < line.len() && line[pos].is_ascii_whitespace() {
          pos += 1;
        }
        total_kb = parse_u64_fast(&line[pos..]);
      } else if line.starts_with(b"MemAvailable:") {
        let mut pos = 13;
        while pos < line.len() && line[pos].is_ascii_whitespace() {
          pos += 1;
        }
        available_kb = parse_u64_fast(&line[pos..]);
      }

      offset += line_end + 1;
    }
  }

  let used_kb = total_kb.saturating_sub(available_kb);
  (used_kb, total_kb)
}

/// Gets disk info as (used_bytes, total_bytes) for root filesystem
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_disk_info() -> (u64, u64) {
  let mut vfs = MaybeUninit::uninit();
  let path = b"/\0";

  if unsafe { libc::statvfs(path.as_ptr().cast(), vfs.as_mut_ptr()) } != 0 {
    return (0, 0);
  }

  let vfs = unsafe { vfs.assume_init() };
  let total = vfs.f_bsize * vfs.f_blocks;
  let available = vfs.f_bsize * vfs.f_bavail;
  let used = total.saturating_sub(available);

  (used, total)
}

#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_shell() -> String {
  unsafe {
    let ptr = libc::getenv(c"SHELL".as_ptr());
    if ptr.is_null() {
      return "unknown".into();
    }

    let bytes = CStr::from_ptr(ptr).to_bytes();
    let start = bytes.iter().rposition(|&b| b == b'/').map_or(0, |i| i + 1);
    let name = std::str::from_utf8_unchecked(&bytes[start..]);
    name.into()
  }
}

/// Fast integer parsing without stdlib overhead
#[inline]
fn parse_u64_fast(s: &[u8]) -> u64 {
  let mut result = 0u64;
  for &byte in s {
    if byte.is_ascii_digit() {
      result = result * 10 + u64::from(byte - b'0');
    } else {
      break;
    }
  }
  result
}

/// Gets the host/motherboard model
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_host() -> String {
  let mut buffer = [0u8; 256];

  // Try to read board name
  let board_name = if let Ok(bytes_read) =
    read_file_fast("/sys/devices/virtual/dmi/id/board_name", &mut buffer)
  {
    std::str::from_utf8(&buffer[..bytes_read])
      .unwrap_or("")
      .trim()
      .to_string()
  } else {
    String::new()
  };

  if board_name.is_empty() {
    // Try product name as fallback
    if let Ok(bytes_read) =
      read_file_fast("/sys/devices/virtual/dmi/id/product_name", &mut buffer)
    {
      let product = std::str::from_utf8(&buffer[..bytes_read])
        .unwrap_or("")
        .trim();
      if !product.is_empty() {
        return product.to_string();
      }
    }
    return "Unknown".to_string();
  }

  board_name
}

/// Gets the GPU name
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_gpu() -> String {
  let mut buffer = [0u8; 4096];

  // Try to find the primary GPU (card0 or card1)
  for card in ["card0", "card1"] {
    let vendor_path = format!("/sys/class/drm/{}/device/vendor", card);
    let device_path = format!("/sys/class/drm/{}/device/device", card);

    let vendor =
      if let Ok(bytes_read) = read_file_fast(&vendor_path, &mut buffer) {
        std::str::from_utf8(&buffer[..bytes_read])
          .unwrap_or("")
          .trim()
          .to_string()
      } else {
        continue;
      };

    let device =
      if let Ok(bytes_read) = read_file_fast(&device_path, &mut buffer) {
        std::str::from_utf8(&buffer[..bytes_read])
          .unwrap_or("")
          .trim()
          .to_lowercase()
      } else {
        continue;
      };

    // Get driver name from uevent
    let uevent_path = format!("/sys/class/drm/{}/device/uevent", card);
    let driver =
      if let Ok(bytes_read) = read_file_fast(&uevent_path, &mut buffer) {
        let content = std::str::from_utf8(&buffer[..bytes_read]).unwrap_or("");
        content
          .lines()
          .find(|l| l.starts_with("DRIVER="))
          .and_then(|l| l.strip_prefix("DRIVER="))
          .unwrap_or("")
          .to_string()
      } else {
        String::new()
      };

    // Map vendor IDs to names and try to get GPU model
    let device_id = device.trim_start_matches("0x");
    let gpu_name = match vendor.as_str() {
      "0x1002" => {
        lookup_amd_gpu(device_id)
          .unwrap_or_else(|| format!("AMD Radeon ({})", driver))
      },
      "0x10de" => {
        lookup_nvidia_gpu(device_id)
          .unwrap_or_else(|| format!("NVIDIA ({})", driver))
      },
      "0x8086" => format!("Intel ({})", driver),
      _ => continue,
    };

    return gpu_name;
  }

  "Unknown".to_string()
}

/// Lookup table for common AMD GPU device IDs
fn lookup_amd_gpu(device_id: &str) -> Option<String> {
  // Common AMD Radeon device IDs (lowercase)
  match device_id {
    // RX 5000 series (Navi 10/14)
    "731f" => Some("AMD Radeon RX 5700 XT".to_string()),
    "7340" => Some("AMD Radeon RX 5500 XT".to_string()),
    // RX 6000 series (Navi 21/22/23)
    "73bf" => Some("AMD Radeon RX 6800 XT".to_string()),
    "73df" => Some("AMD Radeon RX 6700 XT".to_string()),
    "73ff" => Some("AMD Radeon RX 6600 XT".to_string()),
    // RX 7000 series (Navi 31/32/33)
    "744c" => Some("AMD Radeon RX 7900 XTX".to_string()),
    "7480" => Some("AMD Radeon RX 7700 XT".to_string()),
    // Integrated
    "1636" => Some("AMD Radeon Vega 7".to_string()),
    "15e7" => Some("AMD Radeon Vega 8".to_string()),
    "164c" => Some("AMD Radeon 680M".to_string()),
    _ => None,
  }
}

/// Lookup table for common NVIDIA GPU device IDs
fn lookup_nvidia_gpu(device_id: &str) -> Option<String> {
  // Common NVIDIA GeForce device IDs (lowercase)
  match device_id {
    // RTX 30 series
    "2204" => Some("NVIDIA RTX 3090".to_string()),
    "2206" => Some("NVIDIA RTX 3080".to_string()),
    "2484" => Some("NVIDIA RTX 3070".to_string()),
    "2488" => Some("NVIDIA RTX 3060 Ti".to_string()),
    "2503" => Some("NVIDIA RTX 3060".to_string()),
    // RTX 40 series
    "2684" => Some("NVIDIA RTX 4090".to_string()),
    "2782" => Some("NVIDIA RTX 4080".to_string()),
    "2786" => Some("NVIDIA RTX 4070 Ti".to_string()),
    "2860" => Some("NVIDIA RTX 4070".to_string()),
    // GTX 16 series
    "1f82" => Some("NVIDIA GTX 1650".to_string()),
    "2182" => Some("NVIDIA GTX 1660 Super".to_string()),
    _ => None,
  }
}

/// Gets the desktop environment / window manager
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_wm() -> String {
  // Check XDG_CURRENT_DESKTOP first
  let desktop = unsafe {
    let ptr = libc::getenv(c"XDG_CURRENT_DESKTOP".as_ptr());
    if !ptr.is_null() {
      CStr::from_ptr(ptr).to_str().unwrap_or("").to_string()
    } else {
      String::new()
    }
  };

  // Get session type (wayland/x11)
  let session_type = unsafe {
    let ptr = libc::getenv(c"XDG_SESSION_TYPE".as_ptr());
    if !ptr.is_null() {
      CStr::from_ptr(ptr).to_str().unwrap_or("").to_string()
    } else {
      String::new()
    }
  };

  let desktop = desktop
    .strip_prefix("none+")
    .unwrap_or(&desktop)
    .to_string();

  if desktop.is_empty() && session_type.is_empty() {
    return "Unknown".to_string();
  }

  if session_type.is_empty() {
    desktop
  } else {
    let session_cap = capitalize_first(&session_type);
    format!("{} ({})", desktop, session_cap)
  }
}

/// Gets the terminal emulator name
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_terminal() -> String {
  // Check TERM_PROGRAM first (set by many modern terminals)
  let term_program = unsafe {
    let ptr = libc::getenv(c"TERM_PROGRAM".as_ptr());
    if !ptr.is_null() {
      let s = CStr::from_ptr(ptr).to_str().unwrap_or("");
      if !s.is_empty() {
        return s.to_string();
      }
    }
    String::new()
  };

  // Check TERMINAL env var
  let terminal = unsafe {
    let ptr = libc::getenv(c"TERMINAL".as_ptr());
    if !ptr.is_null() {
      let s = CStr::from_ptr(ptr).to_str().unwrap_or("");
      if !s.is_empty() {
        return s.to_string();
      }
    }
    String::new()
  };

  // Fallback to TERM
  let term = unsafe {
    let ptr = libc::getenv(c"TERM".as_ptr());
    if !ptr.is_null() {
      CStr::from_ptr(ptr)
        .to_str()
        .unwrap_or("unknown")
        .to_string()
    } else {
      "unknown".to_string()
    }
  };

  if !term_program.is_empty() {
    term_program
  } else if !terminal.is_empty() {
    terminal
  } else {
    term
  }
}

/// Gets swap info as (used_kb, total_kb)
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_swap_info() -> (u64, u64) {
  let mut total_kb = 0u64;
  let mut free_kb = 0u64;
  let mut buffer = [0u8; 2048];

  if let Ok(bytes_read) = read_file_fast("/proc/meminfo", &mut buffer) {
    let content = &buffer[..bytes_read];

    let mut offset = 0;
    while offset < content.len() {
      let remaining = &content[offset..];
      let line_end = remaining
        .iter()
        .position(|&b| b == b'\n')
        .unwrap_or(remaining.len());
      let line = &remaining[..line_end];

      if line.starts_with(b"SwapTotal:") {
        let mut pos = 10;
        while pos < line.len() && line[pos].is_ascii_whitespace() {
          pos += 1;
        }
        total_kb = parse_u64_fast(&line[pos..]);
      } else if line.starts_with(b"SwapFree:") {
        let mut pos = 9;
        while pos < line.len() && line[pos].is_ascii_whitespace() {
          pos += 1;
        }
        free_kb = parse_u64_fast(&line[pos..]);
      }

      offset += line_end + 1;
    }
  }

  let used_kb = total_kb.saturating_sub(free_kb);
  (used_kb, total_kb)
}

/// Capitalize first letter of a string
fn capitalize_first(s: &str) -> String {
  let mut chars = s.chars();
  match chars.next() {
    None => String::new(),
    Some(first) => first.to_uppercase().chain(chars).collect(),
  }
}
