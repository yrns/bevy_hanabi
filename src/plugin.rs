#[cfg(feature = "2d")]
use bevy::core_pipeline::core_2d::Transparent2d;
#[cfg(feature = "3d")]
use bevy::core_pipeline::core_3d::Transparent3d;
use bevy::{
    prelude::*,
    render::{
        render_graph::RenderGraph,
        render_phase::DrawFunctions,
        render_resource::{SpecializedComputePipelines, SpecializedRenderPipelines},
        renderer::{RenderAdapterInfo, RenderDevice},
        view::visibility::VisibilitySystems,
        RenderApp, RenderSet,
    },
};

use crate::{
    asset::{EffectAsset, EffectAssetLoader},
    compile_effects, gather_removed_effects,
    modifier::*,
    render::{
        extract_effect_events, extract_effects, prepare_effects, queue_effects,
        DispatchIndirectPipeline, DrawEffects, EffectAssetEvents, EffectBindGroups, EffectSystems,
        EffectsMeta, ExtractedEffects, ParticlesInitPipeline, ParticlesRenderPipeline,
        ParticlesUpdatePipeline, ShaderCache, SimParams, VfxSimulateDriverNode, VfxSimulateNode,
    },
    spawn::{self, Random},
    tick_spawners, Gradient, GradientKey, MotionIntegration, ParticleEffect, Property,
    RemovedEffectsEvent, SimulationCondition, SimulationSpace, Spawner, Value,
};

pub mod main_graph {
    pub mod node {
        /// Label for the simulation driver node running the simulation graph.
        pub const HANABI: &str = "hanabi_driver_node";
    }
}

pub mod simulate_graph {
    /// Name of the simulation sub-graph.
    pub const NAME: &str = "hanabi_simulate_graph";

    pub mod node {
        /// Label for the simulation node (init and update compute passes;
        /// view-independent).
        pub const SIMULATE: &str = "hanabi_simulate_node";
    }
}

/// Plugin to add systems related to Hanabi.
#[derive(Debug, Clone, Copy)]
pub struct HanabiPlugin;

impl Plugin for HanabiPlugin {
    fn build(&self, app: &mut App) {
        let render_device = app.world.get_resource::<RenderDevice>().unwrap().clone();

        // Check device limits
        let limits = render_device.limits();
        if limits.max_bind_groups < 4 {
            let adapter_name = app
                .world
                .get_resource::<RenderAdapterInfo>()
                .map(|ai| &ai.name[..])
                .unwrap_or("<unknown>");
            error!("Hanabi requires a GPU device supporting at least 4 bind groups (Limits::max_bind_groups).\n  Current adapter: {}\n  Supported bind groups: {}", adapter_name, limits.max_bind_groups);
            return;
        }

        // Register asset
        app.add_asset::<EffectAsset>()
            .add_event::<RemovedEffectsEvent>()
            .insert_resource(Random(spawn::new_rng()))
            .init_resource::<ShaderCache>()
            .init_asset_loader::<EffectAssetLoader>()
            .configure_sets((
                EffectSystems::TickSpawners
                    .in_base_set(CoreSet::PostUpdate)
                    // This checks the visibility to skip work, so needs to run after
                    // ComputedVisibility was updated.
                    .after(VisibilitySystems::CheckVisibility),
                EffectSystems::CompileEffects
                    .in_base_set(CoreSet::PostUpdate)
                    // This checks the visibility to skip work, so needs to run after
                    // ComputedVisibility was updated.
                    .after(VisibilitySystems::CheckVisibility),
                EffectSystems::GatherRemovedEffects.in_base_set(CoreSet::PostUpdate),
            ))
            .add_system(tick_spawners.in_set(EffectSystems::TickSpawners))
            .add_system(compile_effects.in_set(EffectSystems::CompileEffects))
            .add_system(gather_removed_effects.in_set(EffectSystems::GatherRemovedEffects));

        // For particle effect serialization:
        app.register_type::<ParticleEffect>();
        app.register_type::<Spawner>();
        app.register_type::<Option<Spawner>>();

        // For asset serialization. Until https://github.com/bevyengine/bevy/issues/4154 is
        // resolved, every type has to be registered individually.
        app.register_asset_reflect::<EffectAsset>()
            .register_type::<Value<f32>>()
            .register_type::<Value<Vec2>>()
            .register_type::<Value<Vec3>>()
            .register_type::<Value<Vec4>>()
            .register_type_data::<Value<f32>, ReflectDefault>()
            .register_type_data::<Value<Vec2>, ReflectDefault>()
            .register_type_data::<Value<Vec3>, ReflectDefault>()
            .register_type_data::<Value<Vec4>, ReflectDefault>()
            .register_type::<(f32, f32)>()
            .register_type::<(Vec2, Vec2)>()
            .register_type::<(Vec3, Vec3)>()
            .register_type::<(Vec4, Vec4)>()
            .register_type_data::<(f32, f32), ReflectDefault>()
            .register_type_data::<(Vec2, Vec2), ReflectDefault>()
            .register_type_data::<(Vec3, Vec3), ReflectDefault>()
            .register_type_data::<(Vec4, Vec4), ReflectDefault>()
            .register_type::<crate::DimValue>()
            .register_type::<SimulationSpace>()
            .register_type::<SimulationCondition>()
            //.register_type::<BoxedModifier>() // does not work yet
            .register_type::<Property>()
            .register_type::<MotionIntegration>()
            .register_type::<ShapeDimension>()
            .register_type::<Gradient<Vec4>>()
            .register_type::<GradientKey<Vec4>>()
            .register_type::<Vec<GradientKey<Vec4>>>()
            .register_type::<Gradient<Vec2>>()
            .register_type::<GradientKey<Vec2>>()
            .register_type::<Vec<GradientKey<Vec2>>>()
            .register_type::<InitPositionCircleModifier>()
            .register_type::<InitPositionSphereModifier>()
            .register_type::<InitPositionCone3dModifier>()
            .register_type::<InitVelocityCircleModifier>()
            .register_type::<InitVelocitySphereModifier>()
            .register_type::<InitVelocityTangentModifier>()
            .register_type::<InitSizeModifier>()
            .register_type::<InitAgeModifier>()
            .register_type::<InitLifetimeModifier>()
            .register_type::<AccelModifier>()
            .register_type::<RadialAccelModifier>()
            .register_type::<TangentAccelModifier>()
            .register_type::<ForceFieldModifier>()
            .register_type::<ForceFieldSource>()
            .register_type_data::<ForceFieldSource, ReflectDefault>()
            .register_type::<Vec<ForceFieldSource>>()
            .register_type::<LinearDragModifier>()
            .register_type::<AabbKillModifier>()
            .register_type::<ParticleTextureModifier>()
            .register_type::<SetColorModifier>()
            .register_type::<ColorOverLifetimeModifier>()
            .register_type::<SetSizeModifier>()
            .register_type::<SizeOverLifetimeModifier>()
            .register_type::<BillboardModifier>()
            .register_type::<OrientAlongVelocityModifier>()
            .register_type::<Option<InitPositionCircleModifier>>()
            .register_type::<Option<InitPositionSphereModifier>>()
            .register_type::<Option<InitPositionCone3dModifier>>()
            .register_type::<Option<InitVelocityCircleModifier>>()
            .register_type::<Option<InitVelocitySphereModifier>>()
            .register_type::<Option<InitVelocityTangentModifier>>()
            .register_type::<Option<InitSizeModifier>>()
            .register_type::<Option<InitAgeModifier>>()
            .register_type::<Option<InitLifetimeModifier>>()
            .register_type::<Option<AccelModifier>>()
            .register_type::<Option<RadialAccelModifier>>()
            .register_type::<Option<TangentAccelModifier>>()
            .register_type::<Option<ForceFieldModifier>>()
            .register_type::<Option<LinearDragModifier>>()
            .register_type::<Option<AabbKillModifier>>()
            .register_type::<Option<ParticleTextureModifier>>()
            .register_type::<Option<SetColorModifier>>()
            .register_type::<Option<ColorOverLifetimeModifier>>()
            .register_type::<Option<SetSizeModifier>>()
            .register_type::<Option<SizeOverLifetimeModifier>>()
            .register_type::<Option<BillboardModifier>>()
            .register_type::<Option<OrientAlongVelocityModifier>>();

        let effects_meta = EffectsMeta::new(render_device);

        // Register the custom render pipeline
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .insert_resource(effects_meta)
            .init_resource::<EffectBindGroups>()
            .init_resource::<DispatchIndirectPipeline>()
            .init_resource::<ParticlesInitPipeline>()
            .init_resource::<SpecializedComputePipelines<ParticlesInitPipeline>>()
            .init_resource::<ParticlesUpdatePipeline>()
            .init_resource::<SpecializedComputePipelines<ParticlesUpdatePipeline>>()
            .init_resource::<ParticlesRenderPipeline>()
            .init_resource::<SpecializedRenderPipelines<ParticlesRenderPipeline>>()
            .init_resource::<ExtractedEffects>()
            .init_resource::<EffectAssetEvents>()
            .init_resource::<SimParams>()
            .configure_sets((
                EffectSystems::PrepareEffects.in_set(RenderSet::Prepare),
                EffectSystems::QueueEffects.in_set(RenderSet::Queue),
            ))
            .edit_schedule(ExtractSchedule, |schedule| {
                schedule
                    .add_system(extract_effects)
                    .add_system(extract_effect_events);
            })
            .add_system(prepare_effects.in_set(EffectSystems::PrepareEffects))
            .add_system(queue_effects.in_set(EffectSystems::QueueEffects));

        // Register the draw function for drawing the particles. This will be called
        // during the main 2D/3D pass, at the Transparent2d/3d phase, after the
        // opaque objects have been rendered (or, rather, commands for those
        // have been recorded).
        #[cfg(feature = "2d")]
        {
            let draw_particles = DrawEffects::new(&mut render_app.world);
            render_app
                .world
                .get_resource::<DrawFunctions<Transparent2d>>()
                .unwrap()
                .write()
                .add(draw_particles);
        }
        #[cfg(feature = "3d")]
        {
            let draw_particles = DrawEffects::new(&mut render_app.world);
            render_app
                .world
                .get_resource::<DrawFunctions<Transparent3d>>()
                .unwrap()
                .write()
                .add(draw_particles);
        }

        // Add the simulation sub-graph. This render graph runs once per frame no matter
        // how many cameras/views are active (view-independent).
        let mut simulate_graph = RenderGraph::default();
        let simulate_node = VfxSimulateNode::new(&mut render_app.world);
        simulate_graph.add_node(simulate_graph::node::SIMULATE, simulate_node);
        let mut graph = render_app.world.get_resource_mut::<RenderGraph>().unwrap();
        graph.add_sub_graph(simulate_graph::NAME, simulate_graph);

        // Add the simulation driver node which executes the simulation sub-graph. It
        // runs before the camera driver, since rendering needs to access simulated
        // particles.
        graph.add_node(main_graph::node::HANABI, VfxSimulateDriverNode {});
        graph.add_node_edge(
            main_graph::node::HANABI,
            bevy::render::main_graph::node::CAMERA_DRIVER,
        );
    }
}
