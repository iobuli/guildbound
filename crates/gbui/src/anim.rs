//! Tweens in the manner of `lv_anim`: a property runs from one value to another over a duration
//! with an easing path, after a delay, once, n times or forever, optionally playing back.
//! `Tweens` holds any number per node; `run_tweens` writes them (on real time) into `Opacity`,
//! `UiTransform` (offset px, scale, rotation rad clockwise) and `Node` width/height (%). A tween
//! that ends can despawn or hide its node. `Blink` squares `Opacity` on and off.
use crate::style::Opacity;
use bevy::prelude::*;
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    Linear,
    InQuad,
    OutQuad,
    InOutQuad,
    OutCubic,
    InOutCubic,
    InOutSine,
    /// Overshoots the end a little, then settles (`lv_anim_path_overshoot`).
    OutBack,
    /// Bounces on the end (`lv_anim_path_bounce`).
    OutBounce,
    /// Holds the start, then jumps to the end (`lv_anim_path_step`).
    Step,
}

pub fn ease(e: Easing, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match e {
        Easing::Linear => t,
        Easing::InQuad => t * t,
        Easing::OutQuad => 1.0 - (1.0 - t) * (1.0 - t),
        Easing::InOutQuad => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
            }
        }
        Easing::OutCubic => 1.0 - (1.0 - t).powi(3),
        Easing::InOutCubic => {
            if t < 0.5 {
                4.0 * t * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
            }
        }
        Easing::InOutSine => -((PI * t).cos() - 1.0) / 2.0,
        Easing::OutBack => {
            let (c1, c3) = (1.70158, 2.70158);
            1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
        }
        Easing::OutBounce => {
            let (n, d) = (7.5625, 2.75);
            if t < 1.0 / d {
                n * t * t
            } else if t < 2.0 / d {
                let t = t - 1.5 / d;
                n * t * t + 0.75
            } else if t < 2.5 / d {
                let t = t - 2.25 / d;
                n * t * t + 0.9375
            } else {
                let t = t - 2.625 / d;
                n * t * t + 0.984375
            }
        }
        Easing::Step => {
            if t >= 1.0 {
                1.0
            } else {
                0.0
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Prop {
    Opacity,
    /// `UiTransform` translation (px).
    Offset,
    /// `UiTransform` scale.
    Scale,
    /// `UiTransform` rotation (rad, clockwise on screen).
    Rotation,
    /// `Node` width / height (%).
    Width,
    Height,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    Once,
    Times(u32),
    Forever,
}

/// What happens to the node when the tween ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Done {
    Keep,
    Despawn,
    Hide,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tween {
    pub prop: Prop,
    /// Scalars use `x`.
    pub from: Vec2,
    pub to: Vec2,
    pub duration: f32,
    pub delay: f32,
    pub ease: Easing,
    pub repeat: Repeat,
    /// Each cycle plays forward, then back.
    pub yoyo: bool,
    pub done: Done,
    pub elapsed: f32,
}

impl Tween {
    pub fn new(prop: Prop, from: Vec2, to: Vec2, duration: f32) -> Self {
        Self {
            prop,
            from,
            to,
            duration,
            delay: 0.0,
            ease: Easing::OutCubic,
            repeat: Repeat::Once,
            yoyo: false,
            done: Done::Keep,
            elapsed: 0.0,
        }
    }
    pub fn opacity(from: f32, to: f32, duration: f32) -> Self {
        Self::new(Prop::Opacity, Vec2::splat(from), Vec2::splat(to), duration)
    }
    pub fn offset(from: Vec2, to: Vec2, duration: f32) -> Self {
        Self::new(Prop::Offset, from, to, duration)
    }
    pub fn scale(from: f32, to: f32, duration: f32) -> Self {
        Self::new(Prop::Scale, Vec2::splat(from), Vec2::splat(to), duration)
    }
    pub fn rotation(from: f32, to: f32, duration: f32) -> Self {
        Self::new(Prop::Rotation, Vec2::splat(from), Vec2::splat(to), duration)
    }
    pub fn width(from: f32, to: f32, duration: f32) -> Self {
        Self::new(Prop::Width, Vec2::splat(from), Vec2::splat(to), duration)
    }
    pub fn ease(mut self, ease: Easing) -> Self {
        self.ease = ease;
        self
    }
    pub fn delay(mut self, delay: f32) -> Self {
        self.delay = delay;
        self
    }
    pub fn repeat(mut self, repeat: Repeat) -> Self {
        self.repeat = repeat;
        self
    }
    pub fn yoyo(mut self) -> Self {
        self.yoyo = true;
        self
    }
    pub fn then(mut self, done: Done) -> Self {
        self.done = done;
        self
    }

    /// Raw (uneased) position 0..1 in the current pass, and whether the tween has ended.
    pub fn phase(&self) -> (f32, bool) {
        let t = self.elapsed - self.delay;
        if t <= 0.0 {
            return (0.0, false);
        }
        let d = self.duration.max(1e-4);
        let pass = if self.yoyo { 2.0 * d } else { d };
        let cycles = match self.repeat {
            Repeat::Once => 1,
            Repeat::Times(n) => n.max(1),
            Repeat::Forever => u32::MAX,
        };
        let cycle = (t / pass).floor();
        if cycle >= cycles as f32 {
            return (if self.yoyo { 0.0 } else { 1.0 }, true);
        }
        let local = t - cycle * pass;
        let x = if local <= d { local / d } else { 2.0 - local / d };
        (x.clamp(0.0, 1.0), false)
    }

    pub fn value(&self) -> Vec2 {
        let (x, _) = self.phase();
        self.from.lerp(self.to, ease(self.ease, x))
    }
}

#[derive(Component, Clone, Debug, Default)]
#[require(Opacity)]
pub struct Tweens(pub Vec<Tween>);

impl Tweens {
    pub fn one(t: Tween) -> Self {
        Self(vec![t])
    }
    pub fn with(mut self, t: Tween) -> Self {
        self.play(t);
        self
    }
    /// Start `t`, replacing a running tween of the same property.
    pub fn play(&mut self, t: Tween) {
        self.0.retain(|old| old.prop != t.prop);
        self.0.push(t);
    }
}

pub fn run_tweens(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut nodes: Query<(
        Entity,
        &mut Tweens,
        &mut Opacity,
        Option<&mut UiTransform>,
        Option<&mut Node>,
        Option<&mut Visibility>,
    )>,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut tweens, mut opacity, mut transform, mut node, mut visibility) in &mut nodes {
        if tweens.0.is_empty() {
            continue;
        }
        let mut gone = false;
        for tween in tweens.0.iter_mut() {
            tween.elapsed += dt;
            let v = tween.value();
            match tween.prop {
                Prop::Opacity => {
                    opacity.set_if_neq(Opacity(v.x));
                }
                Prop::Offset | Prop::Scale | Prop::Rotation => {
                    if let Some(t) = transform.as_mut() {
                        match tween.prop {
                            Prop::Offset => t.translation = Val2::px(v.x, v.y),
                            Prop::Scale => t.scale = Vec2::splat(v.x),
                            _ => t.rotation = Rot2::radians(v.x),
                        }
                    }
                }
                Prop::Width | Prop::Height => {
                    if let Some(n) = node.as_mut() {
                        if tween.prop == Prop::Width {
                            n.width = percent(v.x);
                        } else {
                            n.height = percent(v.x);
                        }
                    }
                }
            }
            if tween.phase().1 {
                match tween.done {
                    Done::Keep => {}
                    Done::Despawn => gone = true,
                    Done::Hide => {
                        if let Some(vis) = visibility.as_mut() {
                            vis.set_if_neq(Visibility::Hidden);
                        }
                    }
                }
            }
        }
        if gone {
            commands.entity(entity).try_despawn();
            continue;
        }
        tweens.0.retain(|t| !t.phase().1);
    }
}

/// Blinks a node (terminal cursor): visible for the first half of each `period`.
#[derive(Component, Clone, Copy, Debug)]
#[require(Opacity)]
pub struct Blink {
    pub period: f32,
    pub t: f32,
}

impl Blink {
    pub fn new(period: f32) -> Self {
        Self { period, t: 0.0 }
    }
}

pub fn blink(time: Res<Time<Real>>, mut nodes: Query<(&mut Blink, &mut Opacity)>) {
    for (mut b, mut o) in &mut nodes {
        b.t = (b.t + time.delta_secs()) % b.period.max(0.05);
        o.set_if_neq(Opacity(if b.t < b.period * 0.5 { 1.0 } else { 0.0 }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easings_start_at_zero_and_end_at_one() {
        for e in [
            Easing::Linear,
            Easing::InQuad,
            Easing::OutQuad,
            Easing::InOutQuad,
            Easing::OutCubic,
            Easing::InOutCubic,
            Easing::InOutSine,
            Easing::OutBack,
            Easing::OutBounce,
            Easing::Step,
        ] {
            assert!(ease(e, 0.0).abs() < 1e-5, "{e:?} at 0");
            assert!((ease(e, 1.0) - 1.0).abs() < 1e-5, "{e:?} at 1");
        }
        assert!(ease(Easing::OutBack, 0.8) > 1.0, "overshoots");
        assert!(ease(Easing::OutQuad, 0.5) > 0.5 && ease(Easing::InQuad, 0.5) < 0.5);
        assert_eq!(ease(Easing::Step, 0.99), 0.0);
        // monotone where it should be
        let mut last = 0.0;
        for i in 0..=100 {
            let v = ease(Easing::InOutCubic, i as f32 / 100.0);
            assert!(v >= last - 1e-6);
            last = v;
        }
    }

    #[test]
    fn delay_repeat_and_yoyo() {
        let mut t = Tween::opacity(0.0, 1.0, 1.0).ease(Easing::Linear).delay(0.5);
        t.elapsed = 0.25;
        assert_eq!(t.value().x, 0.0, "still in the delay");
        t.elapsed = 1.0;
        assert!((t.value().x - 0.5).abs() < 1e-5);
        t.elapsed = 1.6;
        assert_eq!(t.phase(), (1.0, true));

        let mut y = Tween::opacity(0.0, 1.0, 1.0).ease(Easing::Linear).yoyo().repeat(Repeat::Times(2));
        y.elapsed = 1.5;
        assert!((y.value().x - 0.5).abs() < 1e-5, "on the way back");
        y.elapsed = 2.25;
        assert!((y.value().x - 0.25).abs() < 1e-5, "second cycle");
        y.elapsed = 4.1;
        assert_eq!(y.phase(), (0.0, true), "a yoyo ends where it started");

        let mut f = Tween::opacity(0.0, 1.0, 1.0).repeat(Repeat::Forever);
        f.elapsed = 1e5 + 0.5;
        assert!(!f.phase().1);
    }

    #[test]
    fn playing_a_property_replaces_the_running_one() {
        let mut tw = Tweens::one(Tween::opacity(0.0, 1.0, 1.0)).with(Tween::scale(0.9, 1.0, 0.2));
        tw.play(Tween::opacity(1.0, 0.0, 0.3));
        assert_eq!(tw.0.len(), 2);
        assert_eq!(tw.0.iter().find(|t| t.prop == Prop::Opacity).unwrap().from.x, 1.0);
    }
}
