use bevy::prelude::*;
use noise::{NoiseFn, Simplex};
use rand::Rng;
use rayon::prelude::*;
use std::time::Instant;

const WIDTH: usize = 720;
const HEIGHT: usize = 480;
const VARIABLES: usize = 3; // Example: 0 = terrain, 1 = empire, 2 = strength
const OCEAN_CUTOFF: f32 = 0.4;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_systems(Startup, setup);
    app.add_systems(Update, (update_cells,update_colors, draw_fps));
    app.insert_resource(RenderMode::StrengthView);
    app.run();
}

fn setup(mut commands: Commands, windows: Query<&mut Window>) {
    let window_width = windows.iter().next().unwrap().width();
    let window_height = windows.iter().next().unwrap().height();
    let scale_x = WIDTH as f32 / window_width;
    let scale_y = HEIGHT as f32 / window_height;
    let scale = scale_x.max(scale_y);

    commands.spawn(Camera2dBundle {
        transform: Transform::from_xyz(WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0, 100.0),
        projection: OrthographicProjection {
            scale: scale,
            ..Default::default()
        },
        ..Default::default()
    });
    commands.insert_resource(Grid::new(WIDTH, HEIGHT, VARIABLES));

    commands.spawn(TextBundle {
        text: Text::from_section(
            "FPS: 0.00",
            TextStyle {
                font_size: 30.0,
                color: Color::WHITE,
                ..Default::default()
            }
        ).with_justify(JustifyText::Right),
        transform: Transform::from_xyz(window_width / 2.0 - 10.0, window_height / 2.0 - 10.0, 0.0),
        ..Default::default()
    });

    commands.insert_resource(LastDraw::default());

    // Initialize sprites
    for x in 0..WIDTH {
        for y in 0..HEIGHT {
            commands.spawn(SpriteBundle {
                sprite: Sprite {
                    color: Color::WHITE,
                    custom_size: Some(Vec2::new(1.0, 1.0)),
                    ..Default::default()
                },
                transform: Transform::from_xyz(x as f32, y as f32, 0.0),
                ..Default::default()
            });
        }
    }
}

#[derive(Resource)]
struct Grid {
    data: Vec<Vec<Vec<f32>>>,
}

impl Grid {
    fn new(width: usize, height: usize, variables: usize) -> Self {
        let mut data = vec![vec![vec![0.0; variables]; height]; width];
        let mut rng = rand::thread_rng();
        let noise = Simplex::new(rng.gen::<u32>()); //Billow<_> = Billow::new(rng.gen::<u32>());
        let noise2 = Simplex::new(rng.gen::<u32>());
        let noise3 = Simplex::new(rng.gen::<u32>());
        
        data.par_iter_mut().enumerate().for_each(|(x, row)| {
            row.iter_mut().enumerate().for_each(|(y, cell)| {
                let elevation = noise.get([x as f64 / 100.0, y as f64 / 100.0]) as f32 + noise2.get([x as f64 / 50.0, y as f64 / 50.0]) as f32 / 2.0 + noise3.get([x as f64 / 25.0, y as f64 / 25.0]) as f32 / 4.0;
                cell[0] = (elevation + 1.0) / 2.0;
                cell[1] = -1.0;
            });
        });

        for _ in 0..100 {
            let x = rng.gen_range(0..WIDTH);
            let y = rng.gen_range(0..HEIGHT);
            if data[x][y][0] >= OCEAN_CUTOFF {
                data[x][y][1] = rng.gen_range(0.0..100.0);
            }
        }

        Grid { data }
    }

    fn update(&mut self) {
        self.data.par_iter_mut().for_each(|row| {
            row.iter_mut().for_each(|cell| {//strength phase
                if cell[1] != -1.0 {
                    cell[2] += 1.0 - (OCEAN_CUTOFF - cell[0]).abs();
                    cell[2] *= 0.99;
                }
            });
        });
        self.data.par_iter_mut().for_each(|row| {
            row.iter_mut().for_each(|cell| {//attack phase
                // Update cell logic here
                // Example update
            });
        });
        self.data.par_iter_mut().for_each(|row| {
            row.iter_mut().for_each(|cell| {//need phase
                // Update cell logic here
                // Example update
            });
        });
        self.data.par_iter_mut().for_each(|row| {
            row.iter_mut().for_each(|cell| {//need spread phase
                // Update cell logic here
                // Example update
            });
        });
        self.data.par_iter_mut().for_each(|row| {
            row.iter_mut().for_each(|cell| {//resource phase
                // Update cell logic here
                // Example update
            });
        });
    }
}

#[derive(Resource)]
enum RenderMode {
    StrengthView,
    EmpireView,
    TerrainView,
    // Add more render modes here
}

fn update_cells(mut grid: ResMut<Grid>) {
    grid.update();
}

fn update_colors(
    grid: Res<Grid>,
    render_mode: Res<RenderMode>,
    mut query: Query<(&Transform, &mut Sprite)>,
) {
    // Collect query results into a vector
    let mut query_results: Vec<(&Transform, Mut<Sprite>)> = query.iter_mut().collect();

    // Use Rayon to iterate over the vector in parallel
    query_results.par_iter_mut().for_each(|(transform,ref mut sprite)| {
        let x = transform.translation.x as usize;
        let y = transform.translation.y as usize;
        let cell = &grid.data[x][y];

        let color = if cell[1] == -1.0 || matches!(*render_mode, RenderMode::TerrainView) {
            if cell[0] < OCEAN_CUTOFF {
                //ocean
                let brightness = cell[0] / 2.0 + OCEAN_CUTOFF / 2.0;//cell[0] + 0.01 / (cell[0].sqrt());
                Color::hsla(240.0, 1.0, brightness, 1.0)
            } else {
                //land
                let brightness = cell[0] / 3.0 + OCEAN_CUTOFF / 3.0;
                Color::hsla(100.0 + (cell[0] - 0.5) * 30.0 * (1.0 / OCEAN_CUTOFF), 1.0, brightness, 1.0)
            }
        } else {
            match *render_mode {
                RenderMode::StrengthView => {
                    let hue = cell[1] / 10.0;
                    let brightness = cell[2];
                    Color::hsla(hue, 1.0, brightness, 1.0)
                }
                RenderMode::EmpireView => {
                    let hue = cell[1] / 10.0;
                    Color::hsla(hue, 1.0, 0.5, 1.0)
                }
                _ => Color::WHITE,
            }
        };

        sprite.color = color;
    });
}

#[derive(Resource)]
struct LastDraw {
    time: Instant,
}

impl Default for LastDraw {
    fn default() -> Self {
        LastDraw {
            time: Instant::now(),
        }
    }
}

fn draw_fps(
    mut last_draw: ResMut<LastDraw>,
    mut query: Query<(&mut Text, &mut Transform)>,
) {
    let now = Instant::now();
    let duration = now.duration_since(last_draw.time);
    let fps = 1.0 / duration.as_secs_f32();

    // Update the last_draw time
    last_draw.time = now;

    // Update the FPS text
    for (mut text, mut transform) in query.iter_mut() {
        text.sections[0].value = format!("FPS: {:.2}", fps);
        transform.translation = Vec3::new(0.0, 0.0, 0.0); // Adjust the position as needed
    }
}