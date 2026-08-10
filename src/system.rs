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
  if let Some(iface) = default_iface
    && let Some(ip) = get_interface_ip(&iface)
  {
    return ip;
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
        if trimmed.starts_with("|-- ")
          && let Some(ip) = trimmed.strip_prefix("|-- ")
        {
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
        // Check if the previous IP is a host LOCAL (not broadcast)
        if trimmed == "/32 host LOCAL"
          && let Some(ip) = last_ip
        {
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
    let addr: *const libc::sockaddr_in = (&raw const req.ifr_addr).cast();
    let ip = (*addr).sin_addr.s_addr;

    // Convert to dotted decimal (already in network byte order)
    let a = (ip & 0xFF) as u8;
    let b = ((ip >> 8) & 0xFF) as u8;
    let c = ((ip >> 16) & 0xFF) as u8;
    let d = ((ip >> 24) & 0xFF) as u8;

    Some(format!("{a}.{b}.{c}.{d}"))
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
      if line.starts_with("model name")
        && let Some(pos) = line.find(':')
      {
        let model = line[pos + 1..].trim();
        // Shorten the model name to first 4 words
        let words: Vec<&str> = model.split_whitespace().take(4).collect();
        return words.join(" ");
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

/// Gets memory info as (`used_kb`, `total_kb`)
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

/// Gets disk info as (`used_bytes`, `total_bytes`) for root filesystem
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

/// Paths where pci.ids database might be found
const PCI_IDS_PATHS: &[&str] = &[
  "/usr/share/hwdata/pci.ids",
  "/usr/share/misc/pci.ids",
  "/usr/local/share/hwdata/pci.ids",
  "/usr/share/pci.ids",
];

/// Paths where AMD GPU IDs database might be found
const AMD_GPU_IDS_PATHS: &[&str] = &[
  "/usr/share/libdrm/amdgpu.ids",
  "/usr/local/share/libdrm/amdgpu.ids",
];

/// Gets the GPU name with improved detection
#[must_use]
#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub fn get_gpu() -> String {
  let mut buffer = [0u8; 4096];

  // Try DRM detection first (scan all cards)
  if let Some(gpu) = detect_gpu_via_drm(&mut buffer) {
    return gpu;
  }

  // Fallback: scan PCI bus directly
  if let Some(gpu) = detect_gpu_via_pci(&mut buffer) {
    return gpu;
  }

  // Last resort: check NVIDIA proc interface
  if let Some(gpu) = detect_nvidia_proc(&mut buffer) {
    return gpu;
  }

  "Unknown".to_string()
}

/// Detect GPU via DRM subsystem (primary method)
fn detect_gpu_via_drm(buffer: &mut [u8; 4096]) -> Option<String> {
  // Scan /sys/class/drm/ for all card devices
  let drm_path = "/sys/class/drm";

  // Try cards 0-7 to cover multi-GPU systems
  for i in 0..8 {
    let card = format!("card{i}");
    let device_dir = format!("{drm_path}/{card}/device");

    // Check if this card exists by trying to read vendor
    let vendor_path = format!("{device_dir}/vendor");
    let vendor = match read_file_fast(&vendor_path, buffer) {
      Ok(bytes_read) => {
        std::str::from_utf8(&buffer[..bytes_read])
          .unwrap_or("")
          .trim()
          .to_string()
      },
      Err(_) => continue,
    };

    // Parse vendor ID
    let vendor_id = parse_hex_id(&vendor)?;

    // Skip non-GPU vendors
    let vendor_name = get_vendor_name(vendor_id)?;

    // Read device ID
    let device_path = format!("{device_dir}/device");
    let device = match read_file_fast(&device_path, buffer) {
      Ok(bytes_read) => {
        std::str::from_utf8(&buffer[..bytes_read])
          .unwrap_or("")
          .trim()
          .to_string()
      },
      Err(_) => continue,
    };
    let device_id = parse_hex_id(&device)?;

    // Get driver name (more reliable via symlink target name from uevent)
    let driver = get_driver_name(&device_dir, buffer);

    // Try to get GPU name from various sources
    let gpu_name =
      get_gpu_name(vendor_id, device_id, vendor_name, &driver, buffer);

    return Some(gpu_name);
  }

  None
}

/// Detect GPU via PCI bus (fallback method)
fn detect_gpu_via_pci(buffer: &mut [u8; 4096]) -> Option<String> {
  // Scan /sys/bus/pci/devices/ for display class devices (0x03)
  for i in 0..32 {
    // Typical GPU PCI slots
    let pci_addr = format!("0000:0{i:x}:00.0");
    let device_dir = format!("/sys/bus/pci/devices/{pci_addr}");

    // Check class to see if it's a display device
    let class_path = format!("{device_dir}/class");
    if let Ok(bytes_read) = read_file_fast(&class_path, buffer) {
      let class_str = std::str::from_utf8(&buffer[..bytes_read])
        .unwrap_or("")
        .trim();
      // Display class is 0x03xxxx
      if !class_str.starts_with("0x03") {
        continue;
      }

      // Read vendor and device
      let vendor_path = format!("{device_dir}/vendor");
      let vendor = match read_file_fast(&vendor_path, buffer) {
        Ok(bytes_read) => {
          std::str::from_utf8(&buffer[..bytes_read])
            .unwrap_or("")
            .trim()
            .to_string()
        },
        Err(_) => continue,
      };

      let vendor_id = parse_hex_id(&vendor)?;
      let vendor_name = get_vendor_name(vendor_id)?;

      let device_path = format!("{device_dir}/device");
      let device = match read_file_fast(&device_path, buffer) {
        Ok(bytes_read) => {
          std::str::from_utf8(&buffer[..bytes_read])
            .unwrap_or("")
            .trim()
            .to_string()
        },
        Err(_) => continue,
      };
      let device_id = parse_hex_id(&device)?;

      let driver = get_driver_name(&device_dir, buffer);
      let gpu_name =
        get_gpu_name(vendor_id, device_id, vendor_name, &driver, buffer);

      return Some(gpu_name);
    }
  }

  None
}

/// Check NVIDIA's proc interface as last resort
fn detect_nvidia_proc(buffer: &mut [u8; 4096]) -> Option<String> {
  // Try to read NVIDIA GPU info from /proc
  let nvidia_info = "/proc/driver/nvidia/gpus/0000:01:00.0/information";
  if let Ok(bytes_read) = read_file_fast(nvidia_info, buffer) {
    let content = std::str::from_utf8(&buffer[..bytes_read]).unwrap_or("");
    for line in content.lines() {
      if line.starts_with("Model:")
        && let Some(model) = line.strip_prefix("Model:")
      {
        let model = model.trim();
        if !model.is_empty() {
          return Some(format!("NVIDIA {model}"));
        }
      }
    }
  }
  None
}

/// Parse hex ID from string like "0x1002" or "1002"
fn parse_hex_id(s: &str) -> Option<u32> {
  let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");
  u32::from_str_radix(s, 16).ok()
}

/// Map vendor ID to vendor name, returning None for non-GPU vendors
const fn get_vendor_name(vendor_id: u32) -> Option<&'static str> {
  match vendor_id {
    // AMD vendor IDs (includes legacy ATI)
    0x1002 | 0x1022 => Some("AMD"),
    // NVIDIA vendor IDs
    0x10DE | 0x12D2 => Some("NVIDIA"),
    // Intel vendor IDs
    0x8086 | 0x8087 => Some("Intel"),
    // Other GPU vendors
    0x1A03 => Some("ASPEED"),
    0x1AF4 => Some("VirtIO"),
    0x15AD => Some("VMware"),
    0x1AB8 => Some("Parallels"),
    0x1234 => Some("QEMU"),
    _ => None,
  }
}

/// Get driver name from device directory
fn get_driver_name(device_dir: &str, buffer: &mut [u8; 4096]) -> String {
  // Try reading from uevent first (most reliable)
  let uevent_path = format!("{device_dir}/uevent");
  if let Ok(bytes_read) = read_file_fast(&uevent_path, buffer) {
    let content = std::str::from_utf8(&buffer[..bytes_read]).unwrap_or("");
    for line in content.lines() {
      if let Some(driver) = line.strip_prefix("DRIVER=") {
        return driver.to_string();
      }
    }
  }

  String::new()
}

/// Get GPU name from various sources
fn get_gpu_name(
  vendor_id: u32,
  device_id: u32,
  vendor_name: &str,
  driver: &str,
  buffer: &mut [u8; 4096],
) -> String {
  // 1. Try AMD-specific amdgpu.ids file
  if vendor_id == 0x1002
    && let Some(name) = lookup_amd_gpu_ids(device_id, buffer)
  {
    return name;
  }

  // 2. Try pci.ids database
  if let Some(name) = lookup_pci_ids(vendor_id, device_id, buffer) {
    return format!("{vendor_name} {name}");
  }

  // 3. Try hardcoded lookup tables
  let device_hex = format!("{device_id:04x}");
  match vendor_id {
    0x1002 => {
      if let Some(name) = lookup_amd_gpu(&device_hex) {
        return name;
      }
    },
    0x10DE => {
      if let Some(name) = lookup_nvidia_gpu(&device_hex) {
        return name;
      }
    },
    0x8086 => {
      if let Some(name) = lookup_intel_gpu(&device_hex) {
        return name;
      }
    },
    _ => {},
  }

  // 4. Fallback to vendor + driver
  if driver.is_empty() {
    vendor_name.to_string()
  } else {
    format!("{vendor_name} ({driver})")
  }
}

/// Look up GPU name from AMD's amdgpu.ids file
fn lookup_amd_gpu_ids(
  device_id: u32,
  buffer: &mut [u8; 4096],
) -> Option<String> {
  let device_hex = format!("{device_id:04X}");

  for path in AMD_GPU_IDS_PATHS {
    // Read file in chunks since amdgpu.ids can be larger than 4096
    if let Ok(bytes_read) = read_file_fast(path, buffer) {
      let content = std::str::from_utf8(&buffer[..bytes_read]).unwrap_or("");
      // Format: "DEVICE_ID,    REVISION_ID,    "Name""
      // Example: "73BF,	C0,	AMD Radeon RX 6800 XT"
      for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
          continue;
        }
        // Check if line starts with our device ID (case insensitive)
        if line.to_uppercase().starts_with(&device_hex) {
          // Parse the name after the last comma
          let parts: Vec<&str> = line.splitn(3, ',').collect();
          if parts.len() >= 3 {
            let name = parts[2].trim().trim_matches('"');
            if !name.is_empty() {
              return Some(name.to_string());
            }
          }
        }
      }
    }
  }
  None
}

/// Look up GPU name from system pci.ids database
fn lookup_pci_ids(
  vendor_id: u32,
  device_id: u32,
  buffer: &mut [u8; 4096],
) -> Option<String> {
  let vendor_hex = format!("{vendor_id:04x}");
  let device_hex = format!("{device_id:04x}");

  for path in PCI_IDS_PATHS {
    if let Ok(bytes_read) = read_file_fast(path, buffer) {
      let content = std::str::from_utf8(&buffer[..bytes_read]).unwrap_or("");

      let mut in_vendor_section = false;

      for line in content.lines() {
        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
          continue;
        }

        // Vendor line: starts with no whitespace, format: "vendor_id  Vendor
        // Name"
        if !line.starts_with('\t') && !line.starts_with(' ') {
          if line.starts_with(&vendor_hex) {
            in_vendor_section = true;
          } else if in_vendor_section {
            // We've moved past our vendor section
            break;
          }
          continue;
        }

        // Device line: starts with single tab, format: "\tdevice_id  Device
        // Name"
        if in_vendor_section
          && line.starts_with('\t')
          && !line.starts_with("\t\t")
        {
          let line = line.trim_start();
          if line.starts_with(&device_hex) {
            // Extract device name
            let name = line[device_hex.len()..].trim();
            if !name.is_empty() {
              // Extract short name from brackets if present
              // Format: "Full Name [Short Name]"
              if let Some(start) = name.find('[')
                && let Some(end) = name.find(']')
                && end > start
              {
                return Some(name[start + 1..end].to_string());
              }
              return Some(name.to_string());
            }
          }
        }
      }
    }
  }
  None
}

/// Lookup table for common AMD GPU device IDs
fn lookup_amd_gpu(device_id: &str) -> Option<String> {
  // Common AMD Radeon device IDs (lowercase)
  match device_id {
    // RX 5000 series (Navi 10/14)
    "731f" => Some("AMD Radeon RX 5700 XT".to_string()),
    "7310" => Some("AMD Radeon RX 5700".to_string()),
    "7340" => Some("AMD Radeon RX 5500 XT".to_string()),
    "7341" => Some("AMD Radeon RX 5500".to_string()),
    // RX 6000 series (Navi 21/22/23/24)
    "73a5" => Some("AMD Radeon RX 6950 XT".to_string()),
    "73bf" => Some("AMD Radeon RX 6800 XT".to_string()),
    "73af" => Some("AMD Radeon RX 6900 XT".to_string()),
    "73a2" => Some("AMD Radeon RX 6900 XTX".to_string()),
    "73df" => Some("AMD Radeon RX 6700 XT".to_string()),
    "73ef" => Some("AMD Radeon RX 6700".to_string()),
    "73ff" => Some("AMD Radeon RX 6600 XT".to_string()),
    "73e3" => Some("AMD Radeon RX 6600".to_string()),
    "7422" => Some("AMD Radeon RX 6500 XT".to_string()),
    "743f" => Some("AMD Radeon RX 6400".to_string()),
    // RX 7000 series (Navi 31/32/33)
    "744c" => Some("AMD Radeon RX 7900 XTX".to_string()),
    "7448" => Some("AMD Radeon RX 7900 XT".to_string()),
    "745e" => Some("AMD Radeon RX 7900 GRE".to_string()),
    "7470" => Some("AMD Radeon RX 7800 XT".to_string()),
    "7480" => Some("AMD Radeon RX 7700 XT".to_string()),
    "7489" => Some("AMD Radeon RX 7600 XT".to_string()),
    "7483" => Some("AMD Radeon RX 7600".to_string()),
    // Integrated APUs
    "1636" => Some("AMD Radeon Vega 7".to_string()),
    "15e7" | "15dd" => Some("AMD Radeon Vega 8".to_string()),
    "1638" => Some("AMD Radeon Vega 6".to_string()),
    "164c" => Some("AMD Radeon 680M".to_string()),
    "164d" => Some("AMD Radeon 660M".to_string()),
    "1900" => Some("AMD Radeon R4".to_string()),
    "15d8" => Some("AMD Radeon Vega 3".to_string()),
    "163f" => Some("AMD Radeon 780M".to_string()),
    "1506" => Some("AMD Radeon R5".to_string()),
    // Older GPUs (Polaris/Vega)
    "67df" => Some("AMD Radeon RX 580".to_string()),
    "67ef" => Some("AMD Radeon RX 560".to_string()),
    "699f" => Some("AMD Radeon RX 550".to_string()),
    "687f" => Some("AMD Radeon RX Vega 64".to_string()),
    "6863" => Some("AMD Radeon RX Vega 56".to_string()),
    _ => None,
  }
}

/// Lookup table for common NVIDIA GPU device IDs
fn lookup_nvidia_gpu(device_id: &str) -> Option<String> {
  // Common NVIDIA GeForce device IDs (lowercase)
  match device_id {
    // RTX 50 series (Blackwell)
    "2b85" => Some("NVIDIA RTX 5090".to_string()),
    "2b87" => Some("NVIDIA RTX 5080".to_string()),
    "2bc7" => Some("NVIDIA RTX 5070 Ti".to_string()),
    "2bc6" => Some("NVIDIA RTX 5070".to_string()),
    // RTX 40 series (Ada Lovelace)
    "2684" => Some("NVIDIA RTX 4090".to_string()),
    "2702" => Some("NVIDIA RTX 4090 D".to_string()),
    "2704" => Some("NVIDIA RTX 4080 Super".to_string()),
    "2782" => Some("NVIDIA RTX 4080".to_string()),
    "2783" => Some("NVIDIA RTX 4070 Ti Super".to_string()),
    "2786" => Some("NVIDIA RTX 4070 Ti".to_string()),
    "2860" => Some("NVIDIA RTX 4070 Super".to_string()),
    "2882" => Some("NVIDIA RTX 4070".to_string()),
    "28a0" => Some("NVIDIA RTX 4060 Ti".to_string()),
    "28a1" => Some("NVIDIA RTX 4060".to_string()),
    // RTX 30 series (Ampere)
    "2204" => Some("NVIDIA RTX 3090 Ti".to_string()),
    "2203" => Some("NVIDIA RTX 3090".to_string()),
    "2208" => Some("NVIDIA RTX 3080 Ti".to_string()),
    "2206" => Some("NVIDIA RTX 3080".to_string()),
    "2482" => Some("NVIDIA RTX 3070 Ti".to_string()),
    "2484" => Some("NVIDIA RTX 3070".to_string()),
    "2486" | "2488" => Some("NVIDIA RTX 3060 Ti".to_string()),
    "2503" | "2504" => Some("NVIDIA RTX 3060".to_string()),
    "2520" => Some("NVIDIA RTX 3050".to_string()),
    // RTX 20 series (Turing)
    "1e04" => Some("NVIDIA RTX 2080 Ti".to_string()),
    "1e07" => Some("NVIDIA RTX 2080 Super".to_string()),
    "1e82" => Some("NVIDIA RTX 2080".to_string()),
    "1e84" => Some("NVIDIA RTX 2070 Super".to_string()),
    "1f02" => Some("NVIDIA RTX 2070".to_string()),
    "1f06" => Some("NVIDIA RTX 2060 Super".to_string()),
    "1f08" => Some("NVIDIA RTX 2060".to_string()),
    // GTX 16 series (Turing)
    "2182" => Some("NVIDIA GTX 1660 Super".to_string()),
    "2184" => Some("NVIDIA GTX 1660".to_string()),
    "1c82" => Some("NVIDIA GTX 1050 Ti".to_string()),
    "1f82" => Some("NVIDIA GTX 1650 Super".to_string()),
    "1f0a" | "2188" => Some("NVIDIA GTX 1650".to_string()),
    // GTX 10 series (Pascal)
    "1b80" => Some("NVIDIA GTX 1080".to_string()),
    "1b81" => Some("NVIDIA GTX 1070".to_string()),
    "1b82" => Some("NVIDIA GTX 1070 Ti".to_string()),
    "1c02" | "1c03" => Some("NVIDIA GTX 1060".to_string()),
    _ => None,
  }
}

/// Lookup table for common Intel GPU device IDs
fn lookup_intel_gpu(device_id: &str) -> Option<String> {
  match device_id {
    // Arc A-series (Alchemist)
    "56a0" => Some("Intel Arc A770".to_string()),
    "56a1" => Some("Intel Arc A750".to_string()),
    "56a5" => Some("Intel Arc A580".to_string()),
    "5690" => Some("Intel Arc A370M".to_string()),
    "5691" => Some("Intel Arc A350M".to_string()),
    "5692" => Some("Intel Arc A310".to_string()),
    // Xe Graphics (Tiger Lake, Alder Lake, Raptor Lake)
    "9a49" | "9a40" | "46a6" | "4626" | "a780" | "a7a0" => {
      Some("Intel Xe Graphics".to_string())
    },
    // UHD Graphics
    "3e92" | "3e91" | "3e98" | "9bc8" => Some("Intel UHD 630".to_string()),
    "5917" | "5916" | "3ea0" | "9b41" => Some("Intel UHD 620".to_string()),
    "4e61" => Some("Intel UHD Graphics".to_string()),
    // HD Graphics
    "1912" | "191b" => Some("Intel HD 530".to_string()),
    "5912" | "591b" => Some("Intel HD 630".to_string()),
    "0a16" => Some("Intel HD 4400".to_string()),
    "0412" => Some("Intel HD 4600".to_string()),
    _ => None,
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
    if ptr.is_null() {
      "unknown".to_string()
    } else {
      CStr::from_ptr(ptr)
        .to_str()
        .unwrap_or("unknown")
        .to_string()
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

/// Gets swap info as (`used_kb`, `total_kb`)
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
