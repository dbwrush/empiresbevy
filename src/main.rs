use bevy::window::PrimaryWindow;
use bevy::{prelude::*, utils::HashMap};
use noise::{NoiseFn, Simplex};
use rand::Rng;
use rayon::prelude::*;
use std::time::Instant;
use std::env;

const WIDTH: usize = 16 * 30;
const HEIGHT: usize = 9 * 30;
const VARIABLES: usize = 4; // Terrain, strength, empire
const OCEAN_CUTOFF: f32 = 0.3;
const EMPIRE_PROBABILITY: i32 = 1000;
const TERRAIN_IMPORTANCE: f32 = 0.2;
const LOOP_DIST: usize = 100;
const BOAT_PROP: f32 = 0.001;

fn main() {
    env::set_var("RUST_BACKTRACE", "full");
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_systems(Startup, setup);
    app.add_systems(Update, (update_colors, draw_fps, update_render_mode_system, update_empires));
    app.add_systems(PreUpdate, (update_boats_system.before(pull_system), pull_system.before(update_cell_map_system), update_cell_map_system));
    app.add_systems(PostUpdate, (push_system.before(update_cell_map_system), update_cell_map_system));
    app.insert_resource(RenderMode::AgeView);
    app.insert_resource(GameData { max_strength: 0.0 , max_age: 0, send_boats: false });
    app.insert_resource(MapData(HashMap::default(), Vec::new()));
    app.run();
}

fn setup(mut commands: Commands, mut windows: Query<&mut Window, With<PrimaryWindow>>, mut entity_map: ResMut<MapData>) {
    let window_width = windows.iter().next().unwrap().width();
    let window_height = windows.iter().next().unwrap().height();
    let scale_x = WIDTH as f32 / window_width;
    let scale_y = HEIGHT as f32 / window_height;
    let scale = scale_x.max(scale_y);
    if let Ok(mut window) = windows.get_single_mut() {
        window.title = "Empires!".to_string();
    }


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

    let mut count = 0;

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
            }).insert(CellMarker);
            if terrain > OCEAN_CUTOFF {
                // chance to spawn an empire using cell.set_empire()
                let mut empire = -1;
                if rand::thread_rng().gen_range(0..EMPIRE_PROBABILITY) < 1 {
                    empire = empire_count;
                    empire_count += 1;
                    //println!("Empire {} has been created at ({}, {})", empire, x, y);
                    entity_map.1.push((rand::thread_rng().gen_range(0..360) as f32, rand::thread_rng().gen_range(0..1000) as f32 / 1000.0, rand::thread_rng().gen_range(0..1000) as f32 / 1000.0, 1.0));
                }
                count += 1;

                commands.spawn(Cell::new(x, y, terrain, empire));
                entity_map.0.insert((x, y), ((x, y), empire, 0.0, 0.0, (0, 0), 0.0, empire, 0, HashMap::new(), 0.0));
            }
        }
    }
    println!("{} cells created", count);
    commands.insert_resource(grid);
}

#[derive(Resource)]
struct MapData(HashMap<(usize, usize), ((usize, usize), i32, f32, f32, (usize, usize), f32, i32, u32, HashMap<(usize, usize), (i32, f32)>, f32)>, Vec<(f32, f32, f32, f32)>);
//vec is empire data including hue, saturation, aggression, and tech factor


#[derive(Resource)]
struct GameData {
    max_strength: f32,
    max_age: u32,
    send_boats: bool,
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
        
        data.iter_mut().enumerate().for_each(|(x, row)| {
            row.iter_mut().enumerate().for_each(|(y, cell)| {
                let mut elevation = get_elevation(noise, noise2, noise3, x, y);
                if x < LOOP_DIST || x > WIDTH - LOOP_DIST {
                    //at the edge, terrain will be the average of this and the opposite side.
                    let mut opp_prop = x as f32 / LOOP_DIST as f32 * -0.5 + 0.5;
                    let opp_x = WIDTH - x - 1;
                    if x > WIDTH - LOOP_DIST {
                        opp_prop = opp_x as f32 / LOOP_DIST as f32 * -0.5 + 0.5;
                    }
                    let opp_elevation = get_elevation(noise, noise2, noise3, opp_x, y);
                    elevation = elevation * (1.0 - opp_prop) + opp_elevation * opp_prop;
                }
                cell[0] = (elevation + 1.0) / 2.0;
                cell[1] = -1.0;
            });
        });

        Grid { data}
    }
}

fn get_elevation(noise: Simplex, noise2: Simplex, noise3: Simplex, x: usize, y: usize) -> f32 {
    return noise.get([x as f64 / 200.0, y as f64 / 200.0]) as f32 * 2.0 + noise.get([x as f64 / 100.0, y as f64 / 100.0]) as f32 + noise2.get([x as f64 / 50.0, y as f64 / 50.0]) as f32 / 2.0 + noise3.get([x as f64 / 25.0, y as f64 / 25.0]) as f32 / 4.0 + noise3.get([x as f64 / 12.5, y as f64 / 12.5]) as f32 / 8.0;
}

#[derive(Component)]
struct CellMarker;

#[derive(Component)]
struct Boat {
    direction: (f32, f32),
    strength: f32,
    empire: i32,
}

impl Boat {
    fn move_boat(&mut self, mut position: (i32, i32)) -> (usize, usize) {
        //use direction values to decide if boat moves on one axis or both randomly
        //if direction value is negative, that means the boat is moving in the negative direction
        if rand::thread_rng().gen_range(0..1000) as f32 / 1000.0 < self.direction.0.abs() {
            if self.direction.0 > 0.0 {
                position.0 += 1;
            } else {
                position.0 -= 1;
            }
        }
        if rand::thread_rng().gen_range(0..1000) as f32 / 1000.0 < self.direction.1.abs() {
            if self.direction.1 > 0.0 {
                position.1 += 1;
            } else {
                position.1 -= 1;
            }
        }
        if position.0 >= WIDTH as i32 {
            position.0 -= WIDTH as i32;
        }
        if position.0 < 0 {
            position.0 += WIDTH as i32;
        }
        return (position.0 as usize, position.1 as usize);
    }
}

#[derive(Component)]
struct Cell {
    position: (usize, usize),
    empire: i32,
    strength: f32,
    need: f32,
    boat_need: f32,
    send_target: (usize, usize),
    send_amount: f32,
    send_empire: i32,
    terrain: f32,
    age: u32,
    ocean_need_prop: f32,
    boat_target: (usize, usize),
    boat_strength: f32,
}

impl Cell {
    fn new(x: usize, y: usize, terrain: f32, empire: i32) -> Self {
        Cell {            
            position: (x, y),
            empire,
            strength: terrain,
            need: 0.0,
            boat_need: 0.0,
            send_target: (x, y),
            send_amount: 0.0,
            send_empire: empire,
            terrain,
            age: 0,
            ocean_need_prop: 0.0,
            boat_target: (0, 0),
            boat_strength: 0.0,
        }
    }

    fn get(& self) -> ((usize, usize), i32, f32, f32, (usize, usize), f32, i32, u32, HashMap<(usize, usize), (i32, f32)>, f32) {
        //0 = position, 1 = empire, 2 = strength, 3 = need, 4 = send_target, 5 = send_amount, 6 = send_empire
        (self.position, self.empire, self.strength, self.need, self.send_target, self.send_amount, self.send_empire, self.age, HashMap::new(), self.boat_need)
    }

    //neighbors are the 8 cells surrounding this cell, accessible through the hashmap.
    fn push(&mut self, data: Vec<((usize, usize), i32, f32, f32, (usize, usize), f32, i32)>, aggression: f32, coastlines: Vec<(usize, usize)>) {//I call this 'push' because the cell is reading data from neighbors and pushing a decision
        let mut max_enemy_strength = 0.0;
        let mut max_need = 0.0;
        let mut max_need_position = self.position;
        let mut min_enemy_strength = 0.0;
        let mut min_enemy_position = self.position;
        self.boat_need *= 0.9;
        self.need = (self.boat_need as f32).sqrt();
        self.send_target = self.position;
        self.send_amount = 0.0;
        self.send_empire = self.empire;
        self.boat_strength = 0.0;

        if self.empire == -1 {
            return;
        } else {
            self.boat_need += (coastlines.len() as f32)/self.age as f32;
        }

        for i in 0..data.len() {
            if let Some(neighbor_cell) = data.get(i) {
                if neighbor_cell.0 == self.position {
                    continue;
                }
                if self.empire == neighbor_cell.1 {
                   if neighbor_cell.3 > max_need {
                        max_need = neighbor_cell.3;
                        max_need_position = neighbor_cell.0;
                    }
                } else {
                    if neighbor_cell.2 > max_enemy_strength {
                        max_enemy_strength = neighbor_cell.2;
                    }
                    if neighbor_cell.2 < min_enemy_strength || min_enemy_position == self.position {
                        min_enemy_strength = neighbor_cell.2;
                        min_enemy_position = neighbor_cell.0;
                    }
                    if neighbor_cell.1 != self.empire {
                        self.need += 1.0;
                    }
                    if neighbor_cell.1 == -1 {
                        self.need -= 0.9;
                    } else if neighbor_cell.2 > self.strength {
                        self.need += 3.0;
                    }
                }
            }
        }
       let extra = self.strength - max_enemy_strength / 3.0;
        if extra > 0.0 {
            if extra > (3.0 * (1.0 - aggression)) * min_enemy_strength && min_enemy_position != self.position {
                self.send_target = min_enemy_position;
                self.send_amount = extra;
            } else if max_need > 0.0 && max_need_position != self.position{
                (self.send_target.0, self.send_target.1) = (max_need_position.0, max_need_position.1);
                self.send_amount = extra * 0.5;
            }
        }
        self.need += max_need * 0.999999;
        self.strength -= self.send_amount;
        if coastlines.len() > 0 && self.boat_need > 1.0 && self.strength > 1.0 / BOAT_PROP {
            self.boat_target = coastlines[rand::thread_rng().gen_range(0..coastlines.len())];
            self.boat_strength = self.strength * self.ocean_need_prop;
            self.boat_strength = self.boat_strength.max(self.strength);
            self.strength -= self.boat_strength;
            //println!("Attempting to launch boat from ({}, {}) with strength {}", self.position.0, self.position.1, self.boat_strength);
        }
    }

    fn pull(&mut self, data: Vec<((usize, usize), i32, f32, f32, (usize, usize), f32, i32)>, tech: f32, boat_attacks: f32) {//I call this 'pull' because the cell is pulling the decisions from other cells to update its own data
        // Check the send_ variables of all neighbors to see if they are sending strength to this cell
        //self.empire = grid_data.0;
        //self.strength = grid_data.1;

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
                if neighbor_cell.6 != self.empire && neighbor_cell.4 == self.position && neighbor_cell.1 != -1 {
                    //println!("Empire {} is attacking cell ({}, {}) from ({}, {})", neighbor_cell.6, self.position.0, self.position.1, neighbor_cell.0.0, neighbor_cell.0.1);
                    if self.strength - neighbor_cell.5 / 3.0 < 0.0 {
                        self.age = 0;
                        self.empire = neighbor_cell.6;
                        //println!("Empire {} has taken cell ({}, {})", self.empire, self.position.0, self.position.1);
                        self.strength = neighbor_cell.5 / 3.0 - self.strength;
                    } else {
                        self.strength -= neighbor_cell.5 / 3.0;
                    }
                }
            }
        }
        if self.empire != -1 {
            // Use terrain data from the grid to determine how much strength this cell should generate. The closer to ocean level, the more strength is made.
            let terrain_factor = ((1.0 - ((OCEAN_CUTOFF - self.terrain).abs() / (1.0 - OCEAN_CUTOFF))) * TERRAIN_IMPORTANCE + (1.0 - TERRAIN_IMPORTANCE) + tech).min(1.0);
            self.strength += terrain_factor;
            // Multiply strength by 0.99 so it can't just go up forever.
            self.strength *= terrain_factor;
            self.need *= 1.0 - (self.terrain * TERRAIN_IMPORTANCE / 1.3);
            self.boat_need += boat_attacks;
            self.age += 1;
        }
    }
}

fn push_system(mut query: Query<&mut Cell>, cell_map: Res<MapData>) {
    //println!("Pushing");

    //track start time of push
    //let start = Instant::now();

    query.par_iter_mut().for_each(|mut cell| {//iterate through all cells on many threads
        let position = cell.position;//get cell's position
        let mut data = Vec::new();//initialize data to be sent to cell.push
        let mut ocean = Vec::new();
        for i in 0..9 {//iterate through the 8 possible neighbor positions
            let mut neighbor_x:i32 = (position.0 as i32) + i % 3 - 1;
            let neighbor_y:i32 = (position.1 as i32) + i / 3 - 1;
            if neighbor_x < 0 {
                neighbor_x += WIDTH as i32;
            }
            if neighbor_x >= WIDTH as i32 {
                neighbor_x -= WIDTH as i32;
            }
            if (neighbor_y as usize) >= HEIGHT || neighbor_y < 0 {
                continue;
            }
            if cell.position == (neighbor_x as usize, neighbor_y as usize) {
                continue;
            }
            if let Some(neighbor) = cell_map.0.get(&(neighbor_x as usize, neighbor_y as usize)).clone() {
                data.push((neighbor.0, neighbor.1, neighbor.2, neighbor.3, neighbor.4, neighbor.5, neighbor.6));
            } else {
                //neighbor is in the map but isn't in the hashmap, so it's ocean
                ocean.push((neighbor_x as usize, neighbor_y as usize));
            }
        }
        let mut aggression = 0.0;
        if cell.empire != -1 {
            aggression = cell_map.1[cell.empire as usize].2;
        }
        //println!("Pushed {} neighbors to cell at ({}, {})", data.len(), position.0, position.1);
        cell.push(data, aggression, ocean);
    });

    //print time duration of push
    //println!("Push took {:?}", start.elapsed());

    //println!("Pushed");
}

//iterate through all cells, run the get() function on them to update CellMap
fn update_cell_map_system(mut commands: Commands, mut cell_map: ResMut<MapData>, mut game_data: ResMut<GameData>, query: Query<&Cell>) {
    let mut max_strength = 0.0;
    let mut max_age = 0;
    //track start time of update
    //let start = Instant::now();
    //println!("Updating");
    query.iter().for_each(|cell| {
        if game_data.send_boats && cell.empire != -1 && cell.boat_strength > 0.0 {
            let spawn_location = cell.boat_target;
            let mut direction = (
                (spawn_location.0 as i32 - cell.position.0 as i32) as f32,
                (spawn_location.1 as i32 - cell.position.1 as i32) as f32
            );
            if direction == (1.0, 1.0) {
                direction = (0.5, 0.5);
            }
            if direction.0 > direction.1 {
                let a = (rand::thread_rng().gen_range(0..500) as f32 / 1000.0 - 0.25) * direction.0;
                direction.0 -= a;
                direction.1 += a;
            } else {
                let a = (rand::thread_rng().gen_range(0..500) as f32 / 1000.0 - 0.25) * direction.1;
                direction.1 -= a;
                direction.0 += a;
            }
            let boat = Boat {
                direction,
                strength: cell.boat_strength,
                empire: cell.empire,
            };
            commands.spawn(SpriteBundle {
                sprite: Sprite {
                    color: Color::hsla(cell_map.1[cell.empire as usize].0, cell_map.1[cell.empire as usize].1, 0.5, 1.0),
                    custom_size: Some(Vec2::new(1.0, 1.0)),
                    ..Default::default()
                },
                transform: Transform::from_xyz(spawn_location.0 as f32, spawn_location.1 as f32, 1.0),
                ..Default::default()
            }).insert(boat);
        }
        cell_map.0.insert(cell.position, cell.get());
        if cell.strength > max_strength {
            max_strength = cell.strength;
        }
        if cell.age > max_age {
            max_age = cell.age;
        }
    });
    game_data.max_age = max_age;
    game_data.max_strength = max_strength;
    game_data.send_boats = !game_data.send_boats;

    //print time duration of update
    //println!("Update took {:?}", start.elapsed());
}

fn update_boats_system(mut commands: Commands, mut query: Query<(Entity, &mut Boat, &mut Transform)>, mut grid: ResMut<MapData>) {
    query.iter_mut().for_each(|(entity, mut boat, mut transform)| {
        let mut position = boat.move_boat((transform.translation.x as i32, transform.translation.y as i32));
        if position.1 >= HEIGHT {
            boat.direction.1 *= -1.0;
            position = boat.move_boat((transform.translation.x as i32, transform.translation.y as i32));
        }
        //check if we've hit land
        if let Some(cell) = grid.0.get_mut(&(position.0 as usize, position.1 as usize)) {
            //add boat empire and strength to the vec at the end of the cell data
            cell.8.insert(position, (boat.empire, boat.strength));
            //println!("Boat has arrived at ({}, {})", position.0, position.1);
            //remove the boat
            commands.entity(entity).despawn();
            //let count = grid.0.get(&(position.0 as usize, position.1 as usize)).unwrap().8.len();
            //println!("This cell has recieved {} boats", count);
        } else {
            transform.translation = Vec3::new(position.0 as f32, position.1 as f32, 0.0);
        }
        if transform.translation.x < 0.0 || transform.translation.x >= WIDTH as f32 {//loop around the world
            transform.translation.x = (WIDTH as f32 - 1.0) - transform.translation.x.abs();
        }
    });
}

fn pull_system(mut query: Query<&mut Cell>, cell_map: Res<MapData>) {
    //println!("Pulling");

    //track start time of pull
    //let start = Instant::now();

    query.par_iter_mut().for_each(|mut cell| {//iterate through all cells on many threads
        let position = cell.position;//get cell's position
        let mut data = Vec::new();//initialize data to be sent to cell.push
        let boat_data = cell_map.0.get(&(position.0, position.1)).unwrap().8.clone();
        let mut boat_attacks = 0.0;
        for i in 0..9 {//iterate through the 8 possible neighbor positions
            let mut neighbor_x:i32 = (position.0 as i32) + i % 3 - 1;
            let neighbor_y:i32 = (position.1 as i32) + i / 3 - 1;
            if neighbor_x < 0 {
                neighbor_x += WIDTH as i32;
            }
            if neighbor_x >= WIDTH as i32 {
                neighbor_x -= WIDTH as i32;
            }
            if (neighbor_y as usize) >= HEIGHT || neighbor_y < 0 {
                continue;
            }
            if cell.position == (neighbor_x as usize, neighbor_y as usize) {
                continue;
            }
            if let Some(neighbor) = cell_map.0.get(&(neighbor_x as usize, neighbor_y as usize)) {
                data.push((neighbor.0, neighbor.1, neighbor.2, neighbor.3, neighbor.4, neighbor.5, neighbor.6));
            }
        }
        for (position, boat) in boat_data.iter() {
            data.push((*position, boat.0, boat.1, 0.0, *position, boat.1, boat.0));
            if boat.0 != cell.empire {
                boat_attacks += boat.1;
            }
            //println!("Added boat to data for cell at ({}, {})", position.0, position.1);
        }
        if boat_data.len() > 0 {
            //println!("This cell has recieved {} boats", boat_data.len());
        }
        let mut tech = 0.0;
        if cell.empire != -1 {
            tech = cell_map.1[cell.empire as usize].3;
        }
        cell.pull(data, tech, boat_attacks);
    });

    //print time duration of pull
    //println!("Pull took {:?}", start.elapsed());
}

fn update_empires(mut cell_map: ResMut<MapData>) {
    //iterate through all empires in cell_map.1, give each a small chance to have a slight boost to tech factor
    for i in 0..cell_map.1.len() {
        if rand::thread_rng().gen_range(0..1000000000) < 1 {
            cell_map.1[i].3 += 0.000000000001;
        }
    }
}

#[derive(Resource)]
enum RenderMode {
    StrengthView,
    EmpireView,
    TerrainView,
    NeedView,
    SendView,
    AgeView,
    BoatNeedView,
    // Add more render modes here
}

fn update_render_mode_system(keyboard_input: Res<ButtonInput<KeyCode>>, mut render_mode: ResMut<RenderMode>) {
    if keyboard_input.just_pressed(KeyCode::Digit1) {
        *render_mode = RenderMode::EmpireView;
    } else if keyboard_input.just_pressed(KeyCode::Digit2) {
        *render_mode = RenderMode::StrengthView;
    } else if keyboard_input.just_pressed(KeyCode::Digit3) {
        *render_mode = RenderMode::NeedView;
    } else if keyboard_input.just_pressed(KeyCode::Digit4) {
        *render_mode = RenderMode::TerrainView;
    } else if keyboard_input.just_pressed(KeyCode::Digit5) {
        *render_mode = RenderMode::SendView;
    } else if keyboard_input.just_pressed(KeyCode::Digit6) {
        *render_mode = RenderMode::AgeView;
    } else if keyboard_input.just_pressed(KeyCode::Digit7) {
        *render_mode = RenderMode::BoatNeedView;
    }
}



fn update_colors(
    grid: Res<Grid>,
    cell_map: Res<MapData>,
    render_mode: Res<RenderMode>,
    game_data: Res<GameData>,
    mut query: Query<(&Transform, &mut Sprite, Option<&CellMarker>)>,
) {
    // Collect query results into a vector
    //let start = Instant::now();
    let mut query_results: Vec<(&Transform, Mut<Sprite>, Option<&CellMarker>)> = query.iter_mut().collect();
    let max_strength: f32 = game_data.max_strength;

    // Use Rayon to iterate over the vector in parallel
    query_results.par_iter_mut().for_each(|(transform,ref mut sprite, cell_marker)| {
        let x = transform.translation.x as usize;
        let y = transform.translation.y as usize;
        if !(x >= WIDTH || y >= HEIGHT) && cell_marker.is_some() {
            //check if cell_marker is Some, which means it's not a boat.

            let terrain = &grid.data[x][y];
            let max_age = game_data.max_age as f32;

            //some grid spots don't have cells because they are ocean
            //check if a cell exists at this position before trying to access it
            //let cell = cell_map.0.get(&(x, y)).unwrap_or(&((0, 0), -1, 0.0, 0.0, (0, 0), 0.0, -1));

            let cell = match cell_map.0.get(&(x, y)) {
                Some(cell) => cell,
                None => &((0, 0), -1, 0.0, 0.0, (0, 0), 0.0, -1, 0, HashMap::new(), 0.0),
            };
            let color = if matches!(*render_mode, RenderMode::TerrainView) || cell.1 == -1 {
                if terrain[0] < OCEAN_CUTOFF {
                    //ocean
                    let brightness = terrain[0] / 2.0 + OCEAN_CUTOFF / 2.0;//cell[0] + 0.01 / (cell[0].sqrt());
                    Color::hsla(240.0, 1.0, brightness, 1.0)
                } else {
                    //land
                    let brightness = terrain[0] / 3.0 + OCEAN_CUTOFF / 3.0;
                    Color::hsla(100.0 + (terrain[0] - 0.5) * 30.0 * (1.0 / OCEAN_CUTOFF), 1.0 - (terrain[0] - OCEAN_CUTOFF).abs() + OCEAN_CUTOFF, brightness, 1.0)
                }
            } else {
                //println!("Empire {} has strength {} and need {} at ({}, {})", cell.1, cell.2, cell.3, x, y);
                let e_hue = cell_map.1[cell.1 as usize].0;
                let e_sat = cell_map.1[cell.1 as usize].1;
                match *render_mode {
                    RenderMode::StrengthView => {
                        let brightness = ((cell.2 as f32 / max_strength) + cell.2 as f32 / 100.0) / 2.0;
                        Color::hsla(e_hue, e_sat, brightness, 1.0)
                    }
                    RenderMode::EmpireView => {
                        Color::hsla(e_hue, e_sat, terrain[0]/2.0 + 0.25, 1.0)
                    }
                    RenderMode::NeedView => {
                        let brightness = cell.3.sqrt() / 48.0;
                        Color::hsla(e_hue, e_sat, brightness, 1.0)
                    }
                    RenderMode::SendView => {
                        let lr = cell.4.0 - cell.0.0;
                        let ud = cell.4.1 - cell.0.1;
                        //lr and ud together are a vector of the direction the strength is being sent
                        //hue should be the angle of the direction of that vector
                        let mut angle = (ud as f32).atan2(lr as f32).to_degrees();
                        if angle < 0.0 {
                            angle += 360.0;
                        }
                        let hue = angle;
                        let brightness = ((cell.5 as f32 / max_strength) + cell.5 as f32 / 100.0) / 2.0;
                        Color::hsla(hue, 1.0, brightness, 1.0)
                    }
                    RenderMode::AgeView => {
                        let brightness = (cell.7 as f32 / max_age).min(0.5);
                        Color::hsla(e_hue, e_sat, brightness, 1.0)
                    }
                    RenderMode::BoatNeedView => {
                        let brightness = cell.9 as f32 / 48.0;
                        Color::hsla(e_hue, e_sat, brightness, 1.0)
                    }
                    _ => Color::WHITE,
                }
            };

            sprite.color = color;
        }
    });
    //println!("Render took {:?}", start.elapsed());
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