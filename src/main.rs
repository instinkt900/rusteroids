use bevy::prelude::*;
use bevy::ecs::schedule::*;
use bevy::time::FixedTimestep;
use bevy::utils::Duration;
use bevy_prototype_debug_lines::*;
use rand_pcg::Pcg32;
use rand::{Rng, SeedableRng};

//const TIME_STEP: f32 = 1.0 / 60.0;

// drawing constants
const SHIP_CORNERS: [Vec3; 3] = [
    Vec3 { x:  0.0, y:  5.0, z: 0.0},
    Vec3 { x: -5.0, y: -5.0, z: 0.0},
    Vec3 { x:  5.0, y: -5.0, z: 0.0},
];
const PLANET_POINT_COUNT: u32 = 80;
const ASTEROID_POINT_COUNT: u32 = 15;
const BULLET_POINT_COUNT: u32 = 4;
const ASTEROID_RADIUS_VARIANCE: f32 = 2.0;

const GRAVITY: f32 = 250.0;

const SHIP_ROTATION_SPEED: f32 = 0.07;
const SHIP_MAX_THRUST: f32 = 1.25;
const SHIP_RADIUS: f32 = 10.0;
const SHIP_MASS: f32 = 10.0;
const SHIP_FIRE_DELAY: u64 = 100;

const BULLET_VELOCITY: f32 = 300.0;
const BULLET_RADIUS: f32 = 1.0;
const BULLET_MASS: f32 = 10.0;
const BULLET_LIFETIME_MS: u64 = 3000;

const PLANET_START_RADIUS: f32 = 30.0;
const PLANET_START_MASS: f32 = 500.0;
const PLANET_RADIUS_CONSUME_SCALE: f32 = 0.3;
const PLANET_MASS_CONSUME_SCALE: f32 = 5.0;

const ASTEROID_SPAWN_DISTANCE: f32 = 640.0;
const ASTEROID_LIFETIME_MS: u64 = 60000;
const ASTEROID_SPAWN_DELAY_MIN_MS: u64 = 2000;
const ASTEROID_SPAWN_DELAY_MAX_MS: u64 = 4000;
const ASTEROID_RADIUS_MIN: f32 = 10.0;
const ASTEROID_RADIUS_MAX: f32 = 20.0;
const ASTEROID_MASS_MIN: f32 = 10.0;
const ASTEROID_MASS_MAX: f32 = 20.0;
const ASTEROID_VELOCITY_MIN: f32 = 20.0;
const ASTEROID_VELOCITY_MAX: f32 = 60.0;
const ASTEROID_DRAG_CONSTANT: f32 = 300.0;
const ASTEROID_DRAG_RADIUS_CONTRIBUTION: f32 = 5.0;
const ASTEROID_FRACTURE_COUNT: u32 = 3;             // broken asteroids break into N parts
const ASTEROID_FRACTURE_RADIUS_FACTOR: f32 = 0.3;   // each broken part has F radius of its parent
const ASTEROID_FRACTURE_MASS_FACTOR: f32 = 0.3;     // each broken part has F mass of its parent
const ASTEROID_FRACTURE_MIN_RADIUS: f32 = 4.0;      // any asteroid smaller than this does not fracture
const ASTEROID_FRACTURE_VEL_MIN: f32 = 10.0;        // min velocity to randomly apply to each fractured part
const ASTEROID_FRACTURE_VEL_MAX: f32 = 30.0;        // max velocity to randomly apply to each fractured part

const TRAIL_MAX_LIFE_MS: u64 = 3000;
const TRAIL_START_ALPHA: f32 = 0.2;

const EXPLOSION_MAX_LIFE_MS: u64 = 500;
const EXPLOSION_MAX_RADIUS: f32 = 40.0;

const GRAVITY_VIS_RATE: f32 = 0.5;
const GRAVITY_VIS_MASS_FACTOR: f32 = 1.0015;
const GRAVITY_VIS_SIZE: f32 = 30.0;

#[derive(Clone, Eq, PartialEq, Debug, Hash)]
enum GameState {
    Title,
    Playing,
    GameOver,
}

#[derive(Resource, Default)]
struct AsteroidTimer { duration: Duration }

#[derive(Component, Deref, DerefMut)]
struct Lifetime(Duration);

#[derive(Component, Deref, DerefMut)]
struct Radius(f32);

#[derive(Component, Deref, DerefMut)]
struct Mass(f32);

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

#[derive(Component)]
struct Trail { last_pos: Vec3 }

#[derive(Component)]
struct TrailLine {
    start: Vec3,
    end: Vec3,
    alpha: f32
}

#[derive(Component)]
struct Explosion;

#[derive(Component)]
struct GravityVis {
    radius: f32
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle {
        transform: Transform::from_xyz(0.0, 0.0, 5.0),
        ..Default::default()
    });
}

fn setup_title(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = asset_server.load("LASER_REGULAR.otf");
    let text_style = TextStyle {
        font,
        font_size: 120.0,
        color: Color::WHITE,
    };
    let text_alignment = TextAlignment::CENTER;

    commands.spawn(
        Text2dBundle {
            text: Text::from_section("SCHWARZSCHILD", text_style.clone())
                .with_alignment(text_alignment),
            ..default()
        }
    );
}

fn update_title(mut state: ResMut<State<GameState>>, keyboard_input: Res<Input<KeyCode>>) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        state.set(GameState::Playing).unwrap();
    }
}

fn teardown_title(mut commands: Commands, entities: Query<Entity, With<Text>>) {
    for entity in &entities {
        commands.entity(entity).despawn_recursive();
        println!("deleted entity...");
    }
}

fn setup_playing(mut commands: Commands) {
    let player_start = Vec3::new(0.0, 300.0, 0.0);
    commands.spawn((Ship { fire_delay: Duration::from_millis(0) },
                    Radius(SHIP_RADIUS),
                    Mass(SHIP_MASS),
                    Transform::from_translation(player_start),
                    Velocity(Vec2::new(0.0,0.0)),
                    Trail { last_pos: player_start }));
    commands.spawn((Planet,
                    Radius(PLANET_START_RADIUS),
                    Mass(PLANET_START_MASS),
                    Transform::from_xyz(0.0, 0.0, 0.0),
                    GravityVis { radius: 0.0 } ));
}

fn check_player(mut state: ResMut<State<GameState>>, query: Query<&Ship>) {
    if query.is_empty() {
        state.set(GameState::GameOver).unwrap();
    }
}

fn teardown_playing(mut commands: Commands, entities: Query<Entity, Or<(With<Ship>, With<Planet>, With<Bullet>, With<Asteroid>, With<Explosion>, With<TrailLine>)>>) {
    for entity in &entities {
        commands.entity(entity).despawn_recursive();
    }
}

fn setup_gameover(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = asset_server.load("LASER_REGULAR.otf");
    let text_style = TextStyle {
        font,
        font_size: 60.0,
        color: Color::WHITE,
    };
    let text_alignment = TextAlignment::CENTER;

    commands.spawn(
        Text2dBundle {
            text: Text::from_section("GAME OVER", text_style.clone())
                .with_alignment(text_alignment),
            ..default()
        }
    );
}

fn update_gameover(mut state: ResMut<State<GameState>>, keyboard_input: Res<Input<KeyCode>>) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        state.set(GameState::Playing).unwrap();
    }
}

fn teardown_gameover(mut commands: Commands, entities: Query<Entity, With<Text>>) {
    for entity in &entities {
        commands.entity(entity).despawn_recursive();
    }
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

fn apply_gravity(planet_query: Query<(&Transform, &Mass), With<Planet>>, mut entity_query: Query<(&Transform, &Mass, &mut Velocity)>, time: Res<Time>) {
    for (planet_transform, planet_mass) in &planet_query {
        let planet_mass = **planet_mass;
        for (entity_transform, entity_mass, mut entity_velocity) in &mut entity_query {
            let entity_mass = **entity_mass;
            let gravity_vector = (planet_transform.translation - entity_transform.translation).normalize();
            let distance = (planet_transform.translation - entity_transform.translation).length();
            let gravity_force = GRAVITY * (planet_mass * entity_mass) / f32::max(1.0, distance * distance);
            entity_velocity.x += gravity_vector.x * gravity_force * time.delta_seconds();
            entity_velocity.y += gravity_vector.y * gravity_force * time.delta_seconds();
        }
    }
}

fn apply_asteroid_gravity(mut query: Query<(&Transform, &Radius, &Mass, &mut Velocity), With<Asteroid>>, time: Res<Time>) {
    let mut iter = query.iter_combinations_mut();
    while let Some([(transform1, Radius(radius1), Mass(mass1), mut velocity1), (transform2, Radius(radius2), Mass(mass2), mut velocity2)]) = iter.fetch_next() {
        let gravity_vector = (transform1.translation - transform2.translation).normalize();
        let distance = f32::max(radius1 + radius2, (transform1.translation - transform2.translation).length());
        let gravity_force = GRAVITY * mass1 * mass2 / f32::max(1.0, distance * distance);
        velocity1.x -= gravity_vector.x * gravity_force * time.delta_seconds();
        velocity1.y -= gravity_vector.y * gravity_force * time.delta_seconds();
        velocity2.x += gravity_vector.x * gravity_force * time.delta_seconds();
        velocity2.y += gravity_vector.y * gravity_force * time.delta_seconds();
    }
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_seconds();
        transform.translation.y += velocity.y * time.delta_seconds();
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
            commands.spawn((Bullet, Lifetime(Duration::from_millis(BULLET_LIFETIME_MS)), Radius(BULLET_RADIUS), Mass(BULLET_MASS), Transform::from_translation(bullet_position), Velocity ( Vec2::new(bullet_velocity.x, bullet_velocity.y) )));
            ship.fire_delay = Duration::from_millis(SHIP_FIRE_DELAY);
        }
    }
}

fn planet_colision(mut commands: Commands, mut planet_query: Query<(&mut Radius, &mut Mass, &Transform), With<Planet>>, entity_query: Query<(Entity, &Radius, &Mass, &Transform, Option<&Bullet>), Without<Planet>>) {
    for (mut planet_radius, mut planet_mass, planet_transform) in &mut planet_query {
        let mut planet_radius_value = **planet_radius;
        let mut planet_mass_value = **planet_mass;
        for (entity, entity_radius, entity_mass, entity_transform, optional_bullet) in &entity_query {
            let entity_radius = **entity_radius;
            let entity_mass = **entity_mass;
            let distance = Vec3::distance(planet_transform.translation, entity_transform.translation);
            if distance < (planet_radius_value + entity_radius) {
                commands.entity(entity).despawn();
                if optional_bullet.is_none() {
                    planet_radius_value += entity_radius * PLANET_RADIUS_CONSUME_SCALE;
                    planet_mass_value += entity_mass * PLANET_MASS_CONSUME_SCALE;
                }
            }
        }
        **planet_radius = planet_radius_value;
        **planet_mass = planet_mass_value;
    }
}

fn asteroid_collision(mut commands: Commands, asteroid_query: Query<(Entity, &Radius, &Mass, &Transform, &Velocity), With<Asteroid>>, bullet_query: Query<(Entity, &Radius, &Transform), With<Bullet>>) {
    for (asteroid_entity, asteroid_radius, asteroid_mass, asteroid_transform, asteroid_velocity) in &asteroid_query {
        let asteroid_radius = **asteroid_radius;
        let asteroid_mass = **asteroid_mass;
        for (bullet_entity, bullet_radius, bullet_transform) in &bullet_query {
            let bullet_radius = **bullet_radius;
            let distance = Vec3::distance(asteroid_transform.translation, bullet_transform.translation);
            if distance < (asteroid_radius + bullet_radius) {
                commands.entity(asteroid_entity).despawn();
                commands.entity(bullet_entity).despawn();
                commands.spawn((Explosion,
                                Transform::from_translation(asteroid_transform.translation),
                                Velocity(Vec2::new(asteroid_velocity.x, asteroid_velocity.y)),
                                Lifetime(Duration::from_millis(EXPLOSION_MAX_LIFE_MS))));
                if asteroid_radius > ASTEROID_FRACTURE_MIN_RADIUS {
                    let mut rng = rand::thread_rng();
                    let new_radius = asteroid_radius * ASTEROID_FRACTURE_RADIUS_FACTOR;
                    let new_mass = asteroid_mass * ASTEROID_FRACTURE_MASS_FACTOR;
                    let max_angle = 2.0 * std::f32::consts::PI;
                    let angle_section = max_angle / (ASTEROID_FRACTURE_COUNT as f32);
                    let mut spawn_angle: f32 = rng.gen_range(0.0..max_angle);
                    for _i in 0..ASTEROID_FRACTURE_COUNT {
                        let spawn_x = asteroid_transform.translation.x + new_radius * spawn_angle.cos();
                        let spawn_y = asteroid_transform.translation.y + new_radius * spawn_angle.sin();
                        let shape_seed = rng.gen::<u64>();
                        let asteroid_velocity_x = asteroid_velocity.x + spawn_angle.cos() * rng.gen_range(ASTEROID_FRACTURE_VEL_MIN..ASTEROID_FRACTURE_VEL_MAX);
                        let asteroid_velocity_y = asteroid_velocity.y + spawn_angle.sin() * rng.gen_range(ASTEROID_FRACTURE_VEL_MIN..ASTEROID_FRACTURE_VEL_MAX);
                        commands.spawn((Asteroid { seed: shape_seed },
                                        Radius(new_radius),
                                        Mass(new_mass),
                                        Transform::from_xyz(spawn_x, spawn_y, 0.0), 
                                        Velocity(Vec2::new(asteroid_velocity_x, asteroid_velocity_y)),
                                        Lifetime(Duration::from_millis(ASTEROID_LIFETIME_MS))
                                        ));
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
        let max_angle = 2.0 * std::f32::consts::PI;
        let spawn_angle: f32 = rng.gen_range(0.0..max_angle);
        let spawn_x = ASTEROID_SPAWN_DISTANCE * spawn_angle.cos();
        let spawn_y = ASTEROID_SPAWN_DISTANCE * spawn_angle.sin();
        let asteroid_radius = rng.gen_range(ASTEROID_RADIUS_MIN..ASTEROID_RADIUS_MAX);
        let asteroid_mass = rng.gen_range(ASTEROID_MASS_MIN..ASTEROID_MASS_MAX);
        let shape_seed = rng.gen::<u64>();
        let asteroid_speed = rng.gen_range(ASTEROID_VELOCITY_MIN..ASTEROID_VELOCITY_MAX);
        let velocity_angle = spawn_angle + max_angle / 3.5;
        let asteroid_velocity_x = asteroid_speed * velocity_angle.cos();
        let asteroid_velocity_y = asteroid_speed * velocity_angle.sin();
        commands.spawn((Asteroid { seed: shape_seed },
                        Radius(asteroid_radius),
                        Mass(asteroid_mass),
                        Transform::from_xyz(spawn_x, spawn_y, 0.0),
                        Velocity(Vec2::new(asteroid_velocity_x, asteroid_velocity_y)),
                        Lifetime(Duration::from_millis(ASTEROID_LIFETIME_MS))
                        ));
        asteroid_timer.duration = Duration::from_millis(rng.gen_range(ASTEROID_SPAWN_DELAY_MIN_MS..ASTEROID_SPAWN_DELAY_MAX_MS));
    }
}

fn asteroid_drag(planet_query: Query<(&Transform, &Radius), With<Planet>>, mut asteroid_query: Query<(&Transform, &Radius, &mut Velocity), With<Asteroid>>, time: Res<Time>) {
    for (planet_transform, planet_radius) in &planet_query {
        let planet_radius = **planet_radius;
        for (asteroid_transform, asteroid_radius, mut asteroid_velocity) in &mut asteroid_query {
            let asteroid_radius = **asteroid_radius;
            let distance = Vec3::distance(planet_transform.translation, asteroid_transform.translation) - planet_radius;
            let drag_factor = time.delta_seconds() * (ASTEROID_DRAG_CONSTANT + asteroid_radius * ASTEROID_DRAG_RADIUS_CONTRIBUTION) / distance;
            let mut asteroid_speed = asteroid_velocity.length();
            if asteroid_speed > drag_factor {
                asteroid_speed -= drag_factor;
            } else {
                asteroid_speed = 0.0;
            }
            **asteroid_velocity = asteroid_velocity.normalize() * asteroid_speed;
        }
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
        draw_irregular_circle(&mut lines, asteroid.seed, position, radius - ASTEROID_RADIUS_VARIANCE, radius + ASTEROID_RADIUS_VARIANCE, Color::GREEN, ASTEROID_POINT_COUNT);
    }
}

fn draw_trail(mut commands: Commands, mut query: Query<(&Transform, &mut Trail)>) {
    for (transform, mut trail) in &mut query {
        commands.spawn((TrailLine{ start: transform.translation, end: trail.last_pos, alpha: TRAIL_START_ALPHA },
                        Lifetime(Duration::from_millis(TRAIL_MAX_LIFE_MS))));
        trail.last_pos = transform.translation;
    }
}

fn draw_trail_lines(query: Query<(&TrailLine, &Lifetime)>, mut lines: ResMut<DebugLines>) {
    for (line, lifetime) in &query {
        let alpha = line.alpha * (lifetime.as_millis() as f32 / TRAIL_MAX_LIFE_MS as f32);
        lines.line_colored(line.start, line.end, 0.0, Color::rgba(1.0, 1.0, 1.0, alpha));
    }
}

fn draw_explosion(query: Query<(&Transform, &Lifetime), With<Explosion>>, mut lines: ResMut<DebugLines>) {
    for (transform, lifetime) in &query {
        let age = (EXPLOSION_MAX_LIFE_MS - lifetime.as_millis() as u64) as f32;
        let factor = 1.0 - (1.0 - age / EXPLOSION_MAX_LIFE_MS as f32).powf(2.0);
        let radius = factor * EXPLOSION_MAX_RADIUS;
        let alpha = 0.5 * lifetime.as_millis() as f32 / EXPLOSION_MAX_LIFE_MS as f32;
        draw_circle(&mut lines, transform.translation, radius, Color::rgba(1.0, 1.0, 1.0, alpha), 20);
    }
}

fn update_gravity_vis(mut query: Query<(&Mass, &mut GravityVis)>, time: Res<Time>) {
    for (Mass(mass), mut gravity_vis) in &mut query {
        let shrink = time.delta_seconds() * GRAVITY_VIS_RATE * GRAVITY_VIS_MASS_FACTOR.powf(*mass - PLANET_START_MASS);
        if gravity_vis.radius > shrink {
            gravity_vis.radius -= shrink;
        } else {
            let remainder = shrink % 1.0;
            gravity_vis.radius = 1.0 - remainder;
        }
    }
}

fn visualise_gravity(query: Query<(&Transform, &Radius, &GravityVis)>, mut lines: ResMut<DebugLines>) {
    for (transform, Radius(radius), gravity_vis) in &query {
        let radius = radius + GRAVITY_VIS_SIZE * gravity_vis.radius;
        draw_circle(&mut lines, transform.translation, radius, Color::rgba(1.0, 1.0, 1.0, 0.05), 40);
    }
}

fn main() {
    App::new()
    .add_plugins(DefaultPlugins)
    .add_plugin(DebugLinesPlugin::default())
    .insert_resource(AsteroidTimer{ duration: Duration::from_secs(5) })
    .add_state(GameState::Title)
    .add_startup_system(setup_camera)
    .add_system_set(SystemSet::on_enter(GameState::Title).with_system(setup_title))
    .add_system_set(SystemSet::on_update(GameState::Title).with_system(update_title))
    .add_system_set(SystemSet::on_exit(GameState::Title).with_system(teardown_title))
    .add_system_set(SystemSet::on_enter(GameState::Playing).with_system(setup_playing))
    .add_system_set(SystemSet::on_update(GameState::Playing)
        .with_system(ship_control)
        .with_system(apply_gravity)
        .with_system(apply_velocity)
        .with_system(asteroid_drag)
        .with_system(planet_colision)
        .with_system(asteroid_collision)
        .with_system(fire_control)
        .with_system(lifetime_control)
        .with_system(space_clamp)
        .with_system(asteroid_spawner)
        .with_system(update_gravity_vis)
        .with_system(check_player)
    )
    .add_system_set(SystemSet::on_exit(GameState::Playing).with_system(teardown_playing))
    .add_system_set(SystemSet::on_enter(GameState::GameOver).with_system(setup_gameover))
    .add_system_set(SystemSet::on_update(GameState::GameOver).with_system(update_gameover))
    .add_system_set(SystemSet::on_exit(GameState::GameOver).with_system(teardown_gameover))
    .add_system(ship_render)
    .add_system(planet_render)
    .add_system(bullet_render)
    .add_system(asteroid_render)
    .add_system(draw_trail)
    .add_system(draw_trail_lines)
    .add_system(draw_explosion)
    .add_system(visualise_gravity)
    .add_system(bevy::window::close_on_esc)
    .run();
}
