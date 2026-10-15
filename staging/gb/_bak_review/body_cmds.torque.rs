/// Motor torque limit (N m) from crates/sim phys_mass: hold the hanging chain level against gravity with 1.5x
/// headroom plus 20 rad/s^2 on its inertia about the joint (segment as a solid cylinder, radius 0.04 m approx).
pub fn torque_limit_for(chain_mass: f32, chain_com_dist: f32, length: f32) -> f32 {
    use guildbound_sim::phys_mass as pm;
    let props = pm::segment_props(pm::Segment { mass: chain_mass, length: length.max(0.05), radius: 0.04 }, &[]);
    pm::torque_limit(chain_mass, chain_com_dist, props.inertia_joint[0][0].max(props.inertia_joint[2][2]), 20.0, 1.5)
}
// In ragdoll_motors, before `let mut joint = rj.clone();`:
//   let mut chain = vec![ci]; let mut k = 0;
//   while k < chain.len() { let c = chain[k]; chain.extend(links.iter().enumerate().filter(|(_, l)| l.2 == Some(c)).map(|(i, _)| i)); k += 1; }
//   let m = |i: usize| world.get::<Mass>(links[i].1).map_or(1.0, |m| m.0);
//   let total: f32 = chain.iter().map(|&i| m(i)).sum();
//   let com = chain.iter().map(|&i| pos(world, links[i].1) * m(i)).sum::<Vec3>() / total.max(1e-3);
//   let limit = torque_limit_for(total, com.distance(cp), child_end.map_or(0.25, |e| e.distance(cp)));
// then .with_max_torque(limit), and RagdollTrack.joints becomes Vec<(String, f32, f32, f32)> carrying limit.
