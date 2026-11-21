use macroquad::prelude::*;

const LANE_W: f32 = 2.2;
const GRAVITY: f32 = 32.0;
const JUMP_V: f32 = 11.5;
const ROLL_TIME: f32 = 0.65;

fn lane_x(l: i32) -> f32 {
    (l - 1) as f32 * LANE_W
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Low,   // barreira baixa: pular
    High,  // barra alta: rolar
    Train, // vagão: trocar de faixa
}

struct Obstacle {
    lane: i32,
    z: f32,
    kind: Kind,
}

impl Obstacle {
    fn len(&self) -> f32 {
        match self.kind {
            Kind::Train => 9.0,
            _ => 0.8,
        }
    }
}

struct Coin {
    lane: i32,
    z: f32,
    y: f32,
    taken: bool,
}

struct Game {
    // jogador
    lane: i32,
    x: f32,
    y: f32,
    vy: f32,
    roll_t: f32,
    z: f32, // posição ao longo da pista (diminui)
    speed: f32,
    run_t: f32,
    // mundo
    obstacles: Vec<Obstacle>,
    coins: Vec<Coin>,
    next_z: f32,
    // estado
    dead: bool,
    coins_got: u32,
    dist: f32,
    best: u32,
    swipe_start: Option<Vec2>,
}

impl Game {
    fn new(best: u32) -> Self {
        let mut g = Game {
            lane: 1,
            x: 0.0,
            y: 0.0,
            vy: 0.0,
            roll_t: 0.0,
            z: 0.0,
            speed: 14.0,
            run_t: 0.0,
            obstacles: vec![],
            coins: vec![],
            next_z: -35.0,
            dead: false,
            coins_got: 0,
            dist: 0.0,
            best,
            swipe_start: None,
        };
        g.fill_world();
        g
    }

    fn score(&self) -> u32 {
        (self.dist as u32) + self.coins_got * 10
    }

    fn height(&self) -> f32 {
        if self.roll_t > 0.0 {
            0.8
        } else {
            1.7
        }
    }

    fn fill_world(&mut self) {
        while self.next_z > self.z - 110.0 {
            self.spawn_row();
        }
    }

    fn coin_line(&mut self, lane: i32, start: f32, n: i32) {
        for i in 0..n {
            self.coins.push(Coin {
                lane,
                z: start - i as f32 * 1.6,
                y: 0.9,
                taken: false,
            });
        }
    }

    fn spawn_row(&mut self) {
        let s = self.next_z;
        let mut used = 2.0;
        match rand::gen_range(0, 5) {
            0 => {
                let lane = rand::gen_range(0, 3);
                let kind = if rand::gen_range(0, 2) == 0 { Kind::Low } else { Kind::High };
                self.obstacles.push(Obstacle { lane, z: s, kind });
                let other = (lane + 1 + rand::gen_range(0, 2)) % 3;
                self.coin_line(other, s, 5);
            }
            1 => {
                let free = rand::gen_range(0, 3);
                for l in 0..3 {
                    if l != free {
                        self.obstacles.push(Obstacle { lane: l, z: s - 4.5, kind: Kind::Train });
                    }
                }
                self.coin_line(free, s, 6);
                used = 9.0;
            }
            2 => {
                for l in 0..3 {
                    self.obstacles.push(Obstacle { lane: l, z: s, kind: Kind::Low });
                }
            }
            3 => {
                let lane = rand::gen_range(0, 3);
                self.obstacles.push(Obstacle { lane, z: s, kind: Kind::Low });
                let n = 9;
                for i in 0..n {
                    let t = i as f32 / (n - 1) as f32;
                    self.coins.push(Coin {
                        lane,
                        z: s + 4.0 - i as f32,
                        y: 0.9 + 1.5 * (std::f32::consts::PI * t).sin(),
                        taken: false,
                    });
                }
                used = 5.0;
            }
            _ => {
                let open = rand::gen_range(0, 3);
                for l in 0..3 {
                    if l != open {
                        self.obstacles.push(Obstacle { lane: l, z: s, kind: Kind::High });
                    }
                }
                self.coin_line(open, s, 4);
            }
        }
        self.next_z = s - used - rand::gen_range(14.0, 24.0);
    }

    fn jump(&mut self) {
        if self.y <= 0.001 {
            self.vy = JUMP_V;
            self.roll_t = 0.0;
        }
    }

    fn roll(&mut self) {
        self.roll_t = ROLL_TIME;
        if self.y > 0.001 {
            self.vy = -22.0; // queda rápida
        }
    }

    fn handle_input(&mut self) {
        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
            self.lane = (self.lane - 1).max(0);
        }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
            self.lane = (self.lane + 1).min(2);
        }
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Space) {
            self.jump();
        }
        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
            self.roll();
        }

        // deslizar na tela (celular)
        for t in touches() {
            match t.phase {
                TouchPhase::Started => self.swipe_start = Some(t.position),
                TouchPhase::Ended => {
                    if let Some(s) = self.swipe_start.take() {
                        let d = t.position - s;
                        if d.length() > 30.0 {
                            if d.x.abs() > d.y.abs() {
                                if d.x > 0.0 {
                                    self.lane = (self.lane + 1).min(2);
                                } else {
                                    self.lane = (self.lane - 1).max(0);
                                }
                            } else if d.y < 0.0 {
                                self.jump();
                            } else {
                                self.roll();
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn update(&mut self, dt: f32) {
        if self.dead {
            return;
        }
        self.handle_input();

        // velocidade crescente
        self.speed = (14.0 + self.dist * 0.004).min(40.0);
        self.z -= self.speed * dt;
        self.dist += self.speed * dt;
        self.run_t += dt * self.speed * 0.5;

        // troca de faixa suave
        let tx = lane_x(self.lane);
        self.x += (tx - self.x) * (1.0 - (-18.0 * dt).exp());

        // física vertical
        if self.y > 0.0 || self.vy > 0.0 {
            self.vy -= GRAVITY * dt;
            self.y += self.vy * dt;
            if self.y <= 0.0 {
                self.y = 0.0;
                self.vy = 0.0;
            }
        }
        if self.roll_t > 0.0 {
            self.roll_t -= dt;
        }

        self.fill_world();
        self.obstacles.retain(|o| o.z - o.len() / 2.0 < self.z + 15.0);
        self.coins.retain(|c| c.z < self.z + 15.0 && !c.taken);

        // colisões
        let h = self.height();
        let top = self.y + h;
        let px = self.x;
        let pz = self.z;
        let py = self.y;
        let mut crashed = false;
        for o in &self.obstacles {
            if (px - lane_x(o.lane)).abs() > 0.85 {
                continue;
            }
            if (o.z - pz).abs() > o.len() / 2.0 + 0.3 {
                continue;
            }
            let hit = match o.kind {
                Kind::Low => py < 1.0,
                Kind::High => top > 1.1,
                Kind::Train => py < 3.0,
            };
            if hit {
                crashed = true;
                break;
            }
        }
        if crashed {
            self.dead = true;
            self.best = self.best.max(self.score());
        }

        // moedas
        let mut got = 0;
        for c in self.coins.iter_mut() {
            if (px - lane_x(c.lane)).abs() < 0.9
                && (c.z - pz).abs() < 0.8
                && c.y > py - 0.3
                && c.y < top + 0.3
            {
                c.taken = true;
                got += 1;
            }
        }
        self.coins_got += got;
    }

    // ---------- desenho ----------

    fn draw(&self) {
        clear_background(Color::new(0.55, 0.78, 0.95, 1.0));
        set_default_camera();
        let sw = screen_width();
        let sh = screen_height();
        let bands = 24;
        for i in 0..bands {
            let t = i as f32 / bands as f32;
            draw_rectangle(
                0.0,
                t * sh,
                sw,
                sh / bands as f32 + 1.0,
                Color::new(0.05 + 0.75 * t, 0.02 + 0.2 * t, 0.2 + 0.4 * t, 1.0),
            );
        }

        let cam = Camera3D {
            position: vec3(self.x * 0.55, 4.0 + self.y * 0.35, self.z + 6.5),
            target: vec3(self.x * 0.45, 1.6 + self.y * 0.25, self.z - 10.0),
            up: vec3((lane_x(self.lane) - self.x) * 0.03, 1.0, 0.0),
            fovy: 55.0f32.to_radians(),
            ..Default::default()
        };
        set_camera(&cam);

        self.draw_world();
        self.draw_obstacles();
        self.draw_coins();
        self.draw_player();

        set_default_camera();
        self.draw_hud();
    }

    fn draw_world(&self) {
        let road_w = 3.0 * LANE_W;
        // base da pista
        draw_cube(
            vec3(0.0, -0.25, self.z - 50.0),
            vec3(road_w, 0.5, 200.0),
            None,
            Color::new(0.1, 0.1, 0.16, 1.0),
        );
        // listras para dar sensação de movimento
        let base = (self.z / 4.0).floor() * 4.0;
        for i in -4..32 {
            let zz = base - i as f32 * 4.0;
            let idx = (zz / 4.0) as i32;
            let c = if idx.rem_euclid(2) == 0 {
                Color::new(0.16, 0.14, 0.24, 1.0)
            } else {
                Color::new(0.12, 0.11, 0.19, 1.0)
            };
            draw_cube(vec3(0.0, 0.01, zz), vec3(road_w, 0.02, 3.9), None, c);
        }
        // divisórias de faixa
        for sx in [-LANE_W / 2.0, LANE_W / 2.0] {
            draw_cube(
                vec3(sx, 0.03, self.z - 50.0),
                vec3(0.08, 0.02, 200.0),
                None,
                Color::new(0.2, 0.95, 1.0, 1.0),
            );
        }
        // calçadas
        for sx in [-road_w / 2.0 - 1.5, road_w / 2.0 + 1.5] {
            draw_cube(
                vec3(sx, 0.1, self.z - 50.0),
                vec3(3.0, 0.2, 200.0),
                None,
                Color::new(0.2, 0.18, 0.3, 1.0),
            );
        }
        // prédios dos lados
        let base_i = (self.z / 6.0).floor() as i32;
        for i in -4..24 {
            let idx = base_i - i;
            let zz = idx as f32 * 6.0;
            for (side, sx) in [(0, -10.0f32), (1, 10.0f32)] {
                let seed = idx.wrapping_mul(7919).wrapping_add(side * 104729);
                let h = 5.0 + seed.rem_euclid(12) as f32;
                let tint = seed.rem_euclid(5) as f32 * 0.05;
                draw_cube(
                    vec3(sx, h / 2.0, zz),
                    vec3(5.0, h, 5.2),
                    None,
                    Color::new(0.12 + tint, 0.1 + tint * 0.5, 0.25 + tint, 1.0),
                );
                let fx = if side == 0 { sx + 2.52 } else { sx - 2.52 };
                for wy in (1..h as i32).step_by(2) {
                    for wz in -1..=1 {
                        let lit = (seed + wy * 3 + wz * 5).rem_euclid(3) != 0;
                        let col = if lit {
                            Color::new(1.0, 0.9, 0.5, 1.0)
                        } else {
                            Color::new(0.2, 0.25, 0.4, 1.0)
                        };
                        draw_cube(
                            vec3(fx, wy as f32 + 0.5, zz + wz as f32 * 1.5),
                            vec3(0.05, 1.0, 0.9),
                            None,
                            col,
                        );
                    }
                }
            }
        }
    }

    fn draw_obstacles(&self) {
        for o in &self.obstacles {
            let x = lane_x(o.lane);
            match o.kind {
                Kind::Low => {
                    draw_cube(vec3(x, 0.5, o.z), vec3(1.8, 1.0, 0.8), None, Color::new(1.0, 0.25, 0.7, 1.0));
                    draw_cube(vec3(x, 0.75, o.z), vec3(1.82, 0.25, 0.82), None, Color::new(0.2, 0.95, 1.0, 1.0));
                    draw_cube_wires(vec3(x, 0.5, o.z), vec3(1.8, 1.0, 0.8), BLACK);
                }
                Kind::High => {
                    draw_cube(vec3(x - 0.9, 1.25, o.z), vec3(0.18, 2.5, 0.3), None, GRAY);
                    draw_cube(vec3(x + 0.9, 1.25, o.z), vec3(0.18, 2.5, 0.3), None, GRAY);
                    draw_cube(vec3(x, 1.85, o.z), vec3(2.0, 1.3, 0.5), None, RED);
                    draw_cube_wires(vec3(x, 1.85, o.z), vec3(2.0, 1.3, 0.5), BLACK);
                }
                Kind::Train => {
                    let len = o.len();
                    draw_cube(vec3(x, 1.5, o.z), vec3(1.9, 3.0, len), None, Color::new(0.45, 0.2, 0.85, 1.0));
                    draw_cube(vec3(x, 3.02, o.z), vec3(1.7, 0.1, len - 0.4), None, LIGHTGRAY);
                    draw_cube(vec3(x, 0.6, o.z), vec3(1.93, 0.3, len), None, YELLOW);
                    for k in 0..4 {
                        let wz = o.z - len / 2.0 + 1.2 + k as f32 * 2.2;
                        draw_cube(vec3(x - 0.96, 2.0, wz), vec3(0.04, 0.8, 1.2), None, SKYBLUE);
                        draw_cube(vec3(x + 0.96, 2.0, wz), vec3(0.04, 0.8, 1.2), None, SKYBLUE);
                    }
                    draw_cube(vec3(x - 0.6, 0.9, o.z + len / 2.0 + 0.02), vec3(0.3, 0.3, 0.05), None, Color::new(1.0, 1.0, 0.7, 1.0));
                    draw_cube(vec3(x + 0.6, 0.9, o.z + len / 2.0 + 0.02), vec3(0.3, 0.3, 0.05), None, Color::new(1.0, 1.0, 0.7, 1.0));
                    draw_cube(vec3(x, 1.7, o.z + len / 2.0 - 0.02), vec3(1.2, 1.0, 0.05), None, Color::new(0.8, 0.9, 1.0, 1.0));
                    draw_cube_wires(vec3(x, 1.5, o.z), vec3(1.9, 3.0, len), BLACK);
                }
            }
        }
    }

    fn draw_coins(&self) {
        let spin = get_time() as f32 * 4.0;
        for c in &self.coins {
            let x = lane_x(c.lane);
            let w = 0.12 + 0.35 * spin.sin().abs();
            draw_cube(vec3(x, c.y, c.z), vec3(w, 0.5, 0.5), None, GOLD);
            draw_cube(vec3(x, c.y, c.z), vec3(w * 1.1, 0.3, 0.3), None, YELLOW);
        }
    }

    fn draw_player(&self) {
        let x = self.x;
        let z = self.z;
        // sombra
        draw_cube(vec3(x, 0.03, z), vec3(0.9, 0.02, 0.9), None, Color::new(0.0, 0.0, 0.0, 0.5));

        let body = Color::new(0.75, 0.78, 0.85, 1.0);
        let accent = Color::new(0.2, 0.95, 1.0, 1.0);

        if self.roll_t > 0.0 {
            // rolando: bolinha compacta
            draw_cube(vec3(x, self.y + 0.4, z), vec3(0.8, 0.8, 0.8), None, body);
            draw_cube(vec3(x, self.y + 0.4, z - 0.41), vec3(0.5, 0.3, 0.05), None, accent);
            return;
        }

        let in_air = self.y > 0.01;
        let swing = (self.run_t * 2.0).sin() * 0.4;
        let s = if in_air { 0.3 } else { swing };
        let bob = if in_air { 0.0 } else { (self.run_t * 4.0).sin().abs() * 0.06 };
        let lean = (lane_x(self.lane) - self.x) * 0.18;
        let by = self.y + bob;
        let arm = if in_air { 0.25 } else { 0.0 };
        // pernas
        draw_cube(vec3(x - 0.17, by + 0.4, z + s), vec3(0.22, 0.8, 0.25), None, DARKBLUE);
        draw_cube(vec3(x + 0.17, by + 0.4, z - s), vec3(0.22, 0.8, 0.25), None, DARKBLUE);
        // tronco
        draw_cube(vec3(x + lean * 0.5, by + 1.1, z), vec3(0.7, 0.8, 0.4), None, body);
        draw_cube(vec3(x + lean * 0.5, by + 1.2, z - 0.21), vec3(0.4, 0.15, 0.03), None, accent);
        // braços
        draw_cube(vec3(x - 0.45 + lean * 0.5, by + 1.1 + arm, z - s), vec3(0.18, 0.7, 0.2), None, body);
        draw_cube(vec3(x + 0.45 + lean * 0.5, by + 1.1 + arm, z + s), vec3(0.18, 0.7, 0.2), None, body);
        // cabeça
        draw_cube(vec3(x + lean, by + 1.65, z), vec3(0.55, 0.45, 0.45), None, Color::new(0.55, 0.58, 0.65, 1.0));
        draw_cube(vec3(x + lean, by + 1.68, z - 0.23), vec3(0.4, 0.14, 0.03), None, accent);
        draw_cube(vec3(x + lean, by + 1.98, z), vec3(0.05, 0.25, 0.05), None, GRAY);
        draw_sphere(vec3(x + lean, by + 2.12, z), 0.07, None, accent);
    }

    fn draw_hud(&self) {
        draw_text(&format!("Pontos: {}", self.score()), 20.0, 40.0, 40.0, WHITE);
        draw_text(&format!("Moedas: {}", self.coins_got), 20.0, 80.0, 32.0, GOLD);
        draw_text(&format!("Recorde: {}", self.best.max(self.score())), 20.0, 115.0, 28.0, LIGHTGRAY);

        if self.dead {
            let w = screen_width();
            let h = screen_height();
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
            let t = "GAME OVER";
            let d = measure_text(t, None, 80, 1.0);
            draw_text(t, (w - d.width) / 2.0, h / 2.0 - 20.0, 80.0, RED);
            let t2 = format!("Pontos: {}   Recorde: {}", self.score(), self.best);
            let d2 = measure_text(&t2, None, 36, 1.0);
            draw_text(&t2, (w - d2.width) / 2.0, h / 2.0 + 30.0, 36.0, WHITE);
            let t3 = "Espaço / Enter / toque para recomeçar";
            let d3 = measure_text(t3, None, 28, 1.0);
            draw_text(t3, (w - d3.width) / 2.0, h / 2.0 + 75.0, 28.0, LIGHTGRAY);
        }
    }
}

fn conf() -> Conf {
    Conf {
        window_title: "Neon Dash".to_owned(),
        window_width: 480,
        window_height: 800,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

#[macroquad::main(conf)]
async fn main() {
    rand::srand(macroquad::miniquad::date::now() as u64);
    let mut game = Game::new(0);

    loop {
        let dt = get_frame_time().min(0.05);

        if game.dead {
            let tapped = touches().iter().any(|t| t.phase == TouchPhase::Started);
            if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) || tapped {
                let best = game.best;
                game = Game::new(best);
            }
        }

        game.update(dt);
        game.draw();
        next_frame().await;
    }
}
