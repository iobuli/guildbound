//! cargo run -p posenet --release -- POSES.bin OUT.bin [epochs]
//!
//! Trains PoseNet on the library's poses (written by the game: Studio > Possession > "Build pose data") and
//! reports the error on clips it never saw (every tenth clip is held out), per bone, in degrees.
use posenet::math::{angle, from6};
use posenet::pose::{frame_features, frame_outputs};
use posenet::{BONES, Dataset, FEATURES, Mlp, OUTPUTS};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(src), Some(dst)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: posenet POSES.bin OUT.bin [epochs]");
        std::process::exit(2);
    };
    let epochs: usize = args.get(3).and_then(|e| e.parse().ok()).unwrap_or(60);
    let data = Dataset::from_bytes(&std::fs::read(src).expect("read poses")).expect("pose data");
    let held = |clip: u32| clip % 10 == 0;
    let (mut xi, mut yi, mut xv, mut yv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for f in &data.frames {
        let (x, y) = (frame_features(&f.bones), frame_outputs(&f.bones));
        if held(f.clip) {
            xv.push(x);
            yv.push(y)
        } else {
            xi.push(x);
            yi.push(y)
        }
    }
    println!(
        "{} clips, {} training poses, {} held-out poses",
        data.clips.len(),
        xi.len(),
        xv.len()
    );
    let mut net = Mlp::new(&[FEATURES, 256, 256, OUTPUTS], 0x9E37_79B9);
    net.fit_normalisation(&xi, &yi);
    let start = std::time::Instant::now();
    net.train(&xi, &yi, epochs, 64, 2e-3, 0.3, 1e-3, |e, l| {
        if e % 5 == 0 || e + 1 == epochs {
            println!(
                "epoch {e:3}  train loss {l:.4}  ({:.0} s)",
                start.elapsed().as_secs_f32()
            );
        }
    });
    println!("held-out loss {:.4}", net.loss(&xv, &yv));
    let mut err = vec![0.0f32; BONES.len()];
    for (x, y) in xv.iter().zip(&yv) {
        let p = net.run(x);
        for (b, e) in err.iter_mut().enumerate() {
            *e += angle(from6(&p[b * 6..b * 6 + 6]), from6(&y[b * 6..b * 6 + 6])).to_degrees();
        }
    }
    let n = xv.len().max(1) as f32;
    let mean: f32 = err.iter().sum::<f32>() / (n * BONES.len() as f32);
    println!("held-out mean bone error {mean:.1} deg");
    for (b, e) in BONES.iter().zip(&err) {
        println!("  {b:18} {:5.1} deg", e / n);
    }
    let t = std::time::Instant::now();
    for x in xv.iter().take(1000) {
        std::hint::black_box(net.run(x));
    }
    println!(
        "inference {:.3} ms per pose",
        t.elapsed().as_secs_f64() * 1000.0 / xv.len().clamp(1, 1000) as f64
    );
    std::fs::write(dst, net.to_bytes()).expect("write weights");
    println!("saved {dst}");
}
