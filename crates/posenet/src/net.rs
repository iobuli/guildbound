//! A multilayer perceptron: dense layers with ELU between them, inputs and outputs normalised by the training
//! set's mean and spread, trained by Adam on mean squared error. Saved as little-endian f32s.

#[derive(Clone)]
pub struct Layer {
    pub inputs: usize,
    pub outputs: usize,
    /// Row-major [outputs][inputs].
    pub w: Vec<f32>,
    pub b: Vec<f32>,
}

#[derive(Clone)]
pub struct Mlp {
    pub layers: Vec<Layer>,
    pub in_mean: Vec<f32>,
    pub in_std: Vec<f32>,
    pub out_mean: Vec<f32>,
    pub out_std: Vec<f32>,
}

fn elu(x: f32) -> f32 {
    if x > 0.0 { x } else { x.exp() - 1.0 }
}

/// Deterministic pseudo-random numbers (xorshift) for initial weights and shuffling.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Standard normal (Box-Muller).
    pub fn normal(&mut self) -> f32 {
        let (a, b) = (self.unit().max(1e-7), self.unit());
        (-2.0 * a.ln()).sqrt() * (std::f32::consts::TAU * b).cos()
    }
}

impl Mlp {
    pub fn new(sizes: &[usize], seed: u64) -> Self {
        let mut rng = Rng(seed | 1);
        let layers = sizes.windows(2).map(|s| {
            let bound = (6.0 / (s[0] + s[1]) as f32).sqrt();
            Layer { inputs: s[0], outputs: s[1], w: (0..s[0] * s[1]).map(|_| (rng.unit() * 2.0 - 1.0) * bound).collect(), b: vec![0.0; s[1]] }
        }).collect();
        let (i, o) = (sizes[0], *sizes.last().unwrap());
        Self { layers, in_mean: vec![0.0; i], in_std: vec![1.0; i], out_mean: vec![0.0; o], out_std: vec![1.0; o] }
    }

    /// Normalised forward pass, keeping each layer's activations (for training).
    fn forward_keep(&self, x: &[f32], acts: &mut Vec<Vec<f32>>) {
        acts.clear();
        acts.push(x.to_vec());
        for (k, l) in self.layers.iter().enumerate() {
            let input = acts.last().unwrap();
            let last = k + 1 == self.layers.len();
            let out: Vec<f32> = (0..l.outputs).map(|o| {
                let row = &l.w[o * l.inputs..(o + 1) * l.inputs];
                let z = l.b[o] + row.iter().zip(input).map(|(w, x)| w * x).sum::<f32>();
                if last { z } else { elu(z) }
            }).collect();
            acts.push(out);
        }
    }

    /// Raw inputs -> raw outputs.
    pub fn run(&self, input: &[f32]) -> Vec<f32> {
        let x: Vec<f32> = input.iter().enumerate().map(|(i, v)| (v - self.in_mean[i]) / self.in_std[i]).collect();
        let mut acts = Vec::new();
        self.forward_keep(&x, &mut acts);
        acts.last().unwrap().iter().enumerate().map(|(i, v)| v * self.out_std[i] + self.out_mean[i]).collect()
    }

    pub fn fit_normalisation(&mut self, inputs: &[Vec<f32>], outputs: &[Vec<f32>]) {
        let stats = |rows: &[Vec<f32>]| {
            let n = rows.len().max(1) as f32;
            let d = rows[0].len();
            let mean: Vec<f32> = (0..d).map(|j| rows.iter().map(|r| r[j]).sum::<f32>() / n).collect();
            let std: Vec<f32> = (0..d).map(|j| (rows.iter().map(|r| (r[j] - mean[j]).powi(2)).sum::<f32>() / n).sqrt().max(1e-3)).collect();
            (mean, std)
        };
        (self.in_mean, self.in_std) = stats(inputs);
        (self.out_mean, self.out_std) = stats(outputs);
    }

    /// Mean squared error (normalised outputs) of the rows.
    pub fn loss(&self, inputs: &[Vec<f32>], outputs: &[Vec<f32>]) -> f32 {
        let mut total = 0.0;
        for (x, y) in inputs.iter().zip(outputs) {
            let p = self.run(x);
            total += p.iter().zip(y).enumerate().map(|(i, (p, y))| ((p - y) / self.out_std[i]).powi(2)).sum::<f32>() / y.len() as f32;
        }
        total / inputs.len().max(1) as f32
    }

    /// AdamW training over shuffled mini-batches. From Holden's Learned Motion Matching (MIT): each sample's
    /// inputs get Gaussian noise at a random strength up to `noise` (in input standard deviations), so the
    /// network learns to answer well for inputs between and around the training poses, and decoupled weight
    /// decay keeps the weights small. `report` gets (epoch, training loss).
    #[allow(clippy::too_many_arguments)]
    pub fn train(&mut self, inputs: &[Vec<f32>], outputs: &[Vec<f32>], epochs: usize, batch: usize, lr: f32, noise: f32, decay: f32, mut report: impl FnMut(usize, f32)) {
        let norm_in: Vec<Vec<f32>> = inputs.iter().map(|r| r.iter().enumerate().map(|(i, v)| (v - self.in_mean[i]) / self.in_std[i]).collect()).collect();
        let norm_out: Vec<Vec<f32>> = outputs.iter().map(|r| r.iter().enumerate().map(|(i, v)| (v - self.out_mean[i]) / self.out_std[i]).collect()).collect();
        let (b1, b2, eps) = (0.9f32, 0.999f32, 1e-8f32);
        let mut m: Vec<(Vec<f32>, Vec<f32>)> = self.layers.iter().map(|l| (vec![0.0; l.w.len()], vec![0.0; l.b.len()])).collect();
        let mut v = m.clone();
        let mut order: Vec<usize> = (0..inputs.len()).collect();
        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        let mut step = 0i32;
        let mut acts = Vec::new();
        for epoch in 0..epochs {
            for i in (1..order.len()).rev() {
                order.swap(i, (rng.next() % (i as u64 + 1)) as usize);
            }
            // cosine decay of the learning rate over the run
            let rate = lr * 0.5 * (1.0 + (std::f32::consts::PI * epoch as f32 / epochs as f32).cos());
            let mut epoch_loss = 0.0;
            for chunk in order.chunks(batch) {
                let mut grads: Vec<(Vec<f32>, Vec<f32>)> = self.layers.iter().map(|l| (vec![0.0; l.w.len()], vec![0.0; l.b.len()])).collect();
                for &s in chunk {
                    let sigma = noise * rng.unit();
                    let x: Vec<f32> = norm_in[s].iter().map(|v| v + sigma * rng.normal()).collect();
                    self.forward_keep(&x, &mut acts);
                    let out = acts.last().unwrap();
                    let n = out.len() as f32;
                    let mut delta: Vec<f32> = out.iter().zip(&norm_out[s]).map(|(p, y)| 2.0 * (p - y) / n).collect();
                    epoch_loss += out.iter().zip(&norm_out[s]).map(|(p, y)| (p - y).powi(2)).sum::<f32>() / n;
                    for k in (0..self.layers.len()).rev() {
                        let l = &self.layers[k];
                        let input = &acts[k];
                        let g = &mut grads[k];
                        for o in 0..l.outputs {
                            let d = delta[o];
                            g.1[o] += d;
                            let row = &mut g.0[o * l.inputs..(o + 1) * l.inputs];
                            for (gw, x) in row.iter_mut().zip(input) {
                                *gw += d * x;
                            }
                        }
                        if k > 0 {
                            // back through the weights, then ELU's slope at the previous layer's output
                            let mut prev = vec![0.0f32; l.inputs];
                            for o in 0..l.outputs {
                                let d = delta[o];
                                for (p, w) in prev.iter_mut().zip(&l.w[o * l.inputs..(o + 1) * l.inputs]) {
                                    *p += d * w;
                                }
                            }
                            for (p, a) in prev.iter_mut().zip(input) {
                                if *a <= 0.0 {
                                    *p *= a + 1.0;
                                }
                            }
                            delta = prev;
                        }
                    }
                }
                step += 1;
                let scale = 1.0 / chunk.len() as f32;
                let (c1, c2) = (1.0 - b1.powi(step), 1.0 - b2.powi(step));
                let adam = |params: &mut [f32], g: &[f32], mm: &mut [f32], vv: &mut [f32]| {
                    for i in 0..params.len() {
                        let gi = g[i] * scale;
                        mm[i] = b1 * mm[i] + (1.0 - b1) * gi;
                        vv[i] = b2 * vv[i] + (1.0 - b2) * gi * gi;
                        params[i] -= rate * ((mm[i] / c1) / ((vv[i] / c2).sqrt() + eps) + decay * params[i]);
                    }
                };
                for (k, l) in self.layers.iter_mut().enumerate() {
                    let ((mw, mb), (vw, vb)) = (&mut m[k], &mut v[k]);
                    adam(&mut l.w, &grads[k].0, mw, vw);
                    adam(&mut l.b, &grads[k].1, mb, vb);
                }
            }
            report(epoch, epoch_loss / inputs.len().max(1) as f32);
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut put = |x: u32| out.extend_from_slice(&x.to_le_bytes());
        put(self.layers.len() as u32);
        for l in &self.layers {
            put(l.inputs as u32);
            put(l.outputs as u32);
        }
        let mut floats: Vec<f32> = Vec::new();
        for l in &self.layers {
            floats.extend(&l.w);
            floats.extend(&l.b);
        }
        for v in [&self.in_mean, &self.in_std, &self.out_mean, &self.out_std] {
            floats.extend(v.iter());
        }
        for f in floats {
            out.extend_from_slice(&f.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        let mut at = 0;
        let mut u32_at = || {
            let v = u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?);
            at += 4;
            Some(v as usize)
        };
        let n = u32_at()?;
        let shape: Vec<(usize, usize)> = (0..n).map(|_| Some((u32_at()?, u32_at()?))).collect::<Option<_>>()?;
        let mut f = b.get(at..)?.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]));
        let mut take = |k: usize| -> Option<Vec<f32>> { let v: Vec<f32> = f.by_ref().take(k).collect(); (v.len() == k).then_some(v) };
        let mut layers = Vec::new();
        for &(i, o) in &shape {
            layers.push(Layer { inputs: i, outputs: o, w: take(i * o)?, b: take(o)? });
        }
        let (i, o) = (shape.first()?.0, shape.last()?.1);
        Some(Self { layers, in_mean: take(i)?, in_std: take(i)?, out_mean: take(o)?, out_std: take(o)? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learns_a_smooth_function_and_survives_a_save() {
        let inputs: Vec<Vec<f32>> = (0..400).map(|i| { let x = i as f32 / 200.0 - 1.0; vec![x, x * x] }).collect();
        let outputs: Vec<Vec<f32>> = inputs.iter().map(|x| vec![(3.0 * x[0]).sin(), x[1] - 0.5 * x[0]]).collect();
        let mut net = Mlp::new(&[2, 32, 32, 2], 7);
        net.fit_normalisation(&inputs, &outputs);
        let before = net.loss(&inputs, &outputs);
        net.train(&inputs, &outputs, 150, 16, 3e-3, 0.0, 0.0, |_, _| {});
        let after = net.loss(&inputs, &outputs);
        assert!(after < before * 0.05 && after < 0.02, "loss {before} -> {after}");
        let back = Mlp::from_bytes(&net.to_bytes()).unwrap();
        assert_eq!(back.run(&[0.3, 0.09]), net.run(&[0.3, 0.09]));
    }
}
