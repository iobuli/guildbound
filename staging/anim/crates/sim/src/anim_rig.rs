//! Rest-pose rig table (from master.glb / mannequin.glb) and conversion of model-space joint deltas
//! (anim_flare / anim_acro hinge convention: rotation about model X, + = flexion) into each bone's
//! LOCAL frame. Pure, no deps. Uses REST (bind) frames: exact for deltas layered on the rest pose,
//! approximate (axis drifts with the animated parent) when layered on a playing clip.

pub type Q = [f64; 4]; // x y z w
pub type V = [f64; 3];

pub fn qmul(a: Q, b: Q) -> Q {
    [a[3]*b[0]+a[0]*b[3]+a[1]*b[2]-a[2]*b[1], a[3]*b[1]-a[0]*b[2]+a[1]*b[3]+a[2]*b[0],
     a[3]*b[2]+a[0]*b[1]-a[1]*b[0]+a[2]*b[3], a[3]*b[3]-a[0]*b[0]-a[1]*b[1]-a[2]*b[2]]
}
pub fn qconj(q: Q) -> Q { [-q[0], -q[1], -q[2], q[3]] }
pub fn qnorm(q: Q) -> Q { let n = q.iter().map(|x| x * x).sum::<f64>().sqrt(); q.map(|x| x / n) }
pub fn rotate(q: Q, v: V) -> V { let p = qmul(qmul(q, [v[0], v[1], v[2], 0.0]), qconj(q)); [p[0], p[1], p[2]] }
pub fn axis_angle(a: V, ang: f64) -> Q { let (s, c) = (0.5 * ang).sin_cos(); [a[0] * s, a[1] * s, a[2] * s, c] }
/// Signed rotation angle of q about unit axis (swing-twist twist part).
pub fn twist_angle(q: Q, axis: V) -> f64 {
    let d = q[0] * axis[0] + q[1] * axis[1] + q[2] * axis[2];
    2.0 * d.atan2(q[3])
}

#[derive(Clone, Debug)]
pub struct Node { pub name: String, pub joint: i32, pub parent: i32, pub t: V, pub r: Q }

#[derive(Clone, Debug)]
pub struct Rig { pub nodes: Vec<Node>, /// joint index -> node index
    pub joints: Vec<usize>, world_r: Vec<Q>, world_t: Vec<V> }

impl Rig {
    /// Parse a fixture written by tools/anim_rig_dump.ps1.
    pub fn parse(text: &str) -> Result<Rig, String> {
        let mut nodes = Vec::new();
        for line in text.lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            if w.first() != Some(&"node") { continue; }
            if w.len() < 11 { return Err(format!("short line: {line}")); }
            let f = |i: usize| w[i].parse::<f64>().map_err(|e| format!("{e}: {line}"));
            let idx: usize = w[1].parse().map_err(|_| line.to_string())?;
            if idx != nodes.len() { return Err(format!("node order {idx}")); }
            nodes.push(Node { joint: w[2].parse().map_err(|_| line.to_string())?, parent: w[3].parse().map_err(|_| line.to_string())?,
                t: [f(4)?, f(5)?, f(6)?], r: qnorm([f(7)?, f(8)?, f(9)?, f(10)?]), name: w[11..].join(" ") });
        }
        Ok(Rig::from_nodes(nodes))
    }
    /// Build from live rest data (the game passes the loaded glTF nodes in node order:
    /// name, skin joint index or -1, parent node or -1, rest translation, rest rotation xyzw).
    pub fn from_nodes(nodes: Vec<Node>) -> Rig {
        let nj = nodes.iter().map(|n| n.joint + 1).max().unwrap_or(0) as usize;
        let mut joints = vec![usize::MAX; nj];
        for (i, n) in nodes.iter().enumerate() { if n.joint >= 0 { joints[n.joint as usize] = i; } }
        let mut rig = Rig { nodes, joints, world_r: vec![], world_t: vec![] };
        let n = rig.nodes.len();
        rig.world_r = vec![[0.0, 0.0, 0.0, 1.0]; n];
        rig.world_t = vec![[0.0; 3]; n];
        for i in 0..n { let (r, t) = rig.compute_world(i, &|_| None); rig.world_r[i] = r; rig.world_t[i] = t; }
        rig
    }
    pub fn joint_count(&self) -> usize { self.joints.len() }
    /// One-time load log: joint count, and which local axis each named bone flexes about
    /// (model-space lateral X), plus a self-check that FK of the converted delta reproduces
    /// a 0.5 rad model-space flexion. Replaces the fixture tests in the live sandbox.
    pub fn report(&self, rig_name: &str, bones: &[&str]) -> String {
        let mut out = format!("[anim_rig] {rig_name}: {} skin joints\n", self.joint_count());
        for b in bones {
            let Some(j) = self.joint_by_name(b) else { out += &format!("[anim_rig]   {b}: MISSING\n"); continue; };
            let (ax, purity) = dominant(self.local_axis(j, [1.0, 0.0, 0.0]));
            let d = axis_angle([1.0, 0.0, 0.0], 0.5);
            let n = self.joints[j];
            let post = self.to_local_post(j, d);
            let posed = qmul(self.nodes[n].r, post);
            let (w, _) = self.compute_world(n, &|k| if k == n { Some(posed) } else { None });
            let want = qmul(d, self.world_rest(j).0);
            let err = { let r = qmul(qconj(want), w); 2.0 * (r[0]*r[0]+r[1]*r[1]+r[2]*r[2]).sqrt().atan2(r[3].abs()) };
            out += &format!("[anim_rig]   {b}: flex axis {ax} (purity {purity:.3}), FK error {err:.2e} rad {}\n", if err < 1e-4 { "ok" } else { "FAIL" });
        }
        out
    }
    /// Rest-frame differences between two rigs sharing joint names, above `deg` degrees.
    pub fn rest_diff(&self, other: &Rig, deg: f64) -> Vec<(String, f64)> {
        let mut v = Vec::new();
        for (j, &n) in self.joints.iter().enumerate() {
            if n == usize::MAX { continue; }
            let name = &self.nodes[n].name;
            if let Some(k) = other.joint_by_name(name) {
                let (a, b) = (self.world_rest(j).0, other.world_rest(k).0);
                let r = qmul(qconj(a), b);
                let ang = 2.0 * (r[0]*r[0]+r[1]*r[1]+r[2]*r[2]).sqrt().atan2(r[3].abs());
                if ang.to_degrees() > deg { v.push((name.clone(), ang.to_degrees())); }
            }
        }
        v
    }
    /// World rest transform of node i, with optional per-node local rotation overrides.
    pub fn compute_world(&self, i: usize, local: &dyn Fn(usize) -> Option<Q>) -> (Q, V) {
        let mut chain = vec![i];
        let mut p = self.nodes[i].parent;
        while p >= 0 { chain.push(p as usize); p = self.nodes[p as usize].parent; }
        let (mut r, mut t) = ([0.0, 0.0, 0.0, 1.0], [0.0; 3]);
        for &k in chain.iter().rev() {
            let o = rotate(r, self.nodes[k].t);
            t = [t[0] + o[0], t[1] + o[1], t[2] + o[2]];
            r = qnorm(qmul(r, local(k).unwrap_or(self.nodes[k].r)));
        }
        (r, t)
    }
    pub fn joint_by_name(&self, name: &str) -> Option<usize> {
        self.joints.iter().position(|&n| n != usize::MAX && self.nodes[n].name == name)
    }
    pub fn world_rest(&self, joint: usize) -> (Q, V) { let n = self.joints[joint]; (self.world_r[n], self.world_t[n]) }
    fn parent_world(&self, joint: usize) -> Q {
        let p = self.nodes[self.joints[joint]].parent;
        if p < 0 { [0.0, 0.0, 0.0, 1.0] } else { self.world_r[p as usize] }
    }
    /// Model-space delta D -> rotation to PRE-multiply onto the bone's local rotation:
    /// `local' = to_local_pre * local`  (= Wp^-1 D Wp).
    pub fn to_local_pre(&self, joint: usize, d: Q) -> Q { let wp = self.parent_world(joint); qnorm(qmul(qmul(qconj(wp), d), wp)) }
    /// Same delta to POST-multiply: `local' = local * to_local_post`  (= W^-1 D W). Bevy: `rot = rot * q`.
    pub fn to_local_post(&self, joint: usize, d: Q) -> Q { let w = self.world_rest(joint).0; qnorm(qmul(qmul(qconj(w), d), w)) }
    /// Which axis of the bone's own local frame a model-space axis maps to.
    pub fn local_axis(&self, joint: usize, model_axis: V) -> V { rotate(qconj(self.world_rest(joint).0), model_axis) }
}

/// "+x", "-z" ... dominant component of an axis, plus its magnitude (1.0 = pure local axis).
pub fn dominant(a: V) -> (String, f64) {
    let i = (0..3).max_by(|&x, &y| a[x].abs().partial_cmp(&a[y].abs()).unwrap()).unwrap();
    (format!("{}{}", if a[i] < 0.0 { "-" } else { "+" }, ["x", "y", "z"][i]), a[i].abs())
}
