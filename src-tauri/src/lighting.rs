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
const COLOR_CYCLE_SECONDS: f64 = 6.0;

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

#[derive(Clone, Serialize)]
pub struct ApplyResult {
    pub revision: u64,
    pub count: usize,
    pub issues: Vec<String>,
    pub active: Vec<Assignment>,
}

#[derive(Serialize)]
pub struct Discovery {
    devices: Vec<Device>,
    state: ApplyResult,
}

enum Request {
    Active(Sender<Vec<Assignment>>),
    Devices(bool, Sender<Discovery>),
    Apply(Vec<Assignment>, Sender<ApplyResult>),
    Shutdown(Sender<()>),
}

pub struct Lighting(Sender<Request>);

impl Lighting {
    pub fn start(app: AppHandle, startup: Result<Option<Vec<Assignment>>, String>) -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut worker = Worker::new(app);
            worker.initial_issues = match startup {
                Ok(Some(assignments)) => worker.apply(assignments).0.issues,
                Ok(None) => Vec::new(),
                Err(error) => vec![error],
            };
            worker.run(receiver);
        });
        Self(sender)
    }

    pub async fn active(&self) -> Result<Vec<Assignment>, String> {
        self.request(Request::Active).await
    }

    pub async fn devices(&self, refresh: bool) -> Result<Discovery, String> {
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

impl Animation {
    fn frame(&self, phase: f64) -> Rgb {
        dim(
            self.kind.color(self.base, phase * self.speed),
            self.brightness,
        )
    }
}

#[derive(Clone, Copy)]
enum AnimationKind {
    Pulse,
    Hue,
    Rainbow(f64),
    GtaVi(f64),
}

impl AnimationKind {
    fn from_mode(mode: &str) -> Option<Self> {
        match mode {
            "pulse" => Some(Self::Pulse),
            "hue" => Some(Self::Hue),
            "sync-rainbow" => Some(Self::Rainbow(0.0)),
            "gta-vi" => Some(Self::GtaVi(0.0)),
            _ => None,
        }
    }

    fn color(self, base: Rgb, seconds: f64) -> Rgb {
        match self {
            Self::Pulse => {
                let level = 0.15 + 0.85 * (0.5 - 0.5 * (seconds * TAU / 4.0).cos());
                base.map(|channel| (f64::from(channel) * level).round() as u8)
            }
            Self::Hue | Self::Rainbow(_) => {
                const COLORS: [Rgb; 6] = [
                    [255, 0, 0],
                    [255, 255, 0],
                    [0, 255, 0],
                    [0, 255, 255],
                    [0, 0, 255],
                    [255, 0, 255],
                ];
                let offset = match self {
                    Self::Rainbow(offset) => offset,
                    _ => 0.0,
                };
                cycle_color(&COLORS, seconds + offset)
            }
            Self::GtaVi(offset) => {
                const SUNSET: [Rgb; 8] = [
                    [74, 55, 255],
                    [125, 25, 234],
                    [194, 25, 193],
                    [239, 80, 128],
                    [255, 165, 66],
                    [239, 80, 128],
                    [194, 25, 193],
                    [125, 25, 234],
                ];
                let phase = (seconds / 2.0 + offset) / COLOR_CYCLE_SECONDS;
                cycle_color(&SUNSET, phase * SUNSET.len() as f64)
            }
        }
    }
}

fn cycle_color(colors: &[Rgb], position: f64) -> Rgb {
    let position = position.rem_euclid(colors.len() as f64);
    let index = position.floor() as usize;
    let start = colors[index];
    let end = colors[(index + 1) % colors.len()];
    std::array::from_fn(|channel| {
        let first = f64::from(start[channel]);
        (first + (f64::from(end[channel]) - first) * position.fract()).round() as u8
    })
}

struct Worker {
    app: AppHandle,
    hardware: Hardware,
    active: BTreeMap<Target, Assignment>,
    animations: BTreeMap<Target, Animation>,
    revision: u64,
    initial_issues: Vec<String>,
    epoch: Instant,
}

impl Worker {
    fn new(app: AppHandle) -> Self {
        Self {
            app,
            hardware: Hardware::discover(),
            active: BTreeMap::new(),
            animations: BTreeMap::new(),
            revision: 0,
            initial_issues: Vec::new(),
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
                    let mut issues = std::mem::take(&mut self.initial_issues);
                    if refresh {
                        issues.extend(self.refresh());
                    }
                    let _ = reply.send(Discovery {
                        devices: self.hardware.devices(),
                        state: self.snapshot(0, issues),
                    });
                }
                Ok(Request::Active(reply)) => {
                    let _ = reply.send(self.active.values().cloned().collect());
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
                let issues = self.frame(now);
                if !issues.is_empty() {
                    self.revision += 1;
                    let _ = self.app.emit("lighting-error", self.snapshot(0, issues));
                }
                next_frame = now + FRAME_INTERVAL;
            }
        }
    }

    fn refresh(&mut self) -> Vec<String> {
        self.revision += 1;
        // Release the old controller handles before enumerating again.
        self.hardware = Hardware::default();
        self.hardware = Hardware::discover();
        let mut stopped = Vec::new();
        let devices = self.hardware.devices();
        self.active.retain(|target, _| {
            let present = devices.iter().any(|device| device.id == target.device_id
                && device.zones.iter().any(|zone| zone.id == target.zone_id));
            if !present {
                stopped.push(format!("{} / {} — Zone unavailable.", target.device_id, target.zone_id));
            }
            present
        });
        let animations: Vec<_> = self
            .animations
            .keys()
            .filter_map(|target| self.active.get(target).cloned())
            .collect();
        self.animations.clear();
        for assignment in animations {
            if let Err(error) = self.apply_one(&assignment) {
                self.active.remove(&assignment.target);
                stopped.push(format!(
                    "{} / {} — {error}",
                    assignment.target.device_id, assignment.target.zone_id
                ));
            }
        }
        stopped
    }

    fn snapshot(&self, count: usize, issues: Vec<String>) -> ApplyResult {
        ApplyResult { revision: self.revision, count, issues, active: self.active.values().cloned().collect() }
    }

    fn apply(&mut self, assignments: Vec<Assignment>) -> (ApplyResult, Option<Instant>) {
        self.revision += 1;
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
                revision: self.revision,
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
        let mode = assignment.mode.as_str();
        let animation = match AnimationKind::from_mode(mode) {
            Some(AnimationKind::Rainbow(_)) => {
                Some(AnimationKind::Rainbow(self.zone_phase_offset(target)))
            }
            Some(AnimationKind::GtaVi(_)) => {
                Some(AnimationKind::GtaVi(self.zone_phase_offset(target)))
            }
            other => other,
        };
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
        let brightness = assignment.brightness.min(100);
        if let Some(kind) = animation {
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

    fn zone_phase_offset(&self, target: &Target) -> f64 {
        // Use the device layout so changing one target doesn't move the others.
        let zones: Vec<_> = self
            .hardware
            .controllers
            .iter()
            .flat_map(|controller| {
                let device = controller.device();
                device
                    .zones
                    .iter()
                    .filter(|zone| zone.direct)
                    .map(|zone| (&device.id, &zone.id))
            })
            .collect();
        zones
            .iter()
            .position(|&(device, zone)| device == &target.device_id && zone == &target.zone_id)
            .map_or(0.0, |index| {
                COLOR_CYCLE_SECONDS * index as f64 / zones.len() as f64
            })
    }

    fn frame(&mut self, now: Instant) -> Vec<String> {
        let phase = now.duration_since(self.epoch).as_secs_f64();
        let mut batches: BTreeMap<String, Vec<(Target, Rgb)>> = BTreeMap::new();
        for (target, animation) in &self.animations {
            let frame = animation.frame(phase);
            if Some(frame) != animation.last {
                batches
                    .entry(target.device_id.clone())
                    .or_default()
                    .push((target.clone(), frame));
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
                                .map(|(target, frame)| (target.zone_id.as_str(), *frame))
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
                Ok(zone_failures) => {
                    for (zone_id, error) in zone_failures {
                        let target = Target { device_id: device.clone(), zone_id };
                        failures.push(format!("Animation stopped ({} / {}) — {error}", device, target.zone_id));
                        self.animations.remove(&target);
                        self.active.remove(&target);
                    }
                    for (target, frame) in batch {
                        if let Some(animation) = self.animations.get_mut(&target) {
                            animation.last = Some(frame);
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
