use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    mem::MaybeUninit,
    sync::{Arc, RwLock},
    time::Duration,
};

use derive_more::derive::Deref;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{async_runtime, AppHandle, Manager};
use tauri_specta::Event;
use tokio::{select, sync::mpsc, task::spawn_blocking, time};
use tpower::{
    ffi::{
        core_foundation::runloop::CFRunLoopRun,
        wrapper::{Device, ServiceConnection},
        AMDeviceNotificationCallbackInfo, AMDeviceNotificationSubscribe, Action, InterfaceType,
    },
    provider::{remote::get_device_ioreg, NormalizedResource},
};

use crate::event::DeviceEvent;

#[derive(Default, Deref)]
pub struct DeviceState(RwLock<HashMap<String, (String, HashSet<InterfaceType>)>>);

#[derive(Serialize, Deserialize, Debug, Clone, Event, Type)]
#[serde(rename_all = "camelCase")]
pub struct DevicePowerTickEvent {
    pub udid: String,
    pub data: NormalizedResource,
}

#[derive(Debug)]
pub struct DeviceMessage {
    device: Device,
    action: Action,
}

/// Active remote connections keyed by UDID — at most one tick source per device
/// so USB+WiFi dual attach does not double-feed history.
struct ConnectedDevice {
    device: Device,
    conn: ServiceConnection,
}

pub fn start_device_listener() -> mpsc::Receiver<DeviceMessage> {
    let (tx, rx) = mpsc::channel::<DeviceMessage>(10);

    extern "C" fn callback(info: *const AMDeviceNotificationCallbackInfo, context: *mut c_void) {
        let tx = unsafe { &*(context as *mut mpsc::Sender<DeviceMessage>) };
        let info = unsafe { *info };
        let device = unsafe { Device::new(info.device) };

        let tx = tx.clone();

        async_runtime::spawn(async move {
            if let Err(err) = tx
                .send(DeviceMessage {
                    device,
                    action: info.action,
                })
                .await
            {
                log::error!("Failed to send device message: {err}");
            }
            // Let Sender drop normally — do not mem::forget.
        });
    }

    spawn_blocking(move || {
        let boxed = Arc::new(tx);
        let mut not = MaybeUninit::uninit();
        let result = unsafe {
            AMDeviceNotificationSubscribe(
                callback,
                0,
                0,
                Arc::as_ptr(&boxed) as *mut _,
                not.as_mut_ptr(),
            )
        };
        if result != 0 {
            log::error!("AMDeviceNotificationSubscribe failed: {result}");
            return;
        }
        // Keep the Arc alive for the lifetime of the run loop.
        let _keep = boxed;
        unsafe { CFRunLoopRun() };
    });

    rx
}

fn prefer_usb(existing: InterfaceType, incoming: InterfaceType) -> bool {
    matches!(incoming, InterfaceType::USB) && !matches!(existing, InterfaceType::USB)
}

pub fn start_device_sender(handle: AppHandle) -> async_runtime::JoinHandle<()> {
    let mut rx = start_device_listener();
    let mut timer = time::interval(Duration::from_millis(2000));

    let mut devices: HashMap<String, ConnectedDevice> = HashMap::new();

    async_runtime::spawn(async move {
        loop {
            select! {
                _ = timer.tick() => {
                    for connected in devices.values() {
                        match get_device_ioreg(&connected.conn) {
                            Ok(res) => {
                                if let Err(err) = (DevicePowerTickEvent {
                                    udid: connected.device.udid.clone(),
                                    data: NormalizedResource::from(&res),
                                }).emit(&handle) {
                                    log::error!("Failed to emit DevicePowerTickEvent: {err}");
                                }
                            }
                            Err(err) => {
                                log::error!("Failed to get IORegistry: {err}");
                            }
                        }
                    }
                }
                Some(DeviceMessage { mut device, action }) = rx.recv() => {
                    match action {
                        Action::Attached => {
                            let udid = device.udid.clone();
                            if udid.is_empty() {
                                log::warn!("Ignoring attached device with empty UDID");
                                continue;
                            }

                            // Emit UI event even if we keep an existing connection.
                            let name_after_prepare;

                            if let Some(existing) = devices.get(&udid) {
                                if !prefer_usb(existing.device.interface_type, device.interface_type) {
                                    // Keep existing tick source; still notify UI of the interface.
                                    if let Err(err) = (DeviceEvent {
                                        udid: udid.clone(),
                                        name: existing.device.name(),
                                        interface: device.interface_type,
                                        action,
                                    }).emit(&handle) {
                                        log::error!("Failed to emit DeviceEvent: {err}");
                                    }
                                    continue;
                                }
                            }

                            if let Err(err) = device.prepare_device() {
                                log::error!("Failed to prepare device {udid}: {err}");
                                continue;
                            }

                            name_after_prepare = device.name();

                            let conn = match device.start_service("com.apple.mobile.diagnostics_relay") {
                                Ok(conn) => conn,
                                Err(err) => {
                                    log::error!("Failed to start diagnostics_relay on {udid}: {err}");
                                    continue;
                                }
                            };

                            if let Err(err) = (DeviceEvent {
                                udid: udid.clone(),
                                // must call `device.name()` after `device.prepare_device()`
                                name: name_after_prepare,
                                interface: device.interface_type,
                                action,
                            }).emit(&handle) {
                                log::error!("Failed to emit DeviceEvent: {err}");
                            }

                            devices.insert(udid, ConnectedDevice { device, conn });
                        },
                        Action::Detached => {
                            log::debug!("Device detached: {}", device.udid);
                            if let Err(err) = (DeviceEvent {
                                udid: device.udid.clone(),
                                name: String::new(),
                                interface: device.interface_type,
                                action,
                            }).emit(&handle) {
                                log::error!("Failed to emit DeviceEvent: {err}");
                            }

                            // Only drop the active connection if this detach matches
                            // the interface we are currently polling.
                            if let Some(connected) = devices.get(&device.udid) {
                                if connected.device.interface_type == device.interface_type {
                                    devices.remove(&device.udid);
                                }
                            }
                            // Detached wrapper never prepared a session — Drop is a no-op.
                        },
                        _ => ()
                    }
                }
            }
        }
    })
}

pub fn setup_device_listener(app: AppHandle) {
    DeviceEvent::listen(&app.clone(), move |event| {
        let event = event.payload;
        let app_state = app.state::<DeviceState>();

        use scopefn::Run;
        let mut guard = match app_state.write() {
            Ok(g) => g,
            Err(e) => {
                log::error!("DeviceState lock poisoned: {e}");
                return;
            }
        };
        guard
            .entry(event.udid.clone())
            .or_insert_with(|| (event.name, HashSet::new()))
            .run(|e| match event.action {
                Action::Attached => {
                    e.1.insert(event.interface);
                }
                Action::Detached => {
                    e.1.remove(&event.interface);
                    if e.1.is_empty() {
                        // Keep the map entry so name lookup still works briefly;
                        // empty interface set is the offline signal for the UI.
                    }
                }
                _ => (),
            });
    });
}
