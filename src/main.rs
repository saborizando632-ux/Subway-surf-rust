use macroquad::prelude::*;

const LANE_W: f32 = 2.2;
const GRAVITY: f32 = 32.0;
const JUMP_V: f32 = 11.5;
const ROLL_TIME: f32 = 0.65;
const COP_TIME: f32 = 15.0;

// Velocidade: começa em START_SPEED e sobe SPEED_GAIN por segundo, até MAX_SPEED
const START_SPEED: f32 = 14.0;
const SPEED_GAIN: f32 = 0.15;
const MAX_SPEED: f32 = 45.0;

fn lane_x(l: i32) -> f32 {
    (l - 1) as f32 * LANE_W
}

// ---------- texturas procedurais ----------

fn hash(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x
        .wrapping_mul(374761393)
        .wrapping_add(y.wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(2147483647));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0xffff) as f32 / 65535.0
}

fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let xf0 = x.floor();
    let yf0 = y.floor();
    let fx = x - xf0;
    let fy = y - yf0;
    let xi = xf0 as i32 as u32;
    let yi = yf0 as i32 as u32;
    let a = hash(xi, yi, seed);
    let b = hash(xi.wrapping_add(1), yi, seed);
    let c = hash(xi, yi.wrapping_add(1), seed);
    let d = hash(xi.wrapping_add(1), yi.wrapping_add(1), seed);
    let u = fx * fx * (3.0 - 2.0 * fx);
    let v = fy * fy * (3.0 - 2.0 * fy);
    a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

fn make_tex<F: Fn(u32, u32) -> Color>(w: u32, h: u32, f: F) -> Texture2D {
    let mut img = Image::gen_image_color(w as u16, h as u16, BLACK);
    for y in 0..h {
        for x in 0..w {
            img.set_pixel(x, y, f(x, y));
        }
    }
    let t = Texture2D::from_image(&img);
    t.set_filter(FilterMode::Linear);
    t
}

struct Textures {
    asphalt: Texture2D,
    concrete: Texture2D,
    metal: Texture2D,
    pavement: Texture2D,
    hazard: Texture2D,
}

impl Textures {
    fn new() -> Self {
        // asfalto: granulado + pedrinhas claras
        let asphalt = make_tex(128, 128, |x, y| {
            let n = vnoise(x as f32 / 8.0, y as f32 / 8.0, 1) * 0.5
                + vnoise(x as f32 / 2.5, y as f32 / 2.5, 2) * 0.5;
            let mut v = 0.55 + n * 0.4;
            if hash(x, y, 3) > 0.985 {
                v += 0.25;
            }
            let v = v.min(1.0);
            Color::new(v, v, (v * 1.05).min(1.0), 1.0)
        });

        // concreto: manchas, escorrimento vertical e juntas de painel
        let concrete = make_tex(64, 128, |x, y| {
            let n = vnoise(x as f32 / 6.0, y as f32 / 6.0, 4) * 0.5
                + vnoise(x as f32 / 2.0, y as f32 / 2.0, 5) * 0.2;
            let streak = vnoise(x as f32 / 3.0, y as f32 / 45.0, 6) * 0.3;
            let mut v = 0.5 + n * 0.5 + streak * 0.4;
            if y % 32 == 0 {
                v *= 0.6;
            }
            let v = v.clamp(0.0, 1.0);
            Color::new(v, v, v, 1.0)
        });

        // metal escovado com nervuras e rebites
        let metal = make_tex(128, 64, |x, y| {
            let brushed = vnoise(x as f32 / 40.0, y as f32 * 1.5, 7) * 0.2;
            let mut v = 0.72 + brushed + hash(x, y, 8) * 0.05;
            if y % 16 == 0 {
                v *= 0.55;
            } else if y % 16 == 1 {
                v = (v * 1.2).min(1.0);
            }
            if x % 64 == 0 {
                v *= 0.5;
            }
            if (x % 64 == 6 || x % 64 == 58) && y % 16 == 8 {
                v = (v * 1.3).min(1.0);
            }
            let v = v.clamp(0.0, 1.0);
            Color::new(v, v, v, 1.0)
        });

        // calçada: placas com rejunte
        let pavement = make_tex(64, 64, |x, y| {
            let n = vnoise(x as f32 / 5.0, y as f32 / 5.0, 9) * 0.3 + hash(x, y, 10) * 0.1;
            let mut v = 0.7 + n;
            if x % 32 < 2 || y % 32 < 2 {
                v = 0.35 + n * 0.3;
            }
            let v = v.clamp(0.0, 1.0);
            Color::new(v, v, v, 1.0)
        });

        // faixas de alerta amarelo/preto, meio desgastadas
        let hazard = make_tex(64, 64, |x, y| {
            let wear = 0.8 + vnoise(x as f32 / 4.0, y as f32 / 4.0, 11) * 0.2;
            if ((x + y) / 10) % 2 == 0 {
                Color::new(0.95 * wear, 0.78 * wear, 0.08 * wear, 1.0)
            } else {
                Color::new(0.1 * wear, 0.1 * wear, 0.11 * wear, 1.0)
            }
        });

        Textures {
            asphalt,
            concrete,
            metal,
            pavement,
            hazard,
        }
    }
}

// névoa: objetos distantes se misturam com o horizonte
fn fog(c: Color, cam_z: f32, z: f32) -> Color {
    let d = (cam_z - z).max(0.0);
    let f = (d / 120.0).clamp(0.0, 1.0).powf(1.6);
    let (fr, fg, fb) = (0.32, 0.18, 0.5);
    Color::new(
        c.r + (fr - c.r) * f,
        c.g + (fg - c.g) * f,
        c.b + (fb - c.b) * f,
        c.a,
    )
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Low,   // barreira baixa: pular
    High,  // placa alta: rolar
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

struct Look {
    shirt: Color,
    pants: Color,
    shoes: Color,
    skin: Color,
    hair: Color,
    belt: Color,
    accent: Color,
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
    // policial
    cop_t: f32,
    cop_gap: f32,
    cop_x: f32,
    invuln: f32,
    // mundo
    obstacles: Vec<Obstacle>,
    coins: Vec<Coin>,
    next_z: f32,
    // estado
    dead: bool,
    coins_got: u32,
    dist: f32,
    best: u32,
    time: f32,       // tempo de corrida em segundos
    timer_pos: usize, // posição do cronômetro na tela
    swipe_start: Option<Vec2>,
}

impl Game {
    fn new(best: u32, timer_pos: usize) -> Self {
        let mut g = Game {
            lane: 1,
            x: 0.0,
            y: 0.0,
            vy: 0.0,
            roll_t: 0.0,
            z: 0.0,
            speed: START_SPEED,
            run_t: 0.0,
            cop_t: COP_TIME,
            cop_gap: 3.0,
            cop_x: 0.0,
            invuln: 0.0,
            obstacles: vec![],
            coins: vec![],
            next_z: -35.0,
            dead: false,
            coins_got: 0,
            dist: 0.0,
            best,
            time: 0.0,
            timer_pos,
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

    // ---------- cronômetro ----------

    fn timer_text(&self) -> String {
        let t = self.time;
        let h = (t / 3600.0) as u32;
        let m = ((t % 3600.0) / 60.0) as u32;
        let s = (t % 60.0) as u32;
        let cs = (t.fract() * 100.0) as u32;
        format!("{:02}:{:02}:{:02}.{:02}", h, m, s, cs)
    }

    fn timer_rect(&self) -> Rect {
        let d = measure_text("00:00:00.00", None, 34, 1.0);
        let w = d.width + 24.0;
        let h = 44.0;
        let sw = screen_width();
        let sh = screen_height();
        let m = 12.0;
        let (x, y) = match self.timer_pos % 4 {
            0 => ((sw - w) / 2.0, m),       // topo, centro
            1 => (sw - w - m, 68.0),        // topo, direita (abaixo do botão de tela cheia)
            2 => ((sw - w) / 2.0, sh - h - m), // baixo, centro
            _ => (m, sh - h - m),           // baixo, esquerda
        };
        Rect::new(x, y, w, h)
    }

    // toque/clique no cronômetro (ou tecla T) muda ele de lugar
    fn handle_timer_input(&mut self) {
        let r = self.timer_rect();
        let mut toggle = is_key_pressed(KeyCode::T);
        if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();
            if r.contains(vec2(mx, my)) {
                toggle = true;
            }
        }
        for t in touches() {
            if t.phase == TouchPhase::Started && r.contains(t.position) {
                toggle = true;
            }
        }
        if toggle {
            self.timer_pos = (self.timer_pos + 1) % 4;
        }
    }

    fn update(&mut self, dt: f32) {
        if self.dead {
            return;
        }
        self.handle_timer_input();
        self.handle_input();

        // posição anterior (para saber se a batida foi de frente ou de lado)
        let prev_x = self.x;
        let prev_z = self.z;

        // velocidade sobe devagarinho com o tempo
        self.time += dt;
        self.speed = (START_SPEED + self.time * SPEED_GAIN).min(MAX_SPEED);
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

        // policial
        if self.invuln > 0.0 {
            self.invuln -= dt;
        }
        if self.cop_t > 0.0 {
            self.cop_t -= dt;
        }
        let target_gap = if self.cop_t > 0.0 { 3.0 } else { 40.0 };
        self.cop_gap += (target_gap - self.cop_gap) * (1.0 - (-2.0 * dt).exp());
        self.cop_x += (self.x - self.cop_x) * (1.0 - (-8.0 * dt).exp());

        self.fill_world();
        self.obstacles.retain(|o| o.z - o.len() / 2.0 < self.z + 15.0);
        self.coins.retain(|c| c.z < self.z + 15.0 && !c.taken);

        // colisões
        let h = self.height();
        let top = self.y + h;
        let px = self.x;
        let pz = self.z;
        let py = self.y;

        let mut front_hit = false;
        let mut side_hit: Option<i32> = None;
        if self.invuln <= 0.0 {
            for o in &self.obstacles {
                let ox = lane_x(o.lane);
                if (px - ox).abs() > 0.85 {
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
                if !hit {
                    continue;
                }
                // já estava alinhado na faixa e ainda não encostava = de frente
                let was_aligned = (prev_x - ox).abs() <= 0.85;
                let was_in_z = (o.z - prev_z).abs() <= o.len() / 2.0 + 0.3;
                if was_aligned && !was_in_z {
                    front_hit = true;
                    break;
                } else {
                    side_hit = Some(o.lane);
                }
            }
        }

        let mut died = front_hit; // de frente: morre sempre
        if let Some(ol) = side_hit {
            if self.cop_t > 0.0 {
                died = true; // policial atrás + batida de lado = perdeu
            } else {
                // sem policial: ele aparece por mais 15s
                self.cop_t = COP_TIME;
                self.invuln = 1.0;
                self.cop_gap = 12.0;
                self.lane = if px < lane_x(ol) { (ol - 1).max(0) } else { (ol + 1).min(2) };
            }
        }
        if died {
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

    fn draw(&self, tex: &Textures) {
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
        // estrelas piscando
        let now = get_time() as f32;
        for i in 0..60u32 {
            let sx = hash(i, 1, 11) * sw;
            let sy = hash(i, 2, 12) * sh * 0.45;
            let tw = 0.5 + 0.5 * (now * 2.0 + i as f32).sin();
            draw_circle(sx, sy, 0.8 + hash(i, 3, 13) * 1.2, Color::new(1.0, 1.0, 1.0, 0.25 + 0.5 * tw));
        }
        // lua
        draw_circle(sw * 0.78, sh * 0.12, 36.0, Color::new(1.0, 0.95, 0.85, 0.12));
        draw_circle(sw * 0.78, sh * 0.12, 22.0, Color::new(0.98, 0.96, 0.88, 0.9));

        let cam = Camera3D {
            position: vec3(self.x * 0.55, 4.0 + self.y * 0.35, self.z + 6.5),
            target: vec3(self.x * 0.45, 1.6 + self.y * 0.25, self.z - 10.0),
            up: vec3((lane_x(self.lane) - self.x) * 0.03, 1.0, 0.0),
            fovy: 55.0f32.to_radians(),
            ..Default::default()
        };
        set_camera(&cam);

        self.draw_world(tex);
        self.draw_obstacles(tex);
        self.draw_coins();
        self.draw_player();
        self.draw_cop();

        set_default_camera();
        self.draw_hud();
    }

    fn draw_world(&self, t: &Textures) {
        let road_w = 3.0 * LANE_W;
        let cz = self.z + 6.5; // posição da câmera (para a névoa)

        // fundo escuro sob a pista (sem frestas)
        draw_cube(
            vec3(0.0, -0.25, self.z - 50.0),
            vec3(road_w + 7.0, 0.5, 200.0),
            None,
            Color::new(0.05, 0.05, 0.09, 1.0),
        );

        // pista e calçadas em blocos texturizados
        let base = (self.z / 4.0).floor() * 4.0;
        for i in -4..32 {
            let zz = base - i as f32 * 4.0;
            let idx = (zz / 4.0) as i32;
            let k = if idx.rem_euclid(2) == 0 { 1.0 } else { 0.88 };
            let road_c = fog(Color::new(0.24 * k, 0.24 * k, 0.30 * k, 1.0), cz, zz);
            draw_cube(vec3(0.0, 0.01, zz), vec3(road_w, 0.02, 3.95), Some(&t.asphalt), road_c);
            let pave_c = fog(Color::new(0.42 * k, 0.40 * k, 0.50 * k, 1.0), cz, zz);
            for sx in [-road_w / 2.0 - 1.5, road_w / 2.0 + 1.5] {
                draw_cube(vec3(sx, 0.1, zz), vec3(3.0, 0.2, 3.95), Some(&t.pavement), pave_c);
            }
        }
        // divisórias de faixa (neon) e bordas (rosa)
        for sx in [-LANE_W / 2.0, LANE_W / 2.0] {
            draw_cube(
                vec3(sx, 0.03, self.z - 50.0),
                vec3(0.07, 0.02, 200.0),
                None,
                Color::new(0.2, 0.95, 1.0, 1.0),
            );
        }
        for sx in [-road_w / 2.0, road_w / 2.0] {
            draw_cube(
                vec3(sx, 0.03, self.z - 50.0),
                vec3(0.1, 0.02, 200.0),
                None,
                Color::new(1.0, 0.25, 0.7, 1.0),
            );
        }

        // prédios e postes
        let base_i = (self.z / 6.0).floor() as i32;
        for i in -4..24 {
            let idx = base_i - i;
            let zz = idx as f32 * 6.0;
            for (side, sx) in [(0, -10.0f32), (1, 10.0f32)] {
                let seed = idx.wrapping_mul(7919).wrapping_add(side * 104729);
                let h = 5.0 + seed.rem_euclid(12) as f32;
                let tint = seed.rem_euclid(5) as f32 * 0.05;
                let wall = fog(
                    Color::new(0.38 + tint, 0.34 + tint * 0.5, 0.55 + tint, 1.0),
                    cz,
                    zz,
                );
                draw_cube(vec3(sx, h / 2.0, zz), vec3(5.0, h, 5.2), Some(&t.concrete), wall);
                // cobertura do prédio
                draw_cube(
                    vec3(sx, h + 0.15, zz),
                    vec3(5.2, 0.3, 5.4),
                    Some(&t.concrete),
                    fog(Color::new(0.3, 0.28, 0.4, 1.0), cz, zz),
                );
                let fx = if side == 0 { sx + 2.52 } else { sx - 2.52 };
                for wy in (1..h as i32).step_by(2) {
                    for wz in -1..=1 {
                        let lit = (seed + wy * 3 + wz * 5).rem_euclid(3) != 0;
                        let warm = (seed + wy * 7 + wz).rem_euclid(4) == 0;
                        let col = if lit {
                            if warm {
                                Color::new(1.0, 0.75, 0.4, 1.0)
                            } else {
                                Color::new(1.0, 0.92, 0.62, 1.0)
                            }
                        } else {
                            Color::new(0.14, 0.18, 0.32, 1.0)
                        };
                        draw_cube(
                            vec3(fx, wy as f32 + 0.5, zz + wz as f32 * 1.5),
                            vec3(0.05, 1.0, 0.9),
                            None,
                            fog(col, cz, zz),
                        );
                    }
                }
            }
            // poste de luz
            if idx.rem_euclid(2) == 0 {
                for sign in [-1.0f32, 1.0f32] {
                    let lx = sign * (road_w / 2.0 + 0.4);
                    let pole = fog(Color::new(0.2, 0.2, 0.25, 1.0), cz, zz);
                    draw_cube(vec3(lx, 2.2, zz), vec3(0.12, 4.4, 0.12), Some(&t.metal), pole);
                    draw_cube(vec3(lx - sign * 0.5, 4.4, zz), vec3(1.0, 0.1, 0.1), Some(&t.metal), pole);
                    draw_sphere(
                        vec3(lx - sign * 0.95, 4.35, zz),
                        0.18,
                        None,
                        fog(Color::new(1.0, 0.92, 0.65, 1.0), cz, zz),
                    );
                    draw_cube(
                        vec3(lx - sign * 0.95, 0.215, zz),
                        vec3(2.4, 0.01, 2.4),
                        None,
                        Color::new(1.0, 0.9, 0.6, 0.07),
                    );
                }
            }
        }
    }

    fn draw_obstacles(&self, t: &Textures) {
        let cz = self.z + 6.5;
        let blink = (get_time() * 6.0).sin() > 0.0;
        for o in &self.obstacles {
            let x = lane_x(o.lane);
            match o.kind {
                Kind::Low => {
                    // barreira de concreto com faixa de alerta
                    let conc = fog(Color::new(0.9, 0.9, 0.95, 1.0), cz, o.z);
                    draw_cube(vec3(x, 0.25, o.z), vec3(1.9, 0.5, 0.8), Some(&t.concrete), conc);
                    draw_cube(vec3(x, 0.7, o.z), vec3(1.5, 0.5, 0.55), Some(&t.concrete), conc);
                    draw_cube(
                        vec3(x, 0.62, o.z + 0.28),
                        vec3(1.52, 0.24, 0.06),
                        Some(&t.hazard),
                        fog(WHITE, cz, o.z),
                    );
                    let lamp = if blink { ORANGE } else { Color::new(0.3, 0.15, 0.05, 1.0) };
                    draw_sphere(vec3(x, 1.03, o.z), 0.1, None, fog(lamp, cz, o.z));
                    draw_cube_wires(vec3(x, 0.5, o.z), vec3(1.9, 1.0, 0.8), Color::new(0.0, 0.0, 0.0, 0.5));
                }
                Kind::High => {
                    // placa suspensa: rolar por baixo
                    let metal = fog(Color::new(0.6, 0.6, 0.66, 1.0), cz, o.z);
                    draw_cube(vec3(x - 0.9, 1.25, o.z), vec3(0.18, 2.5, 0.3), Some(&t.metal), metal);
                    draw_cube(vec3(x + 0.9, 1.25, o.z), vec3(0.18, 2.5, 0.3), Some(&t.metal), metal);
                    draw_cube(vec3(x, 1.85, o.z), vec3(2.0, 1.3, 0.45), Some(&t.hazard), fog(WHITE, cz, o.z));
                    draw_cube_wires(vec3(x, 1.85, o.z), vec3(2.0, 1.3, 0.45), Color::new(0.0, 0.0, 0.0, 0.6));
                    let lamp = if blink { RED } else { Color::new(0.35, 0.05, 0.05, 1.0) };
                    draw_sphere(vec3(x - 0.9, 2.6, o.z), 0.1, None, fog(lamp, cz, o.z));
                    draw_sphere(vec3(x + 0.9, 2.6, o.z), 0.1, None, fog(lamp, cz, o.z));
                }
                Kind::Train => {
                    let len = o.len();
                    let body = fog(Color::new(0.62, 0.38, 1.0, 1.0), cz, o.z);
                    draw_cube(vec3(x, 1.5, o.z), vec3(1.9, 3.0, len), Some(&t.metal), body);
                    // teto com ventilações
                    draw_cube(vec3(x, 3.05, o.z), vec3(1.7, 0.12, len - 0.4), Some(&t.metal), fog(Color::new(0.75, 0.75, 0.8, 1.0), cz, o.z));
                    for k in 0..3 {
                        draw_cube(
                            vec3(x, 3.2, o.z - len / 2.0 + 1.5 + k as f32 * 3.0),
                            vec3(0.8, 0.2, 0.8),
                            Some(&t.metal),
                            fog(Color::new(0.5, 0.5, 0.55, 1.0), cz, o.z),
                        );
                    }
                    // saia amarela
                    draw_cube(vec3(x, 0.6, o.z), vec3(1.93, 0.3, len), Some(&t.metal), fog(Color::new(1.0, 0.85, 0.2, 1.0), cz, o.z));
                    // chassi e rodas
                    draw_cube(vec3(x, 0.25, o.z), vec3(1.6, 0.3, len - 0.6), None, fog(Color::new(0.08, 0.07, 0.1, 1.0), cz, o.z));
                    for k in 0..2 {
                        let wz = o.z - len / 2.0 + 1.8 + k as f32 * (len - 3.6);
                        draw_cube(vec3(x - 0.7, 0.18, wz), vec3(0.25, 0.36, 0.9), None, fog(Color::new(0.05, 0.05, 0.06, 1.0), cz, o.z));
                        draw_cube(vec3(x + 0.7, 0.18, wz), vec3(0.25, 0.36, 0.9), None, fog(Color::new(0.05, 0.05, 0.06, 1.0), cz, o.z));
                    }
                    // janelas iluminadas
                    for k in 0..4 {
                        let wz = o.z - len / 2.0 + 1.2 + k as f32 * 2.2;
                        let glass = fog(Color::new(0.55, 0.85, 1.0, 1.0), cz, o.z);
                        draw_cube(vec3(x - 0.96, 2.0, wz), vec3(0.04, 0.8, 1.2), None, glass);
                        draw_cube(vec3(x + 0.96, 2.0, wz), vec3(0.04, 0.8, 1.2), None, glass);
                    }
                    // frente: faróis e para-brisa
                    let fz = o.z + len / 2.0;
                    let lamp = fog(Color::new(1.0, 1.0, 0.75, 1.0), cz, o.z);
                    draw_sphere(vec3(x - 0.6, 0.9, fz + 0.04), 0.17, None, lamp);
                    draw_sphere(vec3(x + 0.6, 0.9, fz + 0.04), 0.17, None, lamp);
                    draw_cube(vec3(x, 1.9, fz - 0.02), vec3(1.4, 1.0, 0.05), None, fog(Color::new(0.2, 0.3, 0.45, 1.0), cz, o.z));
                    draw_cube(vec3(x, 1.4, fz + 0.02), vec3(1.5, 0.1, 0.05), None, fog(Color::new(1.0, 0.25, 0.7, 1.0), cz, o.z));
                    draw_cube_wires(vec3(x, 1.5, o.z), vec3(1.9, 3.0, len), Color::new(0.0, 0.0, 0.0, 0.5));
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
            draw_sphere(vec3(x, c.y, c.z), 0.38, None, Color::new(1.0, 0.85, 0.2, 0.12));
        }
    }

    fn draw_player(&self) {
        // pisca enquanto está invencível
        if self.invuln > 0.0 && ((get_time() * 16.0) as i32) % 2 == 0 {
            return;
        }

        let x = self.x;
        let z = self.z;
        let look = Look {
            shirt: Color::new(0.14, 0.17, 0.26, 1.0),
            pants: Color::new(0.16, 0.16, 0.2, 1.0),
            shoes: Color::new(0.93, 0.93, 0.97, 1.0),
            skin: Color::new(0.86, 0.66, 0.5, 1.0),
            hair: Color::new(0.1, 0.07, 0.05, 1.0),
            belt: Color::new(0.08, 0.08, 0.1, 1.0),
            accent: Color::new(0.2, 0.95, 1.0, 1.0),
        };

        // sombra (diminui no pulo)
        let sz = (0.95 - self.y * 0.12).max(0.45);
        draw_cube(vec3(x, 0.03, z), vec3(sz, 0.02, sz), None, Color::new(0.0, 0.0, 0.0, 0.5));

        if self.roll_t > 0.0 {
            // rolando: corpo enrolado girando
            let a = self.run_t * 6.0;
            let cy = self.y + 0.42;
            draw_sphere(vec3(x, cy, z), 0.42, None, look.shirt);
            draw_sphere(vec3(x, cy + a.sin() * 0.36, z + a.cos() * 0.36), 0.13, None, look.shoes);
            draw_sphere(vec3(x, cy - a.sin() * 0.36, z - a.cos() * 0.36), 0.12, None, look.accent);
            draw_sphere(vec3(x, cy + 0.05, z - 0.3), 0.2, None, look.hair);
            return;
        }

        let in_air = self.y > 0.01;
        let lean = (lane_x(self.lane) - self.x) * 0.18;
        draw_runner(x, self.y, z, self.run_t, in_air, lean, &look, false);
    }

    fn draw_cop(&self) {
        if self.cop_gap > 25.0 {
            return;
        }
        let x = self.cop_x;
        let z = self.z + self.cop_gap;
        let look = Look {
            shirt: Color::new(0.13, 0.22, 0.5, 1.0),
            pants: Color::new(0.07, 0.09, 0.2, 1.0),
            shoes: Color::new(0.05, 0.05, 0.06, 1.0),
            skin: Color::new(0.9, 0.7, 0.55, 1.0),
            hair: Color::new(0.08, 0.12, 0.3, 1.0),
            belt: Color::new(0.04, 0.04, 0.05, 1.0),
            accent: Color::new(1.0, 0.85, 0.2, 1.0),
        };

        // luzes de sirene no chão
        let on = (get_time() * 10.0).sin() > 0.0;
        let glow = if on {
            Color::new(1.0, 0.1, 0.1, 0.18)
        } else {
            Color::new(0.1, 0.2, 1.0, 0.18)
        };
        draw_cube(vec3(x, 0.04, z), vec3(2.4, 0.01, 2.4), None, glow);
        draw_cube(vec3(x, 0.03, z), vec3(0.95, 0.02, 0.95), None, Color::new(0.0, 0.0, 0.0, 0.5));

        draw_runner(x, 0.0, z, self.run_t + 1.0, false, 0.0, &look, true);

        // sirene no boné
        let siren = if on { RED } else { BLUE };
        draw_sphere(vec3(x, 2.2, z), 0.1, None, siren);
    }

    fn draw_timer(&self) {
        let r = self.timer_rect();
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.5));
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(0.2, 0.95, 1.0, 1.0));
        draw_text(&self.timer_text(), r.x + 12.0, r.y + 31.0, 34.0, WHITE);
    }

    fn draw_hud(&self) {
        draw_text(&format!("Pontos: {}", self.score()), 20.0, 40.0, 40.0, WHITE);
        draw_text(&format!("Moedas: {}", self.coins_got), 20.0, 80.0, 32.0, GOLD);
        draw_text(&format!("Recorde: {}", self.best.max(self.score())), 20.0, 115.0, 28.0, LIGHTGRAY);
        draw_text(&format!("{} km/h", (self.speed * 3.6) as i32), 20.0, 145.0, 24.0, SKYBLUE);

        if self.cop_t > 0.0 && !self.dead {
            draw_text(&format!("Policial: {}s", self.cop_t.ceil() as i32), 20.0, 175.0, 28.0, RED);
        }

        self.draw_timer();

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
            let t4 = format!("Tempo: {}", self.timer_text());
            let d4 = measure_text(&t4, None, 32, 1.0);
            draw_text(&t4, (w - d4.width) / 2.0, h / 2.0 + 68.0, 32.0, SKYBLUE);
            let t3 = "Espaço / Enter / toque para recomeçar";
            let d3 = measure_text(t3, None, 28, 1.0);
            draw_text(t3, (w - d3.width) / 2.0, h / 2.0 + 108.0, 28.0, LIGHTGRAY);
        }
    }
}

// boneco articulado: coxa + canela + tênis, tronco, ombros, braço + antebraço + mão, pescoço e cabeça
fn draw_runner(x: f32, y: f32, z: f32, run_t: f32, in_air: bool, lean: f32, l: &Look, cop: bool) {
    let swing = (run_t * 2.0).sin();
    let bob = if in_air { 0.0 } else { (run_t * 4.0).sin().abs() * 0.06 };
    let by = y + bob;
    let tx = x + lean * 0.5;
    let hx = x + lean;

    // pernas
    for (sx, ph) in [(-0.17f32, 1.0f32), (0.17f32, -1.0f32)] {
        let leg = if in_air { ph * 0.3 } else { swing * ph * 0.45 };
        let back = leg.max(0.0);
        draw_cube(vec3(x + sx, by + 0.68, z + leg * 0.5), vec3(0.22, 0.46, 0.26), None, l.pants);
        draw_cube(
            vec3(x + sx, by + 0.28 + back * 0.35, z + leg * 1.1 + back * 0.25),
            vec3(0.19, 0.46, 0.22),
            None,
            l.pants,
        );
        draw_cube(
            vec3(x + sx, by + 0.07 + back * 0.45, z + leg * 1.15 + back * 0.3 - 0.05),
            vec3(0.22, 0.14, 0.38),
            None,
            l.shoes,
        );
    }

    // tronco, ombros e cinto
    draw_cube(vec3(tx, by + 1.12, z), vec3(0.62, 0.62, 0.34), None, l.shirt);
    draw_cube(vec3(tx, by + 1.42, z), vec3(0.8, 0.18, 0.36), None, l.shirt);
    draw_cube(vec3(tx, by + 0.82, z), vec3(0.64, 0.1, 0.36), None, l.belt);

    if cop {
        // "POLICIA" amarelo nas costas + coldre
        draw_cube(vec3(tx, by + 1.2, z + 0.18), vec3(0.42, 0.1, 0.03), None, l.accent);
        draw_cube(vec3(tx + 0.36, by + 0.72, z), vec3(0.1, 0.28, 0.2), None, l.belt);
    } else {
        // listra neon + mochila
        draw_cube(vec3(tx, by + 1.3, z + 0.18), vec3(0.62, 0.06, 0.03), None, l.accent);
        draw_cube(vec3(tx, by + 1.12, z + 0.28), vec3(0.46, 0.52, 0.2), None, Color::new(0.1, 0.1, 0.13, 1.0));
        draw_cube(vec3(tx, by + 1.12, z + 0.39), vec3(0.3, 0.06, 0.02), None, l.accent);
    }

    // braços
    for (sx, ph) in [(-1.0f32, -1.0f32), (1.0f32, 1.0f32)] {
        let arm = if in_air { -0.6 } else { swing * ph * 0.55 };
        let ax = tx + sx * 0.46;
        draw_cube(vec3(ax, by + 1.2, z + arm * 0.5), vec3(0.16, 0.42, 0.2), None, l.shirt);
        draw_cube(vec3(ax, by + 0.9, z + arm * 0.9 - 0.1), vec3(0.14, 0.38, 0.18), None, l.skin);
        draw_sphere(vec3(ax, by + 0.7, z + arm * 1.0 - 0.14), 0.09, None, l.skin);
    }

    // pescoço e cabeça
    draw_cube(vec3(hx, by + 1.58, z), vec3(0.14, 0.12, 0.14), None, l.skin);
    draw_sphere(vec3(hx, by + 1.78, z), 0.24, None, l.skin);
    if cop {
        // boné
        draw_sphere(vec3(hx, by + 1.84, z + 0.03), 0.27, None, l.shirt);
        draw_cube(vec3(hx, by + 1.97, z - 0.02), vec3(0.54, 0.1, 0.54), None, l.shirt);
        draw_cube(vec3(hx, by + 1.9, z - 0.3), vec3(0.4, 0.04, 0.25), None, l.belt);
        draw_cube(vec3(hx, by + 1.97, z - 0.285), vec3(0.14, 0.1, 0.02), None, l.accent);
    } else {
        // cabelo + faixa neon
        draw_sphere(vec3(hx, by + 1.83, z + 0.04), 0.255, None, l.hair);
        draw_cube(vec3(hx, by + 1.8, z + 0.02), vec3(0.5, 0.07, 0.5), None, l.accent);
    }
}

// ---------- botão de tela cheia (canto superior direito) ----------

fn fs_button_rect() -> Rect {
    Rect::new(screen_width() - 56.0, 12.0, 44.0, 44.0)
}

fn draw_fs_button(r: Rect, fullscreen: bool) {
    let cyan = Color::new(0.2, 0.95, 1.0, 1.0);
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.5));
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, cyan);
    let cx = r.x + r.w / 2.0;
    let cy = r.y + r.h / 2.0;
    let arm = 7.0;
    for (dx, dy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        if fullscreen {
            // setas para dentro: clique para sair da tela cheia
            let (px, py) = (cx + dx * 4.0, cy + dy * 4.0);
            draw_line(px, py, px + dx * arm, py, 2.5, WHITE);
            draw_line(px, py, px, py + dy * arm, 2.5, WHITE);
        } else {
            // cantos para fora: clique para entrar em tela cheia
            let (px, py) = (cx + dx * 11.0, cy + dy * 11.0);
            draw_line(px, py, px - dx * arm, py, 2.5, WHITE);
            draw_line(px, py, px, py - dy * arm, 2.5, WHITE);
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
    let tex = Textures::new();
    let mut game = Game::new(0, 1);
    let mut fullscreen = false;

    loop {
        let dt = get_frame_time().min(0.05);

        // botão de tela cheia (toque, clique ou tecla F)
        let btn = fs_button_rect();
        let mut fs_pressed = is_key_pressed(KeyCode::F);
        if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();
            if btn.contains(vec2(mx, my)) {
                fs_pressed = true;
            }
        }
        for t in touches() {
            if t.phase == TouchPhase::Started && btn.contains(t.position) {
                fs_pressed = true;
            }
        }
        if fs_pressed {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }

        if game.dead {
            // tocar no botão de tela cheia não reinicia o jogo
            let tapped = touches()
                .iter()
                .any(|t| t.phase == TouchPhase::Started && !btn.contains(t.position));
            if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) || tapped {
                let best = game.best;
                let pos = game.timer_pos;
                game = Game::new(best, pos);
            }
        }

        game.update(dt);
        game.draw(&tex);
        draw_fs_button(fs_button_rect(), fullscreen);
        next_frame().await;
    }
}
