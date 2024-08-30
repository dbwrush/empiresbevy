use bevy::{prelude::*, utils::HashMap};
use noise::{NoiseFn, Simplex};
use rand::Rng;
use rayon::prelude::*;
use std::time::Instant;

const WIDTH: usize = 426;
const HEIGHT: usize = 240;
const VARIABLES: usize = 4; // Terrain, strength, empire
const OCEAN_CUTOFF: f32 = 0.4;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_systems(Startup, setup);
    app.add_systems(Update, (update_colors, draw_fps));
    app.add_systems(Update, pull_system);
    app.add_systems(PostUpdate, push_system);
    app.insert_resource(RenderMode::StrengthView);
    app.insert_resource(CellMap(HashMap::default()));
    app.run();
}

fn setup(mut commands: Commands, windows: Query<&mut Window>, mut cell_map: ResMut<CellMap>) {
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

    let grid = Grid::new(WIDTH, HEIGHT, VARIABLES);

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
            let terrain = grid.data[x][y][0];
            commands.spawn(SpriteBundle {
                sprite: Sprite {
                    color: Color::WHITE,
                    custom_size: Some(Vec2::new(1.0, 1.0)),
                    ..Default::default()
                },
                transform: Transform::from_xyz(x as f32, y as f32, 0.0),
                ..Default::default()
            });
            if terrain > OCEAN_CUTOFF {
                let entity = commands.spawn(Cell::new(x, y)).id();
                cell_map.0.insert((x, y), entity);
            }

        }
    }

    commands.insert_resource(grid);
}

#[derive(Resource)]
struct CellMap(HashMap<(usize, usize), Entity>);

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

        Grid { data}
    }
}

#[derive(Component)]
struct Cell {
    empire: i32,
    strength: f32,
    need: f32,
    position: (usize, usize),
    send_amount: f32,
    send_target: (usize, usize),
    send_empire: i32, //this is the empire which commits an attack, in case a cell changes hands while it has already committed forces to an attack
    //send_empire should be updated to the owner of the cell during the push phase.
    //consider adding a pointer to all land neighbors (ignore ocean since no entities are made for ocean)
}

impl Cell {
    fn new(x: usize, y: usize) -> Self {
        Cell {
            empire: -1,
            strength: 0.0,
            need: 0.0,
            position: (x, y),
            send_amount: 0.0,
            send_target: (0, 0),
            send_empire: -1,
        }
    }

    fn push(&mut self, neighbors: &HashMap<(usize, usize), Entity>) {//I call this 'push' because the cell is reading data from neighbors and pushing a decision
        // Read need, strength, and owner data from neighbors (no need for it to be mutable), use it to decide if a cell should hold, send reinforcements,
        // or attack a neighbor
        // Cells need 3x strength to win an attack, so they can afford to have 1/3 the strength of the strongest enemy neighbor
        // If strength is > 0.3x the strongest enemy neighbor, it can survive attacks, extra strength can be sent as attacks or reinforcements
        // Check neighboring friendly (same empire) cells for need, send extra strength to them first in order of need
        // Check neighboring enemy cells for strength, if extra strength is > 0.3x the weakest enemy then attack the weakest enemy
    }

    fn pull(&mut self, neighbors: &HashMap<(usize, usize), Entity>) {//I call this 'pull' because the cell is pulling the decisions from other cells to update its own data
        // Check the send_ variables of all neighbors to see if they are sending strength to this cell
        // First add reinforcements from friendly cells to this cell's strength
        // Then divide incoming strength from enemy cells by 3 and subtract it from this cell's strength. Handle attacks from weakest to strongest.
        // If an attack causes strength to go below 0, change this cell's owner to the attacking empire and multiply strength by -1, all further attacks will be considered reinforcements

        // Update cell data in the grid for rendering.

        // Use terrain data from the grid to determine how much strength this cell should generate. The closer to ocean level, the more strength is made.
        // Multiply strength by 0.99 so it can't just go up forever.
    }
}

fn push_system(mut query: Query<&mut Cell>, cell_map: Res<CellMap>) {
    query.par_iter_mut().for_each(|mut cell| {
        cell.push(&cell_map.0);
    });
}

fn pull_system(mut query: Query<&mut Cell>, cell_map: Res<CellMap>) {
    query.par_iter_mut().for_each(|mut cell| {
        cell.pull(&cell_map.0);
    });
}

#[derive(Resource)]
enum RenderMode {
    StrengthView,
    EmpireView,
    TerrainView,
    // Add more render modes here
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