use bevy::{prelude::*, utils::HashMap};
use noise::{NoiseFn, Simplex};
use rand::Rng;
use rayon::prelude::*;
use std::time::Instant;
use std::env;

const WIDTH: usize = 426;
const HEIGHT: usize = 240;
const VARIABLES: usize = 4; // Terrain, strength, empire
const OCEAN_CUTOFF: f32 = 0.4;

fn main() {
    env::set_var("RUST_BACKTRACE", "full");
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_systems(Startup, setup);
    app.add_systems(Update, (update_colors, draw_fps));
    app.add_systems(PreUpdate, (pull_system.before(update_cell_map_system), update_cell_map_system));
    app.add_systems(PostUpdate, (push_system.before(update_cell_map_system), update_cell_map_system));
    app.insert_resource(RenderMode::EmpireView);
    app.insert_resource(CellMap(HashMap::default()));
    app.run();
}

fn setup(mut commands: Commands, windows: Query<&mut Window>, mut entity_map: ResMut<CellMap>) {
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
    let mut empire_count = 0;
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
                // chance to spawn an empire using cell.set_empire()
                let mut empire = -1;
                if rand::thread_rng().gen_range(0..1000) < 1 {
                    empire = empire_count;
                    empire_count += 1;
                    println!("Empire {} has been created at ({}, {})", empire, x, y);
                }

                commands.spawn(Cell::new(x, y, terrain, empire));
                entity_map.0.insert((x, y), ((x, y), empire, 0.0, 0.0, (0, 0), 0.0, empire));
            }
        }
    }

    commands.insert_resource(grid);
}

#[derive(Resource)]
struct CellMap(HashMap<(usize, usize), ((usize, usize), i32, f32, f32, (usize, usize), f32, i32)>);

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
    position: (usize, usize),
    empire: i32,
    strength: f32,
    need: f32,
    send_target: (usize, usize),
    send_amount: f32,
    send_empire: i32,
    terrain: f32,
}

impl Cell {
    fn new(x: usize, y: usize, terrain: f32, empire: i32) -> Self {
        Cell {            
            position: (x, y),
            empire: empire,
            strength: 0.0,
            need: 0.0,
            send_target: (x, y),
            send_amount: 0.0,
            send_empire: empire,
            terrain,
        }
    }

    fn get(& self) -> ((usize, usize), i32, f32, f32, (usize, usize), f32, i32) {
        //0 = position, 1 = empire, 2 = strength, 3 = need, 4 = send_target, 5 = send_amount, 6 = send_empire
        (self.position, self.empire, self.strength, self.need, self.send_target, self.send_amount, self.send_empire)
    }

    //neighbors are the 8 cells surrounding this cell, accessible through the hashmap.
    fn push(&mut self, data: Vec<((usize, usize), i32, f32, f32, (usize, usize), f32, i32)>) {//I call this 'push' because the cell is reading data from neighbors and pushing a decision
        let mut max_enemy_strength = 0.0;
        let mut max_need = 0.0;
        let mut max_need_position = self.position;
        let mut total_need = 0.0;
        let mut min_enemy_strength = 0.0;
        let mut min_enemy_position = self.position;

        if self.empire == -1 {
            return;
        }

        //println!("Empire {} has strength {} and need {} at ({}, {})", self.empire, self.strength, self.need, self.position.0, self.position.1);

        //iterate through the 8 possible neighbor positions
        for i in 0..data.len() {
            if let Some(neighbor_cell) = data.get(i) {
                if self.empire == neighbor_cell.1 {
                    if neighbor_cell.3 > max_need {
                        max_need = neighbor_cell.3;
                        max_need_position = neighbor_cell.0;
                    }
                    total_need += neighbor_cell.3 / 2.0;
                } else {
                    if neighbor_cell.2 > max_enemy_strength {
                        max_enemy_strength = neighbor_cell.2;
                    }
                    if neighbor_cell.2 < min_enemy_strength || min_enemy_position == self.position {
                        min_enemy_strength = neighbor_cell.2;
                        min_enemy_position = neighbor_cell.0;
                    }
                    self.need += neighbor_cell.2 / 3.0;
                }
            }
        }
        println!("Empire {} has strength {} and need {} at ({}, {})", self.empire, self.strength, self.need, self.position.0, self.position.1);
        if self.strength > self.need {
            println!("Empire {} has enough strength to send reinforcements or attacks", self.empire);
            //safe from attack this turn
            let mut extra = self.strength - total_need;
            if max_need > 0.0 && max_need_position != self.position {
                //send strength to cell with max need
                (self.send_target.0, self.send_target.1) = (max_need_position.0, max_need_position.1);
                self.send_amount = (self.strength - total_need).min(max_need);
                self.send_empire = self.empire;
                extra -= self.send_amount;
                println!("Empire {} is sending {} reinforcements to cell ({}, {})", self.empire, self.send_amount, self.send_target.0, self.send_target.1);
            }
            println!("Extra is {}, {} required", extra, (3.0 * min_enemy_strength));
            println!("Position is ({}, {}), this cell is at ({}, {})", min_enemy_position.0, min_enemy_position.1, self.position.0, self.position.1);
            if extra > 3.0 * min_enemy_strength && min_enemy_position != self.position {
                //send strength to attack weakest enemy
                (self.send_target.0, self.send_target.1) = min_enemy_position;
                self.send_amount = extra;
                self.send_empire = self.empire;
                println!("Empire {} is attacking cell ({}, {}) with {} strength", self.empire, self.send_target.0, self.send_target.1, self.send_amount);
            }
        } else {
            self.send_target = self.position;
            self.send_amount = 0.0;
            self.send_empire = self.empire;
        }
        self.strength -= self.send_amount;
        self.need = self.need + total_need;
    }

    fn pull(&mut self, data: Vec<((usize, usize), i32, f32, f32, (usize, usize), f32, i32)>) {//I call this 'pull' because the cell is pulling the decisions from other cells to update its own data
        // Check the send_ variables of all neighbors to see if they are sending strength to this cell
        for i in 0..data.len() {// First add reinforcements from friendly cells to this cell's strength
            if let Some(neighbor_cell) = data.get(i) {
                if neighbor_cell.6 == self.empire && neighbor_cell.4 == self.position {
                    self.strength += neighbor_cell.5;
                }
            }
        }
        // Then divide incoming strength from enemy cells by 3 and subtract it from this cell's strength. Handle attacks from weakest to strongest.
        // If an attack causes strength to go below 0, change this cell's owner to the attacking empire and multiply strength by -1, all further attacks will be considered reinforcements
        for i in 0..data.len() {
            if let Some(neighbor_cell) = data.get(i) {
                if neighbor_cell.6 != self.empire && neighbor_cell.4 == self.position {
                    println!("Empire {} is attacking cell ({}, {})", neighbor_cell.6, self.position.0, self.position.1);
                    if self.strength - neighbor_cell.5 / 3.0 < 0.0 {
                        self.empire = neighbor_cell.6;
                        println!("Empire {} has taken cell ({}, {})", self.empire, self.position.0, self.position.1);
                        self.strength = neighbor_cell.5 / 3.0 - self.strength;
                    } else {
                        self.strength -= neighbor_cell.5 / 3.0;
                    }
                }
            }
        }
        if self.empire != -1 {
            // Use terrain data from the grid to determine how much strength this cell should generate. The closer to ocean level, the more strength is made.
            self.strength += self.terrain;
            // Multiply strength by 0.99 so it can't just go up forever.
            self.strength *= 0.99;
        }
    }
}

fn push_system(mut query: Query<&mut Cell>, cell_map: Res<CellMap>) {
    query.iter_mut().for_each(|mut cell| {//iterate through all cells on many threads
        let position = cell.position;//get cell's position
        let mut data = Vec::new();//initialize data to be sent to cell.push
        for i in 0..8 {//iterate through the 8 possible neighbor positions
            if let Some(neighbor) = cell_map.0.get(&(position.0 + i % 3 - 1, position.1 + i / 3 - 1)) {
                data.push((neighbor.0, neighbor.1, neighbor.2, neighbor.3, neighbor.4, neighbor.5, neighbor.6));
            }
        }
        cell.push(data);
    });
}

//iterate through all cells, run the get() function on them to update CellMap
fn update_cell_map_system(mut cell_map: ResMut<CellMap>, query: Query<&Cell>) {
    query.iter().for_each(|cell| {
        cell_map.0.insert(cell.position, cell.get());
    });
}

fn pull_system(mut query: Query<&mut Cell>, cell_map: Res<CellMap>) {
    query.iter_mut().for_each(|mut cell| {//iterate through all cells on many threads
        let position = cell.position;//get cell's position
        let mut data = Vec::new();//initialize data to be sent to cell.push
        for i in 0..8 {//iterate through the 8 possible neighbor positions
            if let Some(neighbor) = cell_map.0.get(&(position.0 + i % 3 - 1, position.1 + i / 3 - 1)) {
                data.push((neighbor.0, neighbor.1, neighbor.2, neighbor.3, neighbor.4, neighbor.5, neighbor.6));
            }
        }
        cell.pull(data);
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
    cell_map: Res<CellMap>,
    render_mode: Res<RenderMode>,
    mut query: Query<(&Transform, &mut Sprite)>,
) {
    // Collect query results into a vector
    let mut query_results: Vec<(&Transform, Mut<Sprite>)> = query.iter_mut().collect();

    // Use Rayon to iterate over the vector in parallel
    query_results.par_iter_mut().for_each(|(transform,ref mut sprite)| {
        let x = transform.translation.x as usize;
        let y = transform.translation.y as usize;
        let terrain = &grid.data[x][y];

        //some grid spots don't have cells because they are ocean
        //check if a cell exists at this position before trying to access it
        //let cell = cell_map.0.get(&(x, y)).unwrap_or(&((0, 0), -1, 0.0, 0.0, (0, 0), 0.0, -1));

        let cell = cell_map.0.get(&(x, y)).unwrap_or(&((0, 0), -1, 0.0, 0.0, (0, 0), 0.0, -1));

        let color = if matches!(*render_mode, RenderMode::TerrainView) || cell.1 == -1 {
            if terrain[0] < OCEAN_CUTOFF {
                //ocean
                let brightness = terrain[0] / 2.0 + OCEAN_CUTOFF / 2.0;//cell[0] + 0.01 / (cell[0].sqrt());
                Color::hsla(240.0, 1.0, brightness, 1.0)
            } else {
                //land
                let brightness = terrain[0] / 3.0 + OCEAN_CUTOFF / 3.0;
                Color::hsla(100.0 + (terrain[0] - 0.5) * 30.0 * (1.0 / OCEAN_CUTOFF), 1.0, brightness, 1.0)
            }
        } else {
            //println!("Empire {} has strength {} and need {} at ({}, {})", cell.1, cell.2, cell.3, x, y);
            match *render_mode {
                RenderMode::StrengthView => {
                    let hue = cell.1 as f32 / 70.0 * 360.0;
                    let brightness = cell.2 as f32 / 10.0;
                    Color::hsla(hue, 1.0, brightness, 1.0)
                }
                RenderMode::EmpireView => {
                    let hue = cell.1 as f32  / 70.0 * 360.0;
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