use std::{
    collections::BTreeMap,
    f64::consts::TAU,
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::hardware::{dim, Device, Hardware, Rgb};

const FRAME_INTERVAL: Duration = Duration::from_millis(33);

#[derive(Clone, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub device_id: String,
    pub zone_id: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Assignment {
    pub target: Target,
    pub mode: String,
    pub color: Rgb,
    #[serde(default = "full_brightness")]
    pub brightness: u8,
    #[serde(default = "normal_speed")]
    pub speed: u16,
}

fn full_brightness() -> u8 {
    100
}

fn normal_speed() -> u16 {
    100
}

#[derive(Serialize)]
pub struct ApplyResult {
    pub count: usize,
    pub issues: Vec<String>,
    pub active: Vec<Assignment>,
}

enum Request {
    Devices(bool, Sender<Vec<Device>>),
    Apply(Vec<Assignment>, Sender<ApplyResult>),
    Shutdown(Sender<()>),
}

pub struct Lighting(Sender<Request>);

impl Lighting {
    pub fn start(app: AppHandle) -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || Worker::new(app).run(receiver));
        Self(sender)
    }

    pub async fn devices(&self, refresh: bool) -> Result<Vec<Device>, String> {
        self.request(|reply| Request::Devices(refresh, reply)).await
    }

    pub async fn apply(&self, assignments: Vec<Assignment>) -> Result<ApplyResult, String> {
        self.request(|reply| Request::Apply(assignments, reply))
            .await
    }

    pub fn shutdown(&self) {
        let (sender, receiver) = mpsc::channel();
        if self.0.send(Request::Shutdown(sender)).is_ok() {
            let _ = receiver.recv_timeout(Duration::from_secs(2));
        }
    }

    async fn request<T: Send + 'static>(
        &self,
        build: impl FnOnce(Sender<T>) -> Request,
    ) -> Result<T, String> {
        let (sender, receiver) = mpsc::channel();
        self.0
            .send(build(sender))
            .map_err(|_| "The lighting service is unavailable.")?;
        tauri::async_runtime::spawn_blocking(move || receiver.recv())
            .await
            .map_err(|error| error.to_string())?
            .map_err(|_| "The lighting service did not respond.".into())
    }
}

struct Animation {
    kind: AnimationKind,
    base: Rgb,
    brightness: u8,
    speed: f64,
    last: Option<Rgb>,
}

#[derive(Clone, Copy)]
enum AnimationKind {
    Pulse,
    Hue,
}

impl AnimationKind {
    fn from_mode(mode: &str) -> Option<Self> {
        match mode {
            "pulse" => Some(Self::Pulse),
            "hue" => Some(Self::Hue),
            _ => None,
        }
    }

    fn color(self, base: Rgb, seconds: f64) -> Rgb {
        match self {
            Self::Pulse => {
                let level = 0.15 + 0.85 * (0.5 - 0.5 * (seconds * TAU / 4.0).cos());
                base.map(|channel| (f64::from(channel) * level).round() as u8)
            }
            Self::Hue => {
                const COLORS: [Rgb; 6] = [
                    [255, 0, 0],
                    [255, 255, 0],
                    [0, 255, 0],
                    [0, 255, 255],
                    [0, 0, 255],
                    [255, 0, 255],
                ];
                let position = seconds.rem_euclid(6.0);
                let index = position.floor() as usize;
                let start = COLORS[index];
                let end = COLORS[(index + 1) % COLORS.len()];
                std::array::from_fn(|channel| {
                    let first = f64::from(start[channel]);
                    (first + (f64::from(end[channel]) - first) * position.fract()).round() as u8
                })
            }
        }
    }
}

struct Worker {
    app: AppHandle,
    hardware: Hardware,
    active: BTreeMap<Target, Assignment>,
    animations: BTreeMap<Target, Animation>,
    epoch: Instant,
}

impl Worker {
    fn new(app: AppHandle) -> Self {
        Self {
            app,
            hardware: Hardware::discover(),
            active: BTreeMap::new(),
            animations: BTreeMap::new(),
            epoch: Instant::now(),
        }
    }

    fn run(mut self, receiver: Receiver<Request>) {
        let mut next_frame = Instant::now();
        loop {
            let request = if self.animations.is_empty() {
                receiver.recv().map_err(|_| RecvTimeoutError::Disconnected)
            } else {
                receiver.recv_timeout(next_frame.saturating_duration_since(Instant::now()))
            };
            match request {
                Ok(Request::Devices(refresh, reply)) => {
                    if refresh {
                        self.refresh();
                    }
                    let _ = reply.send(self.hardware.devices());
                }
                Ok(Request::Apply(assignments, reply)) => {
                    let (result, frame_start) = self.apply(assignments);
                    if let Some(started) = frame_start {
                        next_frame = started + FRAME_INTERVAL;
                    }
                    let _ = reply.send(result);
                }
                Ok(Request::Shutdown(reply)) => {
                    drop(self);
                    let _ = reply.send(());
                    break;
                }
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
            let now = Instant::now();
            if !self.animations.is_empty() && now >= next_frame {
                for error in self.frame(now) {
                    let _ = self.app.emit("lighting-error", error);
                }
                next_frame = now + FRAME_INTERVAL;
            }
        }
    }

    fn refresh(&mut self) {
        // Release the old controller handles before enumerating again.
        self.hardware = Hardware::default();
        self.hardware = Hardware::discover();
        let mut stopped = Vec::new();
        let animations: Vec<_> = self
            .animations
            .keys()
            .filter_map(|target| self.active.get(target).cloned())
            .collect();
        self.animations.clear();
        for assignment in animations {
            if let Err(error) = self.apply_one(&assignment) {
                stopped.push(format!(
                    "{} / {} — {error}",
                    assignment.target.device_id, assignment.target.zone_id
                ));
            }
        }
        if !stopped.is_empty() {
            let _ = self.app.emit(
                "lighting-error",
                format!("Animation stopped: {}", stopped.join(", ")),
            );
        }
    }

    fn apply(&mut self, assignments: Vec<Assignment>) -> (ApplyResult, Option<Instant>) {
        let targets: BTreeMap<_, _> = assignments
            .into_iter()
            .map(|assignment| (assignment.target.clone(), assignment))
            .collect();
        let mut issues = Vec::new();
        let mut changed = Vec::new();
        for (target, assignment) in targets {
            match self.apply_one(&assignment) {
                Ok(()) => {
                    self.active.insert(target.clone(), assignment);
                    changed.push(target);
                }
                Err(error) => issues.push(format!(
                    "{} / {} — {error}",
                    target.device_id, target.zone_id
                )),
            }
        }
        let frame_start = (!self.animations.is_empty()).then(Instant::now);
        if let Some(started) = frame_start {
            issues.extend(self.frame(started));
        }
        (
            ApplyResult {
                count: changed
                    .iter()
                    .filter(|target| self.active.contains_key(*target))
                    .count(),
                issues,
                active: self.active.values().cloned().collect(),
            },
            frame_start,
        )
    }

    fn apply_one(&mut self, assignment: &Assignment) -> Result<(), String> {
        let target = &assignment.target;
        let controller = self
            .hardware
            .controller(&target.device_id)
            .ok_or("Device unavailable.")?;
        let zone = controller
            .device()
            .zones
            .iter()
            .find(|zone| zone.id == target.zone_id)
            .ok_or("Zone unavailable.")?;
        let mode = assignment.mode.as_str();
        let brightness = assignment.brightness.min(100);
        if let Some(kind) = AnimationKind::from_mode(mode) {
            if !zone.direct {
                return Err("Animation is unavailable on this zone.".into());
            }
            let base = assignment.color;
            let speed = f64::from(assignment.speed.clamp(25, 250)) / 100.0;
            if !self.animations.contains_key(target) {
                let initial = dim(
                    kind.color(base, self.epoch.elapsed().as_secs_f64() * speed),
                    brightness,
                );
                controller.begin_animation(&target.zone_id, initial)?;
            }
            self.animations.insert(
                target.clone(),
                Animation {
                    kind,
                    base,
                    brightness,
                    speed,
                    last: None,
                },
            );
            return Ok(());
        }
        if mode != "off" && mode != "solid" && !zone.effects.iter().any(|effect| effect.id == mode)
        {
            return Err("Effect unavailable.".into());
        }
        controller.apply(&target.zone_id, mode, assignment.color, brightness)?;
        self.animations.remove(target);
        Ok(())
    }

    fn frame(&mut self, now: Instant) -> Vec<String> {
        let phase = now.duration_since(self.epoch).as_secs_f64();
        let mut batches: BTreeMap<String, Vec<(Target, Rgb)>> = BTreeMap::new();
        for (target, animation) in &self.animations {
            let color = dim(
                animation
                    .kind
                    .color(animation.base, phase * animation.speed),
                animation.brightness,
            );
            if Some(color) != animation.last {
                batches
                    .entry(target.device_id.clone())
                    .or_default()
                    .push((target.clone(), color));
            }
        }
        // Start each transport concurrently for the same sampled phase. Wait for
        // this batch before advancing so a slow controller cannot accumulate frames.
        let results = thread::scope(|scope| {
            let mut jobs = Vec::new();
            for controller in &mut self.hardware.controllers {
                let device = controller.device().id.clone();
                if let Some(batch) = batches.remove(&device) {
                    jobs.push((
                        device,
                        scope.spawn(move || {
                            let updates: Vec<_> = batch
                                .iter()
                                .map(|(target, color)| (target.zone_id.as_str(), *color))
                                .collect();
                            let result = controller.frames(&updates);
                            (batch, result)
                        }),
                    ));
                }
            }
            jobs.into_iter()
                .map(|(device, job)| {
                    let (batch, result) = job
                        .join()
                        .unwrap_or_else(|_| (Vec::new(), Err("Controller worker failed.".into())));
                    (device, batch, result)
                })
                .collect::<Vec<_>>()
        });
        let mut failures = Vec::new();
        for (device, batch, result) in results {
            match result {
                Ok(()) => {
                    for (target, color) in batch {
                        if let Some(animation) = self.animations.get_mut(&target) {
                            animation.last = Some(color);
                        }
                    }
                }
                Err(error) => {
                    self.animations.retain(|target, _| {
                        let retain = target.device_id != device;
                        if !retain {
                            self.active.remove(target);
                        }
                        retain
                    });
                    failures.push(format!("Animation stopped ({device}) — {error}"));
                }
            }
        }
        failures
    }
}
