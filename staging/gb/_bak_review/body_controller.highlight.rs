/// Driver colour per bone while the Pose tab is open, white sphere on the hovered bone, axes + rings on the selected.
#[allow(clippy::too_many_arguments)]
fn highlight(
    store: Res<ControllerStore>,
    studio: Res<crate::studio::Studio>,
    pb: Res<crate::possession::PhysicalBody>,
    names: Query<(&Name, &GlobalTransform)>,
    window: Option<Single<&Window, With<bevy::window::PrimaryWindow>>>,
    cam: Option<Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>>,
    mut gizmos: Gizmos<crate::studio::StudioGizmos>,
) {
    let pose_tab = studio.open && studio.tab == crate::studio::POSE_TAB;
    let ragdoll = store.target == Target::Stage && pb.on;
    let ray = match (window, cam) { (Some(w), Some(c)) => w.cursor_position().and_then(|p| c.0.viewport_to_world(c.1, p).ok()), _ => None };
    let mut hover: Option<(Vec3, f32)> = None;
    for (n, g) in &names {
        if !store.bones.iter().any(|b| b == n.as_str()) { continue; }
        let p = g.translation();
        if pose_tab {
            gizmos.sphere(Isometry3d::from_translation(p), 0.012, crate::ui_theme::driver_gizmo(bone_state(&store, n.as_str(), ragdoll)));
            if let Some(r) = ray {
                let t = (p - r.origin).dot(*r.direction).max(0.0);
                let d = (r.origin + *r.direction * t).distance(p);
                if d < 0.25 && hover.is_none_or(|h| d < h.1) { hover = Some((p, d)); }
            }
        }
        if store.selected.as_deref() == Some(n.as_str()) {
            gizmos.sphere(Isometry3d::from_translation(p), 0.035, Color::srgb(1.0, 0.85, 0.2));
            let (_, r, _) = g.to_scale_rotation_translation();
            for (ax, c) in [(Vec3::X, Color::srgb(1.0, 0.3, 0.3)), (Vec3::Y, Color::srgb(0.3, 1.0, 0.3)), (Vec3::Z, Color::srgb(0.3, 0.5, 1.0))] {
                gizmos.line(p, p + r * ax * 0.12, c);
                gizmos.circle(Isometry3d::new(p, r * Quat::from_rotation_arc(Vec3::Z, ax)), 0.09, c);
            }
        }
    }
    if let Some((p, _)) = hover { gizmos.sphere(Isometry3d::from_translation(p), 0.03, crate::ui_theme::hover_gizmo()); }
}
