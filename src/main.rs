use bevy::prelude::*;
use bevy::time::FixedTimestep;
use bevy::utils::Duration;
use bevy_prototype_debug_lines::*;
use rand_pcg::Pcg32;
use rand::{Rng, SeedableRng};

const SHIP_CORNERS: [Vec3; 3] = [
    Vec3 { x:  0.0, y:  5.0, z: 0.0},
    Vec3 { x: -5.0, y: -5.0, z: 0.0},
    Vec3 { x:  5.0, y: -5.0, z: 0.0},
];

const TIME_STEP: f32 = 1.0 / 60.0;
const SHIP_ROTATION_SPEED: f32 = 0.05;
const SHIP_MAX_THRUST: f32 = 5.0;
const GRAVITY_SCALE: f32 = 80000.0;
const PLANET_POINT_COUNT: u32 = 20;
const BULLET_VELOCITY: f32 = 600.0;
const SHIP_RADIUS: f32 = 10.0;
const BULLET_RADIUS: f32 = 1.0;
const ASTEROID_POINT_COUNT: u32 = 15;
const ASTEROID_SPAWN_DISTANCE: f32 = 640.0;
const BULLET_POINT_COUNT: u32 = 4;

#[derive(Resource, Default)]
struct AsteroidTimer { duration: Duration }

#[derive(Component, Deref, DerefMut)]
struct Lifetime(Duration);

#[derive(Component, Deref, DerefMut)]
struct Radius(f32);

#[derive(Component)]
struct Mass { scale: f32 }

#[derive(Component)]
struct Planet;

#[derive(Component)]
struct Ship { fire_delay: Duration }

#[derive(Component)]
struct Bullet;

#[derive(Component)]
struct Asteroid { seed: u64 }

#[derive(Component, Deref, DerefMut)]
struct Velocity(Vec2);

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle {
        transform: Transform::from_xyz(0.0, 0.0, 5.0),
        ..Default::default()
    });
    commands.spawn((Ship { fire_delay: Duration::from_millis(0) }, Radius(SHIP_RADIUS), Mass { scale: 1.0 }, Transform::from_xyz(0.0, 300.0, 0.0), Velocity(Vec2::new(0.0,0.0))));
    commands.spawn((Planet, Radius( 30.0 ), Transform::from_xyz(0.0, 0.0, 0.0)));
    commands.spawn((Planet, Radius( 10.0 ), Transform::from_xyz(200.0, -100.0, 0.0)));
    commands.spawn((Planet, Radius( 10.0 ), Transform::from_xyz(-200.0, -100.0, 0.0)));
    commands.insert_resource(AsteroidTimer{ duration: Duration::from_secs(5) });
}

fn lifetime_control(mut commands: Commands, time: Res<Time>, mut query: Query<(Entity, &mut Lifetime)>) {
    for (entity, mut lifetime) in &mut query {
        if !lifetime.is_zero() {
            if lifetime.0 > time.delta() {
                *lifetime = Lifetime(lifetime.0 - time.delta());
            } else {
                *lifetime = Lifetime(Duration::new(0, 0));
            }
        }

        if lifetime.is_zero() {
            commands.entity(entity).despawn();
        }
    }
}

fn ship_control(mut query: Query<(&mut Transform, &mut Velocity), With<Ship>>, keyboard_input: Res<Input<KeyCode>>) {
    for (mut transform, mut velocity) in &mut query {
        let mut direction = 0.0;
        if keyboard_input.pressed(KeyCode::Left) {
           direction = SHIP_ROTATION_SPEED;
        }
        if keyboard_input.pressed(KeyCode::Right) {
           direction = -SHIP_ROTATION_SPEED;  
        }
        transform.rotation = transform.rotation * Quat::from_rotation_z(direction);

        if keyboard_input.pressed(KeyCode::Up) {
            let thrust = transform.rotation * Vec3{ x: 0.0, y: SHIP_MAX_THRUST, z: 0.0};
            velocity.x += thrust.x;
            velocity.y += thrust.y;
        }
    }
}

fn apply_gravity(mut entity_query: Query<(&Mass, &Transform, &mut Velocity)>, planet_query: Query<(&Transform, &Radius), With<Planet>>) {
    for (planet_transform, planet_radius) in &planet_query {
        let planet_radius = **planet_radius;
        for (entity_mass, entity_transform, mut entity_velocity) in &mut entity_query {
            let gravity_vector = (planet_transform.translation - entity_transform.translation).normalize();
            let distance = (planet_transform.translation - entity_transform.translation).length();
            let gravity_force = f32::min(planet_radius, distance) * GRAVITY_SCALE * entity_mass.scale / f32::max(1.0, distance * distance);
            entity_velocity.x += gravity_vector.x * gravity_force * TIME_STEP;
            entity_velocity.y += gravity_vector.y * gravity_force * TIME_STEP;
        }
    }
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * TIME_STEP;
        transform.translation.y += velocity.y * TIME_STEP;
    }
}

fn space_clamp(mut query: Query<&mut Transform, With<Ship>>, windows: Res<Windows>) {
    let window_half_width = windows.get_primary().unwrap().width() / 2.0;
    let window_half_height = windows.get_primary().unwrap().height() / 2.0;
    for mut transform in &mut query {
        if transform.translation.x < -window_half_width {
            transform.translation.x = window_half_width + transform.translation.x % window_half_width;
        }
        if transform.translation.x > window_half_width {
            transform.translation.x = -window_half_width + transform.translation.x % window_half_width;
        }
        if transform.translation.y < -window_half_height {
            transform.translation.y = window_half_height + transform.translation.y % window_half_height;
        }
        if transform.translation.y > window_half_height {
            transform.translation.y = -window_half_height + transform.translation.y % window_half_height;
        }
    }
}

fn fire_control(mut query: Query<(&mut Ship, &Transform)>, mut commands: Commands, keyboard_input: Res<Input<KeyCode>>, time: Res<Time>) {
    for (mut ship, transform) in &mut query {
        if !ship.fire_delay.is_zero() {
            if ship.fire_delay > time.delta() {
                ship.fire_delay = ship.fire_delay - time.delta();
            } else {
                ship.fire_delay = Duration::new(0, 0);
            }
        }
        if ship.fire_delay.is_zero() && keyboard_input.pressed(KeyCode::Space) {
            let bullet_velocity = transform.rotation * Vec3::new(0.0, BULLET_VELOCITY, 0.0);
            let bullet_position = transform.translation + transform.rotation * SHIP_CORNERS[0];
            commands.spawn((Bullet, Lifetime(Duration::from_secs(3)), Radius(BULLET_RADIUS), Mass { scale: 6.0 }, Transform::from_translation(bullet_position), Velocity ( Vec2::new(bullet_velocity.x, bullet_velocity.y) )));
            ship.fire_delay = Duration::from_millis(100);
        }
    }
}

fn planet_colision(mut commands: Commands, planet_query: Query<(&Radius, &Transform), With<Planet>>, entity_query: Query<(Entity, &Radius, &Transform), Without<Planet>>) {
    for (planet_radius, planet_transform) in &planet_query {
        let planet_radius = **planet_radius;
        for (entity, entity_radius, entity_transform) in &entity_query {
            let entity_radius = **entity_radius;
            let distance = Vec3::distance(planet_transform.translation, entity_transform.translation);
            if distance < (planet_radius + entity_radius) {
                commands.entity(entity).despawn();
            }
        }
    }
}

fn asteroid_collision(mut commands: Commands, asteroid_query: Query<(Entity, &Radius, &Transform, &Velocity), With<Asteroid>>, bullet_query: Query<(Entity, &Radius, &Transform), With<Bullet>>) {
    for (asteroid_entity, asteroid_radius, asteroid_transform, asteroid_velocity) in &asteroid_query {
        let asteroid_radius = **asteroid_radius;
        for (bullet_entity, bullet_radius, bullet_transform) in &bullet_query {
            let bullet_radius = **bullet_radius;
            let distance = Vec3::distance(asteroid_transform.translation, bullet_transform.translation);
            if distance < (asteroid_radius + bullet_radius) {
                commands.entity(asteroid_entity).despawn();
                commands.entity(bullet_entity).despawn();

                if asteroid_radius > 4.0 {
                    let mut rng = rand::thread_rng();
                    let new_radius = asteroid_radius / 3.0;
                    let max_angle = 2.0 * std::f32::consts::PI;
                    let angle_section = max_angle / 3.0;
                    let mut spawn_angle: f32 = rng.gen_range(0.0..max_angle);
                    for _i in 0..3 {
                        let spawn_x = asteroid_transform.translation.x + new_radius * spawn_angle.cos();
                        let spawn_y = asteroid_transform.translation.y + new_radius * spawn_angle.sin();
                        let shape_seed = rng.gen::<u64>();
                        let asteroid_velocity_x = asteroid_velocity.x + spawn_angle.cos() * rng.gen_range(10.0..30.0);
                        let asteroid_velocity_y = asteroid_velocity.y + spawn_angle.sin() * rng.gen_range(10.0..30.0);
                        commands.spawn((Asteroid { seed: shape_seed }, Radius( new_radius ), Mass { scale: 1.0 }, Transform::from_xyz(spawn_x, spawn_y, 0.0), Velocity(Vec2::new(asteroid_velocity_x, asteroid_velocity_y))));
                        spawn_angle += angle_section;
                    }
                }
            }
        }
    }
}

fn asteroid_spawner(mut commands: Commands, mut asteroid_timer: ResMut<AsteroidTimer>, time: Res<Time>) {
    if !asteroid_timer.duration.is_zero() {
        if asteroid_timer.duration > time.delta() {
            asteroid_timer.duration = asteroid_timer.duration - time.delta();
        } else {
            asteroid_timer.duration = Duration::new(0, 0);
        }
    }

    if asteroid_timer.duration.is_zero() {
        let mut rng = rand::thread_rng();
        let spawn_angle: f32 = rng.gen_range(0.0..6.28);
        let spawn_x = ASTEROID_SPAWN_DISTANCE * spawn_angle.cos();
        let spawn_y = ASTEROID_SPAWN_DISTANCE * spawn_angle.sin();
        let asteroid_radius = rng.gen_range(7.0..24.0);
        let shape_seed = rng.gen::<u64>();
        let asteroid_velocity_x = rng.gen_range(10.0..50.0);
        let asteroid_velocity_y = rng.gen_range(10.0..50.0);
        commands.spawn((Asteroid { seed: shape_seed }, Radius( asteroid_radius ), Mass { scale: 1.0 }, Transform::from_xyz(spawn_x, spawn_y, 0.0), Velocity(Vec2::new(asteroid_velocity_x, asteroid_velocity_y))));
        asteroid_timer.duration = Duration::from_millis(rng.gen_range(3000..5000));
    }
}

fn ship_render(query: Query<&Transform, With<Ship>>, mut lines: ResMut<DebugLines>) {
    for transform in &query {
        let points: Vec<Vec3> = SHIP_CORNERS.iter().map(|point| transform.transform_point(*point)).collect();
        for i in 0..points.len() {
            let point1 = points[i];
            let point2 = points[(i + 1) % points.len()];
            lines.line_colored(point1, point2, 0.0, Color::GREEN);
        }
    }
}

fn draw_circle(lines: &mut ResMut<DebugLines>, position: Vec3, radius: f32, color: Color, segments: u32) {
    let mut prev_point = position + Vec3::new(radius, 0.0, 0.0);
    for i in 1..segments {
        let angle = 2.0 * std::f32::consts::PI * (i as f32) / (segments as f32);
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        let this_point = position + Vec3::new(x, y, 0.0);
        lines.line_colored(prev_point, this_point, 0.0, color);
        prev_point = this_point;
    }
    lines.line_colored(prev_point, position + Vec3::new(radius, 0.0, 0.0), 0.0, color);
}

fn draw_irregular_circle(lines: &mut ResMut<DebugLines>, seed: u64, position: Vec3, radius_min: f32, radius_max: f32, color: Color, segments: u32) {
    let mut rng = Pcg32::seed_from_u64(seed);
    let first_point = position + Vec3::new(rng.gen_range(radius_min..radius_max), 0.0, 0.0);
    let mut prev_point = first_point;
    for i in 1..segments {
        let point_radius = rng.gen_range(radius_min..radius_max);
        let angle = 2.0 * std::f32::consts::PI * (i as f32) / (segments as f32);
        let x = point_radius * angle.cos();
        let y = point_radius * angle.sin();
        let this_point = position + Vec3::new(x, y, 0.0);
        lines.line_colored(prev_point, this_point, 0.0, color);
        prev_point = this_point;
    }
    lines.line_colored(prev_point, first_point, 0.0, Color::GREEN);
}

fn planet_render(query: Query<(&Radius, &Transform), With<Planet>>, mut lines: ResMut<DebugLines>) {
    for (planet_radius, transform) in &query {
        let position = transform.translation;
        let radius = **planet_radius;
        draw_circle(&mut lines, position, radius, Color::GREEN, PLANET_POINT_COUNT);
    }
}

fn bullet_render(query: Query<(&Radius, &Transform), With<Bullet>>, mut lines: ResMut<DebugLines>) {
    for (radius, transform) in &query {
        let position = transform.translation;
        let radius = **radius;
        draw_circle(&mut lines, position, radius, Color::GREEN, BULLET_POINT_COUNT);
    }
}

fn asteroid_render(query: Query<(&Radius, &Transform, &Asteroid)>, mut lines: ResMut<DebugLines>) {
    for (radius, transform, asteroid) in &query {
        let position = transform.translation;
        let radius = **radius;
        draw_irregular_circle(&mut lines, asteroid.seed, position, radius - 2.0, radius + 2.0, Color::GREEN, ASTEROID_POINT_COUNT);
    }
}

fn main() {
    App::new()
    .add_plugins(DefaultPlugins)
    .add_plugin(DebugLinesPlugin::default())
    .add_startup_system(setup)
    .add_system_set(
        SystemSet::new()
            .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
            .with_system(ship_control)
            .with_system(apply_gravity)
            .with_system(apply_velocity)
            .with_system(fire_control)
            .with_system(lifetime_control)
            .with_system(space_clamp)
            .with_system(asteroid_spawner)
            
    )
    .add_system(planet_colision)
    .add_system(asteroid_collision)
    .add_system(ship_render)
    .add_system(planet_render)
    .add_system(bullet_render)
    .add_system(asteroid_render)
    .run();
}
