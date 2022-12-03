use bevy::{
    core_pipeline::clear_color::ClearColorConfig,
    log::LogPlugin,
    math::EulerRot,
    prelude::*,
    render::{
        camera::Viewport,
        mesh::shape::{Cube, Plane},
        view::RenderLayers,
    },
    window::{WindowId, WindowResized},
};
use bevy_inspector_egui::WorldInspectorPlugin;

use bevy_hanabi::prelude::*;

fn main() {
    App::default()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            level: bevy::log::Level::WARN,
            filter: "bevy_hanabi=warn,multicam=trace".to_string(),
        }))
        .add_system(bevy::window::close_on_esc)
        .add_plugin(HanabiPlugin)
        .add_plugin(WorldInspectorPlugin::new())
        .add_startup_system(setup)
        .add_system(update_camera_viewports)
        .run();
}

#[derive(Component)]
struct SplitCamera {
    /// Grid position of the camera.
    pos: UVec2,
}

fn make_effect(color: Color) -> EffectAsset {
    let mut size_gradient = Gradient::new();
    size_gradient.add_key(0.0, Vec2::splat(1.0));
    size_gradient.add_key(0.5, Vec2::splat(5.0));
    size_gradient.add_key(0.8, Vec2::splat(0.8));
    size_gradient.add_key(1.0, Vec2::splat(0.0));

    let mut color_gradient = Gradient::new();
    color_gradient.add_key(0.0, Vec4::splat(1.0));
    color_gradient.add_key(0.4, Vec4::new(color.r(), color.g(), color.b(), 1.0));
    color_gradient.add_key(1.0, Vec4::splat(0.0));

    EffectAsset {
        name: "effect1".to_string(),
        capacity: 32768,
        spawner: Spawner::rate(5.0.into()),
        ..Default::default()
    }
    .init(PositionSphereModifier {
        center: Vec3::ZERO,
        radius: 2.,
        dimension: ShapeDimension::Surface,
        speed: 6.0.into(),
    })
    .update(AccelModifier {
        accel: Vec3::new(0., -3., 0.),
    })
    .render(ColorOverLifetimeModifier {
        gradient: color_gradient,
    })
    .render(SizeOverLifetimeModifier {
        gradient: size_gradient.clone(),
    })
    .render(BillboardModifier)
}

fn setup(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Render layers for the 4 cameras, using a mix and match to see the differences
    let layers = [
        RenderLayers::layer(0),
        RenderLayers::layer(0).with(2),
        RenderLayers::layer(1).with(2),
        RenderLayers::all(),
    ];

    // Spawn 4 cameras in grid, "4-player couch co-op"-style
    for i in 0..=3_usize {
        let x = (i % 2) as f32 * 100. - 50.;
        let z = (i / 2) as f32 * 100. - 50.;
        commands.spawn((
            Camera3dBundle {
                camera: Camera {
                    // Have a different priority for each camera to ensure determinism
                    priority: i as isize,
                    ..default()
                },
                camera_3d: Camera3d {
                    // Only clear render target from first camera, others additively render on same
                    // target
                    clear_color: if i == 0 {
                        ClearColorConfig::Default
                    } else {
                        ClearColorConfig::None
                    },
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(x, 100.0, z))
                    .looking_at(Vec3::ZERO, Vec3::Y),
                ..default()
            },
            SplitCamera {
                pos: UVec2::new(i as u32 % 2, i as u32 / 2),
            },
            layers[i],
        ));
    }

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::WHITE,
            // Crank the illuminance way (too) high to make the reference cube clearly visible
            illuminance: 100000.,
            shadows_enabled: false,
            ..Default::default()
        },
        transform: Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 1.7, 2.4, 0.)),
        ..Default::default()
    });

    let cube = meshes.add(Mesh::from(Cube { size: 1.0 }));
    let plane = meshes.add(Mesh::from(Plane { size: 200.0 }));
    let mat = materials.add(Color::PURPLE.into());
    let ground_mat = materials.add(Color::OLIVE.into());

    let effect1 = effects.add(make_effect(Color::RED));

    // Ground plane to make it easier to see the different cameras
    commands.spawn((
        PbrBundle {
            transform: Transform::from_translation(Vec3::Y * -20.)
                * Transform::from_scale(Vec3::new(0.4, 1., 1.)),
            mesh: plane,
            material: ground_mat,
            ..Default::default()
        },
        Name::new("ground"),
        RenderLayers::all(),
    ));

    commands
        .spawn((
            Name::new("0"),
            ParticleEffectBundle {
                effect: ParticleEffect::new(effect1),
                transform: Transform::from_translation(Vec3::new(-30., 0., 0.)),
                ..Default::default()
            },
            RenderLayers::layer(0),
        ))
        .with_children(|p| {
            // Reference cube to visualize the emit origin
            p.spawn((
                PbrBundle {
                    mesh: cube.clone(),
                    material: mat.clone(),
                    ..Default::default()
                },
                Name::new("source"),
                RenderLayers::layer(0),
            ));
        });

    let effect2 = effects.add(make_effect(Color::GREEN));

    commands
        .spawn((
            Name::new("1"),
            ParticleEffectBundle {
                effect: ParticleEffect::new(effect2),
                transform: Transform::from_translation(Vec3::new(0., 0., 0.)),
                ..Default::default()
            },
            RenderLayers::layer(1),
        ))
        .with_children(|p| {
            // Reference cube to visualize the emit origin
            p.spawn((
                PbrBundle {
                    mesh: cube.clone(),
                    material: mat.clone(),
                    ..Default::default()
                },
                Name::new("source"),
                RenderLayers::layer(1),
            ));
        });

    let effect3 = effects.add(make_effect(Color::BLUE));

    commands
        .spawn((
            Name::new("2"),
            ParticleEffectBundle {
                effect: ParticleEffect::new(effect3),
                transform: Transform::from_translation(Vec3::new(30., 0., 0.)),
                ..Default::default()
            },
            RenderLayers::layer(2),
        ))
        .with_children(|p| {
            // Reference cube to visualize the emit origin
            p.spawn((
                PbrBundle {
                    mesh: cube.clone(),
                    material: mat.clone(),
                    ..Default::default()
                },
                Name::new("source"),
                RenderLayers::layer(2),
            ));
        });
}

fn update_camera_viewports(
    windows: Res<Windows>,
    mut resize_events: EventReader<WindowResized>,
    mut query: Query<(&mut Camera, &SplitCamera)>,
) {
    // We need to dynamically resize the camera's viewports whenever the window size
    // changes so then each camera always takes up half the screen.
    // A resize_event is sent when the window is first created, allowing us to reuse
    // this system for initial setup.
    for resize_event in resize_events.iter() {
        if resize_event.id == WindowId::primary() {
            let window = windows.primary();
            let dw = window.physical_width() / 2;
            let dh = window.physical_height() / 2;
            let physical_size = UVec2::new(dw, dh);

            for (mut camera, split_camera) in query.iter_mut() {
                camera.viewport = Some(Viewport {
                    physical_position: UVec2::new(
                        dw * split_camera.pos.x,
                        dh * (1 - split_camera.pos.y),
                    ),
                    physical_size,
                    ..default()
                });
            }
        }
    }
}