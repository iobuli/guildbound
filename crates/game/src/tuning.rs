use bevy::prelude::*;

#[derive(Clone, Copy)]
pub(crate) enum Knob {
    Walk,
    Run,
    Acceleration,
    Turn,
    Blend,
    Mass,
    Friction,
    Support,
    Crouch,
    Stride,
    Jump,
    Prepare,
    Landing,
    RollThreshold,
    AirTuck,
    RollSpeed,
    RollTime,
    RunKnee,
    RunLean,
    FootLift,
    SpinPower,
    GripForce,
    StepRadius,
    HandFollow,
    CarryMove,
    PunchImpulse,
    PunchTime,
    PlantGrip,
    HandSpace,
    SpinDrag,
    HeadLead,
    TurnBank,
    BrakeBend,
    BrakeLean,
    ReturnRate,
    Firmness,
    AirLoose,
    ChargeTime,
    MinJump,
    GroundAnchor,
    LThumb,
    LIndex,
    LMiddle,
    LRing,
    LPinky,
    RThumb,
    RIndex,
    RMiddle,
    RRing,
    RPinky,
    Reference,
    Cadence,
    FingerGrip,
    ScratchRate,
    ScratchCurl,
    PaperBlend,
    TopologyFalloff,
    ChainCoherence,
    SymmetryPrior,
    PoseLimit,
    TemporalFit,
    AccelerationReg,
    FineDetail,
    SourceRecovery,
    GraphRadius,
    AimLaunch,
    MomentumCarry,
    MouseSensitivity,
    CameraSmooth,
    CameraLead,
    SoleClearance,
    SoleDensity,
    ContactCompliance,
    FootPredict,
    ContactPreserve,
    LibraryBlend,
    ResidualFreedom,
    ActiveLimbFreedom,
    HandTarget,
    DimensionAdapt,
    UpperShift,
    SupportLean,
    AccelLean,
    ArmCounter,
    LeanResponse,
}
pub(crate) const PAGES: [&str; 17] = [
    "MOVE",
    "BODY / FEET",
    "JUMP / LAND",
    "RUN / ROLL",
    "GRAB / SPIN",
    "HANDS / IMPACT",
    "STEERING / BALANCE",
    "BODY / FLIGHT",
    "LEFT FINGERS",
    "RIGHT FINGERS",
    "RPG / GESTURES",
    "PAPER RIG",
    "PAPER MOTION",
    "AIM / CAMERA",
    "DENSE CONTACT",
    "LIBRARY / RESIDUAL",
    "UPPER BODY WEIGHT",
];
const SPECS: [(&str, f32, f32, f32); 85] = [
    ("Walk speed (m/s)", 0.2, 2.5, 1.3),
    ("Run speed (m/s)", 2.0, 6.0, 3.7),
    ("Acceleration", 4.0, 50.0, 30.0),
    ("Turn (deg/s)", 90.0, 900.0, 540.0),
    ("Blend time (s)", 0.1, 0.8, 0.18),
    ("Controller mass (kg)", 40.0, 120.0, 75.0),
    ("Ground friction", 0.15, 1.4, 0.8),
    ("Foot support", 0.0, 1.0, 1.0),
    ("Crouch (m)", 0.0, 0.25, 0.0),
    ("Stride multiplier", 0.65, 2.0, 1.0),
    ("Jump impulse (Ns)", 180.0, 520.0, 345.0),
    ("Jump preparation (s)", 0.1, 0.45, 0.24),
    ("Landing recovery (s)", 0.12, 0.6, 0.32),
    ("Auto roll speed (m/s)", 0.0, 5.0, 2.3),
    ("Air tuck (m)", 0.0, 0.4, 0.10),
    ("Roll speed (m/s)", 1.0, 7.0, 3.6),
    ("Roll duration (s)", 0.45, 1.1, 0.72),
    ("Run knee lift (m)", 0.05, 0.32, 0.145),
    ("Run lean (rad)", 0.0, 0.35, 0.08),
    ("Swing clearance (m)", 0.0, 0.25, 0.045),
    ("Spin drive", 0.3, 2.0, 1.0),
    ("Grip break force (N)", 600.0, 4000.0, 2200.0),
    ("Circle step radius (m)", 0.2, 0.8, 0.38),
    ("Hand follow rate", 3.0, 20.0, 9.0),
    ("Carry movement", 0.1, 1.5, 0.65),
    ("Punch impulse (Ns)", 4.0, 60.0, 22.0),
    ("Punch duration (s)", 0.22, 0.7, 0.4),
    ("Landing grip", 0.2, 1.0, 0.9),
    ("Two hand spacing (m)", 0.03, 0.16, 0.07),
    ("Spin coast damping", 0.1, 3.0, 0.7),
    ("Head turn lead", 0.0, 1.0, 0.35),
    ("Turn body bank", 0.0, 0.8, 0.15),
    ("Braking knee bend (m)", 0.0, 0.25, 0.07),
    ("Braking lean", 0.0, 0.8, 0.25),
    ("Posture recovery rate", 5.0, 30.0, 14.0),
    ("Body firmness", 0.0, 1.0, 0.68),
    ("Air joint freedom", 0.0, 1.0, 0.25),
    ("Max jump charge (s)", 0.25, 1.4, 0.75),
    ("Minimum jump power", 0.25, 0.85, 0.5),
    ("Ground anchor follow", 3.0, 25.0, 12.0),
    ("Left thumb curl", 0.0, 1.0, 0.0),
    ("Left index curl", 0.0, 1.0, 0.0),
    ("Left middle curl", 0.0, 1.0, 0.0),
    ("Left ring curl", 0.0, 1.0, 0.0),
    ("Left pinky curl", 0.0, 1.0, 0.0),
    ("Right thumb curl", 0.0, 1.0, 0.0),
    ("Right index curl", 0.0, 1.0, 0.0),
    ("Right middle curl", 0.0, 1.0, 0.0),
    ("Right ring curl", 0.0, 1.0, 0.0),
    ("Right pinky curl", 0.0, 1.0, 0.0),
    ("RPG reference influence", 0.0, 1.0, 0.48),
    ("Reference cadence", 0.7, 1.3, 1.0),
    ("Automatic grip curl", 0.0, 1.0, 0.85),
    ("Scratch speed", 0.5, 2.0, 1.0),
    ("Scratch finger curl", 0.0, 1.0, 0.65),
    ("Paper system influence", 0.0, 1.0, 0.48),
    ("Topology falloff", 0.2, 2.5, 0.9),
    ("Chain coherence", 0.0, 1.0, 0.35),
    ("Leg symmetry prior", 0.0, 0.6, 0.10),
    ("Pose limit (degrees)", 90.0, 179.0, 170.0),
    ("Temporal fit", 0.0, 1.0, 0.07),
    ("Acceleration regularizer", 0.0, 1.0, 0.04),
    ("Fine detail preservation", 0.0, 1.0, 0.82),
    ("Source pose recovery", 0.0, 1.0, 0.65),
    ("Topology graph radius", 1.0, 8.0, 4.0),
    ("Aimed launch speed (m/s)", 0.0, 8.0, 3.8),
    ("Jump momentum carry", 0.0, 1.5, 0.82),
    ("Mouse sensitivity", 0.001, 0.015, 0.006),
    ("Camera response", 3.0, 30.0, 14.0),
    ("Camera motion lead (m)", 0.0, 2.0, 0.65),
    ("Sole clearance (m)", 0.002, 0.04, 0.009),
    ("Sole sample density", 3.0, 15.0, 11.0),
    ("Contact compliance", 0.0, 1.0, 0.32),
    ("Foothold lookahead (s)", 0.0, 0.6, 0.28),
    ("Contact preservation", 0.0, 1.0, 0.92),
    ("Library reconstruction", 0.0, 1.0, 0.50),
    ("Procedural residual freedom", 0.0, 1.0, 0.92),
    ("Active limb freedom", 0.0, 1.0, 0.94),
    ("Landmark strike bias", 0.0, 1.0, 0.75),
    ("Body dimension adaptation", 0.0, 1.0, 1.0),
    ("Upper-body shift", 0.0, 1.5, 0.82),
    ("Support-foot lean", 0.0, 1.5, 0.95),
    ("Acceleration lean", 0.0, 1.5, 0.78),
    ("Arm counterbalance", 0.0, 1.2, 0.46),
    ("Weight-shift response", 2.0, 24.0, 11.0),
];
#[derive(Resource, Clone)]
pub(crate) struct Tuning {
    values: [f32; 85],
    pub page: usize,
    pub note: String,
}
impl Default for Tuning {
    fn default() -> Self {
        Self {
            values: SPECS.map(|s| s.3),
            page: 0,
            note: "F5 saves feel.json; F9 loads it.".into(),
        }
    }
}
impl Tuning {
    pub fn get(&self, k: Knob) -> f32 {
        self.values[k as usize]
    }
    pub fn value_fraction(&self, k: Knob) -> f32 {
        let spec = SPECS[k as usize];
        (self.values[k as usize] - spec.1) / (spec.2 - spec.1)
    }
    pub fn set_value(&mut self, k: Knob, value: f32) {
        let spec = SPECS[k as usize];
        self.values[k as usize] = value.clamp(spec.1, spec.2);
    }
    pub fn fraction(&self, slot: usize) -> f32 {
        let i = self.page * 5 + slot;
        let s = SPECS[i];
        (self.values[i] - s.1) / (s.2 - s.1)
    }
    pub fn label(&self, slot: usize) -> String {
        let i = self.page * 5 + slot;
        format!("{}  {:.2}", SPECS[i].0, self.values[i])
    }
    pub fn set(&mut self, slot: usize, f: f32) {
        let i = self.page * 5 + slot;
        let s = SPECS[i];
        self.values[i] = s.1 + (s.2 - s.1) * f.clamp(0.0, 1.0);
    }
    pub fn write_recommended(&self) {
        let value: serde_json::Map<String, serde_json::Value> = SPECS
            .iter()
            .enumerate()
            .map(|(i, spec)| (spec.0.into(), serde_json::json!(self.values[i])))
            .collect();
        let _ = std::fs::create_dir_all("profiles").and_then(|_| {
            std::fs::write(
                "profiles/feel_recommended.json",
                serde_json::to_string_pretty(&value).unwrap(),
            )
        });
    }
    pub fn save(&mut self) {
        let value: serde_json::Map<String, serde_json::Value> = SPECS
            .iter()
            .enumerate()
            .map(|(i, s)| (s.0.into(), serde_json::json!(self.values[i])))
            .collect();
        match std::fs::create_dir_all("profiles").and_then(|_| {
            std::fs::write(
                "profiles/feel.json",
                serde_json::to_string_pretty(&value).unwrap(),
            )
        }) {
            Ok(()) => {
                self.note =
                    "Saved profiles/feel.json. Share this file for your preferred feel.".into()
            }
            Err(e) => self.note = format!("Save failed: {e}"),
        }
    }
    pub fn load(&mut self) {
        match std::fs::read_to_string("profiles/feel.json")
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).map_err(|e| e.to_string()))
        {
            Ok(value) => {
                for (i, s) in SPECS.iter().enumerate() {
                    if let Some(v) = value[s.0].as_f64().filter(|v| v.is_finite()) {
                        self.values[i] = (v as f32).clamp(s.1, s.2);
                    }
                }
                self.note = "Loaded profiles/feel.json.".into();
            }
            Err(e) => self.note = format!("Load failed: {e}"),
        }
    }
}
