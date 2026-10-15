//! Rest-pose rig (from master.glb / mannequin.glb) and conversion of model-space joint deltas
//! (hinge convention: rotation about model X, + = flexion) into each bone's LOCAL frame.
//! Uses REST (bind) frames: exact for deltas layered on the rest pose, approximate when layered on
//! a playing clip. Joint index = the node's position in `skin.joints`, else -1 (fixes 79 vs 65).

use crate::math::{Quat, Vec3};

#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    /// Skin joint index (position in `skin.joints`), or -1 for non-joint nodes.
    pub joint: i32,
    /// Parent node index, or -1.
    pub parent: i32,
    pub t: Vec3,
    pub r: Quat,
}

#[derive(Clone, Debug)]
pub struct Rig {
    pub nodes: Vec<Node>,
    /// joint index -> node index (`usize::MAX` = hole).
    pub joints: Vec<usize>,
    world_r: Vec<Quat>,
    world_t: Vec<Vec3>,
}

impl Rig {
    /// Parse a fixture written by tools/anim_rig_dump.ps1:
    /// `node <idx> <joint> <parent> tx ty tz rx ry rz rw <name...>`.
    pub fn parse(text: &str) -> Result<Rig, String> {
        let mut nodes = Vec::new();
        for line in text.lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            if w.first() != Some(&"node") {
                continue;
            }
            if w.len() < 11 {
                return Err(format!("short line: {line}"));
            }
            let f = |i: usize| w[i].parse::<f64>().map_err(|e| format!("{e}: {line}"));
            let int = |i: usize| w[i].parse::<i32>().map_err(|_| line.to_string());
            if int(1)? as usize != nodes.len() {
                return Err(format!("node order {}", w[1]));
            }
            nodes.push(Node {
                joint: int(2)?,
                parent: int(3)?,
                t: Vec3::new(f(4)?, f(5)?, f(6)?),
                r: Quat {
                    w: f(10)?,
                    x: f(7)?,
                    y: f(8)?,
                    z: f(9)?,
                }
                .normalize(),
                name: w[11..].join(" "),
            });
        }
        Ok(Rig::from_nodes(nodes))
    }

    /// Build from live rest data, nodes in glTF order. `joint` must be the node's position in
    /// `skin.joints` (else -1), so the joint count equals the skin's (65 on master).
    pub fn from_nodes(nodes: Vec<Node>) -> Rig {
        let nj = nodes.iter().map(|n| n.joint + 1).max().unwrap_or(0).max(0) as usize;
        let mut joints = vec![usize::MAX; nj];
        for (i, n) in nodes.iter().enumerate() {
            if n.joint >= 0 {
                joints[n.joint as usize] = i;
            }
        }
        let n = nodes.len();
        let mut rig = Rig {
            nodes,
            joints,
            world_r: vec![Quat::IDENTITY; n],
            world_t: vec![Vec3::ZERO; n],
        };
        for i in 0..n {
            let (r, t) = rig.compute_world(i, &|_| None);
            rig.world_r[i] = r;
            rig.world_t[i] = t;
        }
        rig
    }

    /// Joint index of each node from a glTF skin: position in `skin_joints`, else -1.
    pub fn joint_indices(node_count: usize, skin_joints: &[usize]) -> Vec<i32> {
        let mut v = vec![-1; node_count];
        for (j, &n) in skin_joints.iter().enumerate() {
            if n < node_count {
                v[n] = j as i32;
            }
        }
        v
    }

    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }

    /// joint index -> node index (None for holes / out of range).
    pub fn node_of(&self, joint: usize) -> Option<usize> {
        self.joints.get(joint).copied().filter(|&n| n != usize::MAX)
    }

    /// node index -> joint index (`usize::MAX` for non-joint nodes).
    pub fn node_joint_table(&self) -> Vec<usize> {
        let mut t = vec![usize::MAX; self.nodes.len()];
        for (j, &n) in self.joints.iter().enumerate() {
            if n != usize::MAX {
                t[n] = j;
            }
        }
        t
    }

    /// One-time load log (`[anim_rig]`): joint count, the local flex axis of each named bone and an FK
    /// self-check that the converted delta reproduces a 0.5 rad model-space flexion (ok if < 1e-4 rad).
    pub fn report(&self, rig_name: &str, bones: &[&str]) -> String {
        let mut out = format!(
            "[anim_rig] {rig_name}: {} skin joints\n",
            self.joint_count()
        );
        for b in bones {
            let Some(j) = self.joint_by_name(b) else {
                out += &format!("[anim_rig]   {b}: MISSING\n");
                continue;
            };
            let (ax, purity) = dominant(self.local_axis(j, Vec3::new(1.0, 0.0, 0.0)));
            let err = self.fk_check(j, 0.5);
            out += &format!(
                "[anim_rig]   {b}: flex axis {ax} (purity {purity:.3}), FK error {err:.2e} rad {}\n",
                if err < 1e-4 { "ok" } else { "FAIL" }
            );
        }
        out
    }

    /// FK error (rad) of applying a model-X flexion `angle` to `joint` via `to_local_post`.
    pub fn fk_check(&self, joint: usize, angle: f64) -> f64 {
        let Some(n) = self.node_of(joint) else {
            return f64::INFINITY;
        };
        let d = Quat::from_rotvec(Vec3::new(angle, 0.0, 0.0));
        let posed = self.nodes[n].r * self.to_local_post(joint, d);
        let (w, _) = self.compute_world(n, &|k| if k == n { Some(posed) } else { None });
        super::angle_between(d * self.world_rest(joint).0, w)
    }

    /// Joints whose world rest rotation differs from `other`'s same-named joint by more than `deg`.
    pub fn rest_diff(&self, other: &Rig, deg: f64) -> Vec<(String, f64)> {
        let mut v = Vec::new();
        for (j, &n) in self.joints.iter().enumerate() {
            if n == usize::MAX {
                continue;
            }
            let name = &self.nodes[n].name;
            if let Some(k) = other.joint_by_name(name) {
                let ang =
                    super::angle_between(self.world_rest(j).0, other.world_rest(k).0).to_degrees();
                if ang > deg {
                    v.push((name.clone(), ang));
                }
            }
        }
        v
    }

    /// World transform of node `i`, with optional per-node local rotation overrides.
    pub fn compute_world(&self, i: usize, local: &dyn Fn(usize) -> Option<Quat>) -> (Quat, Vec3) {
        let mut chain = vec![i];
        let mut p = self.nodes[i].parent;
        while p >= 0 {
            chain.push(p as usize);
            p = self.nodes[p as usize].parent;
        }
        let (mut r, mut t) = (Quat::IDENTITY, Vec3::ZERO);
        for &k in chain.iter().rev() {
            t = t + r.rotate(self.nodes[k].t);
            r = (r * local(k).unwrap_or(self.nodes[k].r)).normalize();
        }
        (r, t)
    }

    pub fn joint_by_name(&self, name: &str) -> Option<usize> {
        self.joints
            .iter()
            .position(|&n| n != usize::MAX && self.nodes[n].name == name)
    }

    /// World rest (rotation, translation) of a joint. Panics on a hole.
    pub fn world_rest(&self, joint: usize) -> (Quat, Vec3) {
        let n = self.joints[joint];
        (self.world_r[n], self.world_t[n])
    }

    fn parent_world(&self, joint: usize) -> Quat {
        let p = self.nodes[self.joints[joint]].parent;
        if p < 0 {
            Quat::IDENTITY
        } else {
            self.world_r[p as usize]
        }
    }

    /// Model-space delta D -> PRE-multiply onto local: `local' = q * local` (q = Wp⁻¹ D Wp).
    pub fn to_local_pre(&self, joint: usize, d: Quat) -> Quat {
        let wp = self.parent_world(joint);
        (wp.conj() * d * wp).normalize()
    }
    /// Model-space delta D -> POST-multiply: `local' = local * q` (q = W⁻¹ D W). Bevy: `rot = rot * q`.
    pub fn to_local_post(&self, joint: usize, d: Quat) -> Quat {
        let w = self.world_rest(joint).0;
        (w.conj() * d * w).normalize()
    }
    /// Which axis of the bone's own frame a model-space axis maps to.
    pub fn local_axis(&self, joint: usize, model_axis: Vec3) -> Vec3 {
        self.world_rest(joint).0.conj().rotate(model_axis)
    }
}

/// "+x", "-z" ... dominant component of an axis, plus its magnitude (1.0 = pure local axis).
pub fn dominant(a: Vec3) -> (String, f64) {
    let c = [a.x, a.y, a.z];
    let i = (0..3)
        .max_by(|&x, &y| c[x].abs().total_cmp(&c[y].abs()))
        .unwrap_or(0);
    (
        format!(
            "{}{}",
            if c[i] < 0.0 { "-" } else { "+" },
            ["x", "y", "z"][i]
        ),
        c[i].abs(),
    )
}
