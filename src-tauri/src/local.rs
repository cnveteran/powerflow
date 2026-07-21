use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{async_runtime, Manager, Runtime};
use tauri_plugin_pinia::ManagerExt;
use tauri_specta::Event;
use tokio::{select, sync::mpsc, time};
use tpower::{
    ffi::smc::{SMCConnection, SMCPowerData, SMCReadSensor},
    provider::{get_mac_ioreg, NormalizedResource},
};

use crate::event::{PowerUpdatedEvent, PreferenceEvent, StatusBarItem, WindowLoadedEvent};

pub enum SenderMessage {
    ImmediateSend,
    ChangeInterval(Duration),
    ChangeStatusBarItem(StatusBarItem),
    StatusBarShowCharging(bool),
}

/// Minimum (500 ms) and maximum (60 s) bounds for the power-tick interval.
///
/// Stale preference files (e.g. `updateInterval: 86400000` persisted by
/// earlier builds) would otherwise stall the chart for up to a day.
const MIN_INTERVAL_MS: u64 = 500;
const MAX_INTERVAL_MS: u64 = 60_000;

fn sanitize_interval_ms(ms: u64) -> Duration {
    if ms < MIN_INTERVAL_MS {
        log::warn!("updateInterval {ms}ms too small, clamped to {MIN_INTERVAL_MS}ms");
        Duration::from_millis(MIN_INTERVAL_MS)
    } else if ms > MAX_INTERVAL_MS {
        log::warn!("updateInterval {ms}ms too large, clamped to {MAX_INTERVAL_MS}ms");
        Duration::from_millis(MAX_INTERVAL_MS)
    } else {
        Duration::from_millis(ms)
    }
}

fn sanitize_interval(ms: u64) -> Duration {
    sanitize_interval_ms(ms)
}

pub fn status_bar_text(
    smc: &SMCPowerData,
    status_bar_item: &StatusBarItem,
    show_charging: bool,
) -> f32 {
    if smc.is_charging() && show_charging {
        return smc.delivery_rate;
    }
    match status_bar_item {
        StatusBarItem::System => smc.system_total,
        StatusBarItem::Screen => smc.brightness,
        StatusBarItem::Heatpipe => smc.heatpipe,
    }
}

impl PowerUpdatedEvent {
    pub fn new(value: f32) -> Self {
        Self(format!("{:.1} w", value))
    }

    pub fn new_with(
        smc: &SMCPowerData,
        status_bar_item: &StatusBarItem,
        show_charging: bool,
    ) -> Self {
        Self::new(status_bar_text(smc, status_bar_item, show_charging))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Event, Type)]
#[serde(rename_all = "camelCase")]
pub struct PowerTickEvent {
    pub data: NormalizedResource,
}

pub fn start_sender<R: Runtime>(
    app: &impl Manager<R>,
    mut rx: mpsc::Receiver<SenderMessage>,
) -> async_runtime::JoinHandle<()> {
    let app = app.app_handle().clone();
    let mut smc_conn = SMCConnection::new("AppleSMC").unwrap();

    let mut timer = time::interval(sanitize_interval(
        app.pinia()
            .try_get::<u64>("preference", "updateInterval")
            .unwrap_or(2000),
    ));
    let mut status_bar_item = app
        .pinia()
        .try_get::<StatusBarItem>("preference", "statusBarItem")
        .unwrap_or(StatusBarItem::System);
    let mut show_charging = app
        .pinia()
        .try_get::<bool>("preference", "showCharging")
        .unwrap_or(true);

    async_runtime::spawn(async move {
        loop {
            select! {
                _ = timer.tick() => {
                    let smc = smc_conn.read_sensor();
                    PowerUpdatedEvent::new_with(&smc, &status_bar_item, show_charging)
                        .emit(&app)
                        .unwrap();
                    match get_mac_ioreg() {
                        Ok(ioreg) => {
                            let data: NormalizedResource = (&ioreg, &smc).into();
                            PowerTickEvent { data }.emit(&app).unwrap();
                        }
                        Err(err) => {
                            log::error!("Failed to get IORegistry: {err}");
                        }
                    }
                }
                Some(msg) = rx.recv() => match msg {
                    SenderMessage::ImmediateSend => {
                        let smc = smc_conn.read_sensor();
                        PowerUpdatedEvent::new_with(&smc, &status_bar_item, show_charging)
                            .emit(&app)
                            .unwrap();
                        match get_mac_ioreg() {
                            Ok(ioreg) => {
                                PowerTickEvent {
                                    data: (&ioreg, &smc).into()
                                }.emit(&app).unwrap();
                            }
                            Err(err) => {
                                log::error!("Failed to get IORegistry: {err}");
                            }
                        }
                    },
                    SenderMessage::ChangeInterval(interval) => {
                        timer = time::interval(sanitize_interval_ms(
                            interval.as_millis() as u64,
                        ));
                    },
                    SenderMessage::ChangeStatusBarItem(item) => {
                        status_bar_item = item;
                        PowerUpdatedEvent::new_with(&smc_conn.read_sensor(), &status_bar_item, show_charging)
                            .emit(&app)
                            .unwrap();
                    },
                    SenderMessage::StatusBarShowCharging(show) => {
                        show_charging = show;
                        PowerUpdatedEvent::new_with(&smc_conn.read_sensor(), &status_bar_item, show_charging)
                            .emit(&app)
                            .unwrap();
                    }
                }
            }
        }
    })
}

pub fn setup_sender_with_events<R: Runtime>(app: &impl Manager<R>) {
    let app = app.app_handle();
    let (sender_tx, rx) = mpsc::channel(10);
    start_sender(app, rx);

    // send an immediate update when the main window is loaded
    let tx = sender_tx.clone();
    WindowLoadedEvent::listen(app, move |_| {
        let tx = tx.clone();
        async_runtime::spawn(async move {
            tx.send(SenderMessage::ImmediateSend).await.unwrap();
        });
    });

    let tx = sender_tx.clone();
    PreferenceEvent::listen(app, move |event| {
        if let Some(msg) = match event.payload {
            PreferenceEvent::UpdateInterval(interval) => Some(SenderMessage::ChangeInterval(
                Duration::from_millis(interval.into()),
            )),
            PreferenceEvent::StatusBarItem(item) => Some(SenderMessage::ChangeStatusBarItem(item)),
            PreferenceEvent::StatusBarShowCharging(show) => {
                Some(SenderMessage::StatusBarShowCharging(show))
            }
            PreferenceEvent::Language(_) => {
                // No need to send, perform some menu refreshing
                None
            }
            _ => None,
        } {
            let tx = tx.clone();
            async_runtime::spawn(async move {
                tx.send(msg).await.unwrap();
            });
        }
    });
}
