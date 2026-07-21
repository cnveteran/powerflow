use std::{
    collections::VecDeque,
    ffi::CString,
    mem,
    ops::{Deref, Div},
    time::Duration,
};

use anyhow::bail;
use core_foundation::{
    base::{kCFAllocatorDefault, mach_port_t, TCFType},
    dictionary::{CFDictionary, CFMutableDictionaryRef},
};
use derive_more::Add;
use io_kit_sys::{
    ret::kIOReturnSuccess, IOMasterPort, IOObjectRelease, IORegistryEntryCreateCFProperties,
    IOServiceGetMatchingService, IOServiceMatching,
};
use ratatui::widgets::SparklineBar;
use serde::{Deserialize, Serialize};

use crate::{
    de::{repr, IORegistry},
    ffi::{smc::SMCPowerData, InterfaceType},
    util::{dict_into, skip_until},
};

pub mod remote;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NormalizedResource {
    pub is_local: bool,
    pub is_charging: bool,
    pub time_remain: Duration,
    pub last_update: i64,
    pub adapter_name: Option<String>,
    pub cycle_count: i32,
    pub current_capacity: i32,
    pub max_capacity: i32,
    #[serde(default)]
    pub design_capacity: i32,
    #[serde(flatten)]
    pub data: NormalizedData,
}

#[derive(Debug, Clone, Copy, Default, Add, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NormalizedData {
    pub system_in: f32,
    pub system_load: f32,
    pub battery_power: f32,
    pub adapter_power: f32,
    pub efficiency_loss: f32,
    /// 0 if not available
    pub brightness_power: f32,
    /// 0 if not available
    pub heatpipe_power: f32,
    pub battery_level: i32,
    pub absolute_battery_level: f32,
    pub temperature: f32,

    pub adapter_watts: f32,
    pub adapter_voltage: f32,
    pub adapter_amperage: f32,
}

impl NormalizedData {
    pub fn max_with(self, other: &Self) -> Self {
        Self {
            system_in: self.system_in.max(other.system_in),
            system_load: self.system_load.max(other.system_load),
            battery_power: self.battery_power.max(other.battery_power),
            adapter_power: self.adapter_power.max(other.adapter_power),
            efficiency_loss: self.efficiency_loss.max(other.efficiency_loss),
            battery_level: self.battery_level.max(other.battery_level),
            absolute_battery_level: self
                .absolute_battery_level
                .max(other.absolute_battery_level),
            temperature: self.temperature.max(other.temperature),
            brightness_power: self.brightness_power.max(other.brightness_power),
            heatpipe_power: self.heatpipe_power.max(other.heatpipe_power),
            adapter_watts: self.adapter_watts.max(other.adapter_watts),
            adapter_voltage: self.adapter_voltage.max(other.adapter_voltage),
            adapter_amperage: self.adapter_amperage.max(other.adapter_amperage),
        }
    }
}

impl Div<f32> for NormalizedData {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self {
            system_in: self.system_in / rhs,
            system_load: self.system_load / rhs,
            battery_power: self.battery_power / rhs,
            adapter_power: self.adapter_power / rhs,
            efficiency_loss: self.efficiency_loss / rhs,
            brightness_power: self.brightness_power / rhs,
            heatpipe_power: self.heatpipe_power / rhs,
            battery_level: self.battery_level / rhs as i32,
            absolute_battery_level: self.absolute_battery_level / rhs,
            temperature: self.temperature / rhs,
            adapter_watts: self.adapter_watts / rhs,
            adapter_voltage: self.adapter_voltage / rhs,
            adapter_amperage: self.adapter_amperage / rhs,
        }
    }
}

impl Deref for NormalizedResource {
    type Target = NormalizedData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl From<&IORegistry> for NormalizedResource {
    fn from(io: &IORegistry) -> Self {
        let (system_in, system_load, battery_power, adapter_power, efficiency_loss) =
            if let Some(d) = io.ptd() {
                (
                    d.system_power_in as f32 / 1000.,
                    d.system_load as f32 / 1000.,
                    d.battery_power as f32 / 1000.,
                    (d.system_power_in + d.adapter_efficiency_loss) as f32 / 1000.,
                    d.adapter_efficiency_loss as f32 / 1000.,
                )
            } else {
                Default::default()
            };

        Self {
            is_local: false,
            is_charging: io.is_charging,
            // Same sentinel handling as local — iOS often reports -1 / 65535
            // while TimeRemaining is still computing.
            time_remain: time_remain_iokit(io.time_remaining).unwrap_or(Duration::ZERO),
            last_update: io.update_time,
            adapter_name: io
                .adapter_details
                .name
                .clone()
                .or_else(|| io.adapter_details.description.clone()),
            cycle_count: io.cycle_count,
            max_capacity: io.apple_raw_max_capacity,
            design_capacity: io.design_capacity,
            current_capacity: io.apple_raw_current_capacity,
            data: NormalizedData {
                system_in,
                system_load,
                battery_power,
                adapter_power,
                efficiency_loss,
                brightness_power: 0.,
                heatpipe_power: 0.,
                battery_level: io.current_capacity,
                absolute_battery_level: absolute_battery_level(io),
                temperature: io.temperature as f32 / 100.,

                adapter_watts: io.adapter_details.watts.unwrap_or_default() as f32,
                adapter_voltage: io.adapter_details.adapter_voltage.unwrap_or_default() as f32
                    / 1000.,
                adapter_amperage: io.adapter_details.current.unwrap_or_default() as f32 / 1000.,
            },
        }
    }
}

impl From<(&IORegistry, &SMCPowerData)> for NormalizedResource {
    fn from((io, smc): (&IORegistry, &SMCPowerData)) -> Self {
        Self {
            is_local: true,
            last_update: io.update_time,
            // Prefer amperage / IOKit over SMC CHCC — on macOS 27 CHCC can
            // stay true while the battery is discharging.
            is_charging: is_charging_local(io, smc),
            // Prefer IORegistry's TimeRemaining (updated by IOKit every few
            // seconds) over SMC's B0TE/B0TF (which can stay stale for many
            // minutes). IOKit uses -1 (or a very large value) to signal
            // "still computing" / "unknown"; fall back to SMC in that case.
            time_remain: time_remain_from(io, smc),
            adapter_name: io
                .adapter_details
                .name
                .clone()
                .or_else(|| io.adapter_details.description.clone()),
            cycle_count: io.cycle_count,
            max_capacity: io.apple_raw_max_capacity,
            design_capacity: io.design_capacity,
            current_capacity: io.apple_raw_current_capacity,
            data: NormalizedData {
                system_in: smc.delivery_rate,
                system_load: smc.system_total,
                battery_power: smc.battery_rate.max(smc.delivery_rate - smc.system_total),
                efficiency_loss: io
                    .ptd()
                    .map_or(0.0, |d| d.adapter_efficiency_loss as f32 / 1000.),
                brightness_power: smc.brightness,
                heatpipe_power: smc.heatpipe,
                battery_level: io.current_capacity,
                absolute_battery_level: absolute_battery_level(io),
                temperature: smc.temperature,
                adapter_power: smc.delivery_rate
                    + io.ptd()
                        .map_or(0.0, |d| d.adapter_efficiency_loss as f32 / 1000.),

                adapter_watts: io.adapter_details.watts.unwrap_or_default() as f32,
                adapter_voltage: io.adapter_details.adapter_voltage.unwrap_or_default() as f32
                    / 1000.,
                adapter_amperage: io.adapter_details.current.unwrap_or_default() as f32 / 1000.,
            },
        }
    }
}

const IOKIT_UNKNOWN: i32 = -1;
const IOKIT_SENTINEL: i32 = 65535;
const MAX_PLAUSIBLE_MIN: i32 = 24 * 60;

/// Convert IOKit `TimeRemaining` (minutes) to a Duration, rejecting sentinels.
fn time_remain_iokit(minutes: i32) -> Option<Duration> {
    if minutes != IOKIT_UNKNOWN
        && minutes != IOKIT_SENTINEL
        && (1..=MAX_PLAUSIBLE_MIN).contains(&minutes)
    {
        Some(Duration::from_secs((minutes as u64) * 60))
    } else {
        None
    }
}

/// Local charging detection that does not trust SMC `CHCC` alone.
fn is_charging_local(io: &IORegistry, smc: &SMCPowerData) -> bool {
    // Instant amperage sign is the most reliable signal when present.
    if io.instant_amperage != 0 {
        return io.instant_amperage > 0;
    }
    if io.amperage != 0 {
        return io.amperage > 0;
    }
    // Fall back to IOKit flag, then SMC only if IOKit is silent.
    io.is_charging || smc.is_charging()
}

/// Pick the most reliable remaining-time estimate.
///
/// IOKit exposes `TimeRemaining` (minutes) via `AppleSmartBattery` and
/// refreshes it every few seconds; SMC's `B0TE`/`B0TF` are firmware-level
/// and can stay pinned for many minutes on Apple Silicon. We therefore
/// prefer the IOKit value and only fall back to SMC when IOKit reports an
/// invalid sentinel (`-1`, `65535`, or an unreasonable magnitude).
///
/// NOTE: we intentionally do NOT use `smc.is_charging()` to pick between
/// `time_to_empty` and `time_to_full`. On macOS 27 the SMC `CHCC` key can
/// report charging=true while the battery is actually discharging
/// (amperage < 0), which would cause us to pick the wrong sentinel and
/// display "1000+ hours to full". Instead we try both SMC values and
/// keep whichever is plausible.
fn time_remain_from(io: &IORegistry, smc: &SMCPowerData) -> Duration {
    if let Some(d) = time_remain_iokit(io.time_remaining) {
        return d;
    }

    // Try both SMC values; the one that's not a sentinel is the right one.
    for smc_min in [smc.time_to_empty, smc.time_to_full] {
        if smc_min.is_finite()
            && smc_min > 0.0
            && smc_min < MAX_PLAUSIBLE_MIN as f32
            && (smc_min as i32) != IOKIT_SENTINEL
        {
            return Duration::from_secs_f32(60.0 * smc_min);
        }
    }

    Duration::ZERO
}

pub fn get_mac_ioreg_dict() -> anyhow::Result<CFDictionary> {
    let mut master_port: mach_port_t = 0;
    if unsafe { IOMasterPort(0, &mut master_port) } != 0 {
        bail!("could not get master port");
    }
    let name = CString::new("AppleSmartBattery").unwrap();
    let matching_dict = unsafe { IOServiceMatching(name.as_ptr()) };

    let service = unsafe { IOServiceGetMatchingService(master_port, matching_dict) };
    if service == 0 {
        bail!("AppleSmartBattery service not found");
    }

    let mut properties: CFMutableDictionaryRef = unsafe { mem::zeroed() };
    let kr = unsafe {
        IORegistryEntryCreateCFProperties(service, &mut properties, kCFAllocatorDefault, 0)
    };
    // IOServiceGetMatchingService returns a retained object — always release.
    unsafe { IOObjectRelease(service) };

    if kr != kIOReturnSuccess {
        bail!("could not get properties");
    }

    unsafe { Ok(CFDictionary::wrap_under_create_rule(properties)) }
}

pub fn get_mac_ioreg() -> anyhow::Result<IORegistry> {
    let dic = get_mac_ioreg_dict()?;
    unsafe { mem::transmute(dict_into::<repr::IORegistry>(dic)) }
}

/// Compute the absolute battery level as a percentage (0-100).
///
/// On macOS 27+ `AppleRawCurrentCapacity` / `AppleRawMaxCapacity` are no
/// longer exposed by `AppleSmartBattery`, so the raw-capacity ratio is
/// unavailable. Fall back to the `CurrentCapacity` / `MaxCapacity` pair
/// (already a 0-100 percentage) and finally to 0.0 if neither is usable.
fn absolute_battery_level(io: &IORegistry) -> f32 {
    if io.apple_raw_max_capacity > 0 {
        io.apple_raw_current_capacity as f32 / io.apple_raw_max_capacity as f32 * 100.
    } else if io.max_capacity > 0 {
        io.current_capacity as f32 / io.max_capacity as f32 * 100.
    } else {
        io.current_capacity as f32
    }
}

#[derive(Debug)]
pub struct MergedPowerData {
    pub from: PowerDataFrom,
    pub smc: Option<SMCPowerData>,
    pub ioreg: IORegistry,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PowerDataFrom {
    #[default]
    Local,
    Remote((String, String, InterfaceType)),
}

impl Deref for MergedPowerData {
    type Target = IORegistry;

    fn deref(&self) -> &Self::Target {
        &self.ioreg
    }
}

#[derive(Debug, Default)]
pub struct PowerStatistic {
    pub max_battery_power: f32,
    pub max_input_power: f32,
    pub max_system_power: f32,

    pub battery_history: VecDeque<u64>,
    pub input_history: VecDeque<u64>,
    pub system_history: VecDeque<u64>,
}

impl PowerStatistic {
    pub fn update(&mut self, battery_power: f32, input_power: f32, system_power: f32) {
        if battery_power > self.max_battery_power {
            self.max_battery_power = battery_power;
        }

        if input_power > self.max_input_power {
            self.max_input_power = input_power;
        }

        if system_power > self.max_system_power {
            self.max_system_power = system_power;
        }

        self.battery_history.push_back(battery_power.abs() as u64);
        if self.battery_history.len() > 50 {
            self.battery_history.pop_front();
        }

        self.input_history.push_back(input_power.abs() as u64);
        if self.input_history.len() > 50 {
            self.input_history.pop_front();
        }

        self.system_history.push_back(system_power.abs() as u64);
        if self.system_history.len() > 200 {
            self.system_history.pop_front();
        }
    }

    pub fn battery_history(&self, width: usize) -> Vec<SparklineBar> {
        skip_until(self.battery_history.iter(), width)
            .map(|v| SparklineBar::from(*v))
            .collect()
    }

    pub fn input_history(&self, width: usize) -> Vec<SparklineBar> {
        skip_until(self.input_history.iter(), width)
            .map(|v| SparklineBar::from(*v))
            .collect()
    }

    pub fn system_history(&self, width: usize) -> Vec<SparklineBar> {
        skip_until(self.system_history.iter(), width)
            .map(|v| SparklineBar::from(*v))
            .collect()
    }
}
