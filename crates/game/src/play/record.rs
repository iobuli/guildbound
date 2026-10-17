//! Smoke-test recorder: `GB_RECORD=<dir>` makes every frame advance game time by exactly
//! 1 / `GB_RECORD_FPS` s (default 30) and saves each frame from `GB_RECORD_FROM` to
//! `GB_RECORD_TO` game seconds (defaults 3 and 15) as `<dir>/f_00000.png`..., then quits once the
//! last one is written. However slowly the machine renders, the frames play back at normal speed
//! (`ffmpeg -framerate 30 -i <dir>/f_%05d.png`). Nothing here runs without `GB_RECORD`.
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::time::TimeUpdateStrategy;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[derive(Resource)]
struct Recording {
    dir: String,
    from: f32,
    to: f32,
    requested: usize,
    saved: Arc<AtomicUsize>,
    done: bool,
}

fn env_f32(name: &str, default: f32) -> f32 {
    std::env::var(name).ok().and_then(|s| s.trim().parse().ok()).filter(|v: &f32| v.is_finite()).unwrap_or(default)
}

pub fn plugin(app: &mut App) {
    let Some(dir) = std::env::var("GB_RECORD").ok().filter(|s| !s.trim().is_empty()) else { return };
    let fps = env_f32("GB_RECORD_FPS", 30.0).clamp(1.0, 240.0);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        error!("[record] {dir}: {e}");
        return;
    }
    info!("[record] {dir} at {fps} fps");
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / fps)))
        .insert_resource(Recording {
            dir,
            from: env_f32("GB_RECORD_FROM", 3.0),
            to: env_f32("GB_RECORD_TO", 15.0),
            requested: 0,
            saved: Arc::new(AtomicUsize::new(0)),
            done: false,
        })
        .add_systems(Last, record);
}

fn record(time: Res<Time<Virtual>>, mut rec: ResMut<Recording>, mut commands: Commands, mut exit: MessageWriter<AppExit>) {
    let t = time.elapsed_secs();
    if rec.done {
        return;
    }
    if t >= rec.from && t <= rec.to {
        let path = format!("{}/f_{:05}.png", rec.dir, rec.requested);
        rec.requested += 1;
        let saved = rec.saved.clone();
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path))
            .observe(move |_: On<ScreenshotCaptured>| {
                saved.fetch_add(1, Ordering::Relaxed);
            });
    } else if t > rec.to && rec.saved.load(Ordering::Relaxed) >= rec.requested {
        info!("[record] {} frames", rec.requested);
        rec.done = true;
        exit.write(AppExit::Success);
    }
}
