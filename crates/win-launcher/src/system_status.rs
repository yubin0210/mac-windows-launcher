//! system_status
//! Hardware and operating system status monitor for the macOS Top Menu Bar.
//!
//! Windows: Queries Win32 power management (GetSystemPowerStatus) and system clock (GetLocalTime).
//! Non-Windows: High-fidelity mock status tracker for development and testing.

use mac_physics_core::menu_bar::SystemStatusSnapshot;
use std::time::{Duration, Instant};

/// Monitors hardware and operating system status indicators for the Menu Bar.
#[derive(Debug, Clone)]
pub struct SystemStatusTracker {
    snapshot: SystemStatusSnapshot,
    last_refresh: Option<Instant>,
    min_interval: Duration,
    mock_battery: Option<u8>,
    mock_charging: bool,
    mock_wifi: bool,
    mock_volume: u8,
    is_mock: bool,
}

impl Default for SystemStatusTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemStatusTracker {
    /// Creates a new system status tracker querying real system hardware.
    pub fn new() -> Self {
        let initial_snapshot = SystemStatusSnapshot::new(Some(85), false, true, 65, "12:00 PM");
        let mut tracker = Self {
            snapshot: initial_snapshot,
            last_refresh: None,
            min_interval: Duration::from_millis(1000), // Refresh at most once per second
            mock_battery: Some(85),
            mock_charging: false,
            mock_wifi: true,
            mock_volume: 65,
            is_mock: false,
        };
        tracker.refresh();
        tracker
    }

    /// Creates a mock system status tracker isolated from hardware queries for testing.
    pub fn new_mock() -> Self {
        let initial_snapshot = SystemStatusSnapshot::new(Some(85), false, true, 65, "12:00 PM");
        let mut tracker = Self {
            snapshot: initial_snapshot,
            last_refresh: None,
            min_interval: Duration::from_millis(1000),
            mock_battery: Some(85),
            mock_charging: false,
            mock_wifi: true,
            mock_volume: 65,
            is_mock: true,
        };
        tracker.refresh();
        tracker
    }

    /// Formats the current system clock time into macOS standard format (e.g. "Thu Nov 5 12:23 PM").
    pub fn format_clock() -> String {
        #[cfg(windows)]
        {
            unsafe {
                let st = windows::Win32::System::SystemInformation::GetLocalTime();
                format_system_time(&st)
            }
        }
        #[cfg(not(windows))]
        {
            format_mock_time()
        }
    }

    /// Sets mock battery percentage and charging state (useful for tests on all platforms).
    pub fn set_mock_battery(&mut self, percent: Option<u8>, is_charging: bool) {
        self.mock_battery = percent;
        self.mock_charging = is_charging;
        self.snapshot.battery_percent = percent;
        self.snapshot.is_charging = is_charging;
    }

    /// Sets mock Wi-Fi status.
    pub fn set_mock_wifi(&mut self, connected: bool) {
        self.mock_wifi = connected;
        self.snapshot.wifi_connected = connected;
    }

    /// Sets mock volume percentage.
    pub fn set_mock_volume(&mut self, volume: u8) {
        let clamped = volume.min(100);
        self.mock_volume = clamped;
        self.snapshot.volume_percent = clamped;
    }

    /// Returns the currently cached status snapshot without refreshing.
    pub fn snapshot(&self) -> &SystemStatusSnapshot {
        &self.snapshot
    }

    /// Refreshes and returns the current system status.
    pub fn refresh(&mut self) -> SystemStatusSnapshot {
        let now = Instant::now();
        if let Some(last) = self.last_refresh {
            if now.duration_since(last) < self.min_interval {
                return self.snapshot.clone();
            }
        }
        self.last_refresh = Some(now);

        if self.is_mock {
            self.refresh_mock();
        } else {
            #[cfg(windows)]
            {
                self.refresh_windows();
            }

            #[cfg(not(windows))]
            {
                self.refresh_mock();
            }
        }

        self.snapshot.clone()
    }

    #[cfg(windows)]
    fn refresh_windows(&mut self) {
        use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        use windows::Win32::System::SystemInformation::GetLocalTime;

        // 1. Query Battery Status
        let mut sps = SYSTEM_POWER_STATUS::default();
        let (battery_percent, is_charging) = unsafe {
            if GetSystemPowerStatus(&mut sps).is_ok() {
                let pct = if sps.BatteryLifePercent != 255 {
                    Some(sps.BatteryLifePercent.min(100))
                } else {
                    None // AC desktop without battery
                };
                let charging = pct.is_some()
                    && ((sps.BatteryFlag & 8) != 0
                        || (sps.ACLineStatus == 1 && sps.BatteryLifePercent < 100));
                (pct, charging)
            } else {
                (self.mock_battery, self.mock_charging)
            }
        };

        // 2. Query Local System Time
        let time_text = unsafe {
            let st = GetLocalTime();
            format_system_time(&st)
        };

        self.snapshot = SystemStatusSnapshot::new(
            battery_percent,
            is_charging,
            self.mock_wifi,
            self.mock_volume,
            time_text,
        );
    }

    fn refresh_mock(&mut self) {
        let time_text = format_mock_time();
        self.snapshot = SystemStatusSnapshot::new(
            self.mock_battery,
            self.mock_charging,
            self.mock_wifi,
            self.mock_volume,
            time_text,
        );
    }
}

#[cfg(windows)]
fn format_system_time(st: &windows::Win32::Foundation::SYSTEMTIME) -> String {
    let days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let day_name = days.get(st.wDayOfWeek as usize).unwrap_or(&"Thu");
    let month_name = months
        .get(st.wMonth.saturating_sub(1) as usize)
        .unwrap_or(&"Nov");

    let (hour_12, am_pm) = match st.wHour {
        0 => (12, "AM"),
        1..=11 => (st.wHour, "AM"),
        12 => (12, "PM"),
        _ => (st.wHour - 12, "PM"),
    };

    format!(
        "{} {} {} {}:{:02} {}",
        day_name, month_name, st.wDay, hour_12, st.wMinute, am_pm
    )
}

fn format_mock_time() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Deterministic lightweight local-approx time format without external chrono crate
    let minute = (secs / 60) % 60;
    let hour = ((secs / 3600) + 9) % 24; // KST approx or default UTC+9
    let (hour_12, am_pm) = match hour {
        0 => (12, "AM"),
        1..=11 => (hour, "AM"),
        12 => (12, "PM"),
        _ => (hour - 12, "PM"),
    };

    format!("Thu Nov 5 {}:{:02} {}", hour_12, minute, am_pm)
}
