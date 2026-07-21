use std::{collections::HashMap, mem, ops::Div};

use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;
use tauri::{async_runtime, AppHandle, Manager};
use tauri_specta::{Event, TypedEvent};
use tokio::sync::mpsc;
use tpower::{
    provider::{NormalizedData, NormalizedResource},
    util::get_mac_name,
};

use crate::{
    database::save_charging_history,
    device::{DevicePowerTickEvent, DeviceState},
    local::PowerTickEvent,
};

/// Cap overnight charges (~2s interval → ~2h of samples) to bound RAM / BLOB size.
const MAX_STAGED_SAMPLES: usize = 3600;

struct ChargingHistoryStage {
    data: NormalizedResource,
    raw: String,
}

#[derive(Serialize, Deserialize, Type)]
pub struct ChargingHistory {
    pub is_remote: bool,
    pub name: String,
    pub udid: String,
    pub from_level: i32,
    pub end_level: i32,
    pub duration: i64,
    pub timestamp: i64,
    pub adapter_name: String,
    pub detail: ChargingHistoryDetail,
}

#[derive(Serialize, Deserialize, Type)]
pub struct ChargingHistoryDetail {
    avg: NormalizedData,
    peak: NormalizedData,
    curve: Vec<NormalizedResource>,
    raw: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Type, Event)]
pub struct HistoryRecordedEvent;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum DeviceType {
    Local,
    Remote(String),
}

fn summrize_history(
    app: &AppHandle,
    staged: Vec<ChargingHistoryStage>,
    typ: DeviceType,
) -> Option<ChargingHistory> {
    let name = match typ {
        DeviceType::Local => get_mac_name(),
        DeviceType::Remote(ref udid) => app
            .state::<DeviceState>()
            .read()
            .ok()
            .and_then(|s| s.get(udid).map(|d| d.0.clone())),
    }
    .unwrap_or_default();

    let (first, last) = (staged.first()?, staged.last()?);

    let from_level = first.data.battery_level;
    let end_level = last.data.battery_level;
    let timestamp = first.data.last_update;
    // Guard against clock skew / out-of-order samples.
    let duration = (last.data.last_update - timestamp).max(0);

    let adapter_name = last
        .data
        .adapter_name
        .clone()
        .unwrap_or("Unknown".to_string());

    let avg = staged
        .iter()
        .fold(NormalizedData::default(), |acc, cur| acc + *cur.data)
        .div(staged.len() as f32);
    let peak = staged.iter().fold(NormalizedData::default(), |acc, cur| {
        acc.max_with(&cur.data)
    });
    let (curve, raw) = staged.into_iter().map(|s| (s.data, s.raw)).unzip();

    Some(ChargingHistory {
        is_remote: matches!(typ, DeviceType::Remote(_)),
        name,
        udid: match typ {
            DeviceType::Local => "local".to_string(),
            DeviceType::Remote(ref udid) => udid.clone(),
        },
        from_level,
        end_level,
        duration,
        timestamp,
        adapter_name,
        detail: ChargingHistoryDetail {
            avg,
            peak,
            curve,
            raw,
        },
    })
}

fn push_stage(staged: &mut Vec<ChargingHistoryStage>, data: NormalizedResource) {
    if staged.len() >= MAX_STAGED_SAMPLES {
        let drop_n = staged.len() / 4;
        staged.drain(0..drop_n);
        log::warn!(
            "charging history staging buffer capped; dropped {drop_n} oldest samples"
        );
    }
    let raw = match serde_json::to_string(&data) {
        Ok(s) => s,
        Err(err) => {
            log::error!("Failed to serialize charging sample: {err}");
            String::new()
        }
    };
    staged.push(ChargingHistoryStage { raw, data });
}

fn spawn_history_recorder(
    app: AppHandle,
    mut rx: mpsc::Receiver<(DeviceType, NormalizedResource)>,
) {
    async_runtime::spawn(async move {
        let db = app.state::<SqlitePool>();
        let mut staged: HashMap<DeviceType, Vec<ChargingHistoryStage>> = HashMap::new();

        while let Some((typ, data)) = rx.recv().await {
            let full_charged = data.battery_level >= 100;
            let staged = staged.entry(typ.clone()).or_default();

            let was_charging = staged
                .last()
                .map(|last| last.data.is_charging)
                .unwrap_or(false);
            let unplugged = was_charging && !data.is_charging;
            let already_staging = !staged.is_empty();
            // Do not open a new session that starts already full; do allow
            // appending the 100% sample onto an in-progress charge.
            let should_stage = data.is_charging
                && (already_staging || !full_charged)
                && staged
                    .last()
                    .map(|last| data.last_update != last.data.last_update)
                    .unwrap_or(true);

            // Stage while charging — including the 100% sample — before finalize.
            if should_stage {
                log::info!("staged: {:#?}", staged.len() + 1);
                push_stage(staged, data);
            }

            let reached_full = full_charged
                && !staged.is_empty()
                && staged
                    .last()
                    .map(|s| s.data.battery_level >= 100)
                    .unwrap_or(false);

            if unplugged || reached_full {
                let taked = mem::take(staged);
                // filter out short history
                if taked.len() <= 2 {
                    continue;
                }

                let Some(history) = summrize_history(app.app_handle(), taked, typ) else {
                    continue;
                };

                match save_charging_history(&db, &history).await {
                    Ok(res) => {
                        log::info!(
                            "history of {} saved: {}",
                            history.udid,
                            res.last_insert_rowid()
                        );
                    }
                    Err(e) => {
                        log::error!("history save failed: {:#?}", e);
                    }
                }

                HistoryRecordedEvent.emit(&app).unwrap_or_else(|err| {
                    log::error!("Failed to emit HistoryRecordedEvent: {:?}", err)
                });
            }
        }
    });
}

pub fn setup_history_recorder(app: AppHandle) {
    let (tx, rx) = mpsc::channel(10);
    let tx_cloned = tx.clone();
    PowerTickEvent::listen(&app, move |TypedEvent { payload, .. }| {
        let tx = tx_cloned.clone();
        async_runtime::spawn(async move {
            tx.send((DeviceType::Local, payload.data))
                .await
                .unwrap_or_else(|err| {
                    log::error!("Failed to send PowerTickEvent: {:#?}", err);
                })
        });
    });

    let tx_cloned = tx.clone();
    DevicePowerTickEvent::listen(&app, move |TypedEvent { payload, .. }| {
        let tx = tx_cloned.clone();
        async_runtime::spawn(async move {
            tx.send((DeviceType::Remote(payload.udid), payload.data))
                .await
                .unwrap_or_else(|err| {
                    log::error!("Failed to send DevicePowerTickEvent: {:#?}", err);
                })
        });
    });
    spawn_history_recorder(app.clone(), rx);
}
