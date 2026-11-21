use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;

const LANE_W: f32 = 2.2;
const GRAVITY: f32 = 32.0;
const JUMP_V: f32 = 11.5;
const ROLL_TIME: f32 = 0.65;
const COP_TIME: f32 = 7.0; // segundos que o policial fica atrás de você

// Velocidade: começa em START_SPEED e sobe SPEED_GAIN por segundo, até MAX_SPEED
const START_SPEED: f32 = 14.0;
const SPEED_GAIN: f32 = 0.15;
const MAX_SPEED: f32 = 45.0;

// Chaves azuis (999 trilhões ao abrir o jogo)
const START_KEYS: u64 = 999_000_000_000_000;
// Raio (em metros) que é limpo ao renascer ou quando a prancha salva você
const CLEAR_RADIUS: f32 = 30.0;
// Tempo de espera para usar a prancha de novo depois que ela quebra
const BOARD_CD: f32 = 20.0;

const TAU: f32 = std::f32::consts::TAU;

fn lane_x(l: i32) -> f32 {
    (l - 1) as f32 * LANE_W
}

fn c3(r: f32, g: f32, b: f32) -> Color {
    Color::new(r, g, b, 1.0)
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

// eventos do jogo que viram sons
#[derive(Clone, Copy, PartialEq)]
enum Ev {
    Jump,
    Roll,
    Coin,
    Crash,
    Cop,
    Revive,
    Board,
    BoardBreak,
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
    // prancha (funciona como o totem do Minecraft)
    board_on: bool,
    board_cd: f32,
    revive_cost: u64, // custo em chaves para renascer (1, 2, 4, 8...)
    last_tap: f64,    // para detectar toque duplo
    // skins escolhidas no vestiário
    pskin: usize,
    cskin: usize,
    bskin: usize,
    // mundo
    obstacles: Vec<Obstacle>,
    coins: Vec<Coin>,
    next_z: f32,
    // estado
    dead: bool,
    coins_got: u32,
    dist: f32,
    best: u32,
    time: f32,        // tempo de corrida em segundos
    timer_pos: usize, // posição do cronômetro na tela
    swipe_start: Option<Vec2>,
    events: Vec<Ev>,
}

impl Game {
    fn new(best: u32, timer_pos: usize, pskin: usize, cskin: usize, bskin: usize) -> Self {
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
            board_on: false,
            board_cd: 0.0,
            revive_cost: 1,
            last_tap: -10.0,
            pskin,
            cskin,
            bskin,
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
            events: vec![],
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
            self.events.push(Ev::Jump);
        }
    }

    fn roll(&mut self) {
        self.roll_t = ROLL_TIME;
        self.events.push(Ev::Roll);
        if self.y > 0.001 {
            self.vy = -22.0; // queda rápida
        }
    }

    // ESPAÇO (ou o botão da prancha na tela) coloca a prancha
    fn activate_board(&mut self) {
        if !self.dead && !self.board_on && self.board_cd <= 0.0 {
            self.board_on = true;
            self.events.push(Ev::Board);
        }
    }

    // renascer: some tudo num raio de 30 m
    fn revive(&mut self) {
        let zc = self.z;
        self.obstacles.retain(|o| (o.z - zc).abs() > CLEAR_RADIUS + o.len() / 2.0);
        self.revive_cost = self.revive_cost.saturating_mul(2);
        self.dead = false;
        self.invuln = 2.5;
        self.roll_t = 0.0;
        self.vy = 0.0;
        self.y = 0.0;
        self.events.push(Ev::Revive);
    }

    fn handle_input(&mut self) {
        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
            self.lane = (self.lane - 1).max(0);
        }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
            self.lane = (self.lane + 1).min(2);
        }
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
            self.jump();
        }
        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
            self.roll();
        }
        if is_key_pressed(KeyCode::Space) {
            self.activate_board();
        }

        // botão da prancha na tela (celular)
        let br = board_button_rect();
        if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();
            if br.contains(vec2(mx, my)) {
                self.activate_board();
            }
        }
        for t in touches() {
            if t.phase == TouchPhase::Started {
                if br.contains(t.position) {
                    self.activate_board();
                } else if !self.timer_rect().contains(t.position)
                    && !fs_button_rect().contains(t.position)
                {
                    // dois toques rápidos na tela ativam a prancha
                    let now = get_time();
                    if now - self.last_tap < 0.3 {
                        self.activate_board();
                        self.last_tap = -10.0;
                    } else {
                        self.last_tap = now;
                    }
                }
            }
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
            0 => ((sw - w) / 2.0, m),          // topo, centro
            1 => (sw - w - m, 68.0),           // topo, direita (abaixo do botão de tela cheia)
            2 => ((sw - w) / 2.0, sh - h - m), // baixo, centro
            _ => (m, sh - h - m),              // baixo, esquerda
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

        // policial e prancha
        if self.invuln > 0.0 {
            self.invuln -= dt;
        }
        if self.board_cd > 0.0 {
            self.board_cd -= dt;
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

        // com a prancha você não morre: ela quebra e limpa 30 m em volta
        if self.board_on && (front_hit || side_hit.is_some()) {
            self.board_on = false;
            self.board_cd = BOARD_CD;
            self.invuln = 1.5;
            let zc = self.z;
            self.obstacles.retain(|o| (o.z - zc).abs() > CLEAR_RADIUS + o.len() / 2.0);
            self.events.push(Ev::BoardBreak);
            front_hit = false;
            side_hit = None;
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
                self.events.push(Ev::Cop);
            }
        }
        if died {
            self.dead = true;
            self.best = self.best.max(self.score());
            self.events.push(Ev::Crash);
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
        if got > 0 {
            self.events.push(Ev::Coin);
        }
        self.coins_got += got;
    }

    // ---------- desenho ----------

    fn draw(&self, tex: &Textures) {
        draw_sky();

        // câmera um pouco mais alta
        let cam = Camera3D {
            position: vec3(self.x * 0.55, 5.4 + self.y * 0.35, self.z + 7.0),
            target: vec3(self.x * 0.45, 1.3 + self.y * 0.25, self.z - 10.0),
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
        let look = player_skin(self.pskin).1;

        // sombra (diminui no pulo)
        let sz = (0.95 - self.y * 0.12).max(0.45);
        draw_cube(vec3(x, 0.03, z), vec3(sz, 0.02, sz), None, Color::new(0.0, 0.0, 0.0, 0.5));

        // prancha
        let mut lift = 0.0;
        if self.board_on {
            let (_, bm, bg) = board_skin(self.bskin, get_time() as f32);
            let hover = (get_time() as f32 * 5.0).sin() * 0.03;
            draw_board(x, self.y + 0.1 + hover, z, bm, bg);
            lift = 0.28 + hover;
        }

        if self.roll_t > 0.0 {
            // rolando: corpo enrolado girando
            let a = self.run_t * 6.0;
            let cy = self.y + 0.42 + lift;
            draw_sphere(vec3(x, cy, z), 0.42, None, look.shirt);
            draw_sphere(vec3(x, cy + a.sin() * 0.36, z + a.cos() * 0.36), 0.13, None, look.shoes);
            draw_sphere(vec3(x, cy - a.sin() * 0.36, z - a.cos() * 0.36), 0.12, None, look.accent);
            draw_sphere(vec3(x, cy + 0.05, z - 0.3), 0.2, None, look.hair);
            return;
        }

        let in_air = self.y > 0.01 || self.board_on;
        let lean = (lane_x(self.lane) - self.x) * 0.18;
        draw_runner(x, self.y + lift, z, self.run_t, in_air, lean, &look, false);
    }

    fn draw_cop(&self) {
        if self.cop_gap > 25.0 {
            return;
        }
        let x = self.cop_x;
        let z = self.z + self.cop_gap;
        let look = cop_skin(self.cskin).1;

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

    fn draw_board_button(&self) {
        let r = board_button_rect();
        let cx = r.x + r.w / 2.0;
        let cy = r.y + r.h / 2.0;
        let (_, _, glow) = board_skin(self.bskin, get_time() as f32);
        let ring = if self.board_on {
            Color::new(0.2, 1.0, 0.4, 1.0)
        } else if self.board_cd > 0.0 {
            GRAY
        } else {
            Color::new(0.2, 0.95, 1.0, 1.0)
        };
        draw_circle(cx, cy, 34.0, Color::new(0.0, 0.0, 0.0, 0.55));
        draw_circle_lines(cx, cy, 34.0, 3.0, ring);
        draw_rectangle(cx - 18.0, cy - 12.0, 36.0, 9.0, glow);
        let label = if self.board_on {
            "ATIVA".to_string()
        } else if self.board_cd > 0.0 {
            format!("{}s", self.board_cd.ceil() as i32)
        } else {
            "PRANCHA".to_string()
        };
        let d = measure_text(&label, None, 16, 1.0);
        draw_text(&label, cx - d.width / 2.0, cy + 18.0, 16.0, WHITE);
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
        self.draw_board_button();
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


// ---------- céu ----------

fn draw_sky() {
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
    let now = get_time() as f32;
    for i in 0..60u32 {
        let sx = hash(i, 1, 11) * sw;
        let sy = hash(i, 2, 12) * sh * 0.45;
        let tw = 0.5 + 0.5 * (now * 2.0 + i as f32).sin();
        draw_circle(sx, sy, 0.8 + hash(i, 3, 13) * 1.2, Color::new(1.0, 1.0, 1.0, 0.25 + 0.5 * tw));
    }
    draw_circle(sw * 0.78, sh * 0.12, 36.0, Color::new(1.0, 0.95, 0.85, 0.12));
    draw_circle(sw * 0.78, sh * 0.12, 22.0, Color::new(0.98, 0.96, 0.88, 0.9));
}

fn board_button_rect() -> Rect {
    Rect::new(screen_width() - 84.0, screen_height() - 96.0, 68.0, 68.0)
}

// ---------- skins ----------

fn player_skin(i: usize) -> (&'static str, Look) {
    match i % 6 {
        0 => (
            "Neon",
            Look {
                shirt: c3(0.14, 0.17, 0.26),
                pants: c3(0.16, 0.16, 0.2),
                shoes: c3(0.93, 0.93, 0.97),
                skin: c3(0.86, 0.66, 0.5),
                hair: c3(0.1, 0.07, 0.05),
                belt: c3(0.08, 0.08, 0.1),
                accent: c3(0.2, 0.95, 1.0),
            },
        ),
        1 => (
            "Fogo",
            Look {
                shirt: c3(0.75, 0.12, 0.1),
                pants: c3(0.15, 0.1, 0.1),
                shoes: c3(0.95, 0.8, 0.2),
                skin: c3(0.86, 0.66, 0.5),
                hair: c3(0.9, 0.4, 0.1),
                belt: c3(0.08, 0.05, 0.05),
                accent: c3(1.0, 0.6, 0.1),
            },
        ),
        2 => (
            "Floresta",
            Look {
                shirt: c3(0.1, 0.4, 0.18),
                pants: c3(0.12, 0.16, 0.1),
                shoes: c3(0.85, 0.9, 0.8),
                skin: c3(0.6, 0.42, 0.3),
                hair: c3(0.05, 0.05, 0.05),
                belt: c3(0.06, 0.08, 0.05),
                accent: c3(0.6, 1.0, 0.2),
            },
        ),
        3 => (
            "Ouro",
            Look {
                shirt: c3(0.1, 0.1, 0.1),
                pants: c3(0.1, 0.1, 0.1),
                shoes: c3(0.9, 0.75, 0.2),
                skin: c3(0.95, 0.8, 0.65),
                hair: c3(0.85, 0.7, 0.2),
                belt: c3(0.9, 0.75, 0.2),
                accent: c3(1.0, 0.85, 0.2),
            },
        ),
        4 => (
            "Rosa",
            Look {
                shirt: c3(0.9, 0.3, 0.6),
                pants: c3(0.25, 0.2, 0.4),
                shoes: c3(0.95, 0.95, 0.95),
                skin: c3(0.9, 0.72, 0.58),
                hair: c3(0.3, 0.1, 0.4),
                belt: c3(0.15, 0.1, 0.2),
                accent: c3(1.0, 0.4, 0.9),
            },
        ),
        _ => (
            "Fantasma",
            Look {
                shirt: c3(0.85, 0.87, 0.95),
                pants: c3(0.5, 0.5, 0.62),
                shoes: c3(0.3, 0.25, 0.5),
                skin: c3(0.9, 0.9, 1.0),
                hair: c3(0.55, 0.5, 0.8),
                belt: c3(0.3, 0.3, 0.45),
                accent: c3(0.7, 0.4, 1.0),
            },
        ),
    }
}

fn cop_skin(i: usize) -> (&'static str, Look) {
    match i % 6 {
        0 => (
            "Clássico",
            Look {
                shirt: c3(0.13, 0.22, 0.5),
                pants: c3(0.07, 0.09, 0.2),
                shoes: c3(0.05, 0.05, 0.06),
                skin: c3(0.9, 0.7, 0.55),
                hair: c3(0.08, 0.12, 0.3),
                belt: c3(0.04, 0.04, 0.05),
                accent: c3(1.0, 0.85, 0.2),
            },
        ),
        1 => (
            "SWAT",
            Look {
                shirt: c3(0.1, 0.1, 0.12),
                pants: c3(0.12, 0.12, 0.14),
                shoes: c3(0.03, 0.03, 0.03),
                skin: c3(0.8, 0.62, 0.48),
                hair: c3(0.1, 0.1, 0.12),
                belt: c3(0.2, 0.2, 0.22),
                accent: c3(1.0, 0.5, 0.1),
            },
        ),
        2 => (
            "Xerife",
            Look {
                shirt: c3(0.55, 0.42, 0.25),
                pants: c3(0.35, 0.28, 0.18),
                shoes: c3(0.2, 0.12, 0.08),
                skin: c3(0.9, 0.7, 0.55),
                hair: c3(0.4, 0.3, 0.2),
                belt: c3(0.25, 0.15, 0.08),
                accent: c3(0.95, 0.85, 0.3),
            },
        ),
        3 => (
            "Robô",
            Look {
                shirt: c3(0.55, 0.58, 0.65),
                pants: c3(0.3, 0.32, 0.4),
                shoes: c3(0.15, 0.15, 0.2),
                skin: c3(0.6, 0.65, 0.7),
                hair: c3(0.4, 0.42, 0.5),
                belt: c3(0.1, 0.1, 0.14),
                accent: c3(1.0, 0.15, 0.15),
            },
        ),
        4 => (
            "Neon",
            Look {
                shirt: c3(0.1, 0.05, 0.2),
                pants: c3(0.07, 0.03, 0.15),
                shoes: c3(0.9, 0.2, 0.7),
                skin: c3(0.9, 0.7, 0.55),
                hair: c3(0.1, 0.05, 0.2),
                belt: c3(0.9, 0.2, 0.7),
                accent: c3(1.0, 0.2, 0.8),
            },
        ),
        _ => (
            "Militar",
            Look {
                shirt: c3(0.25, 0.32, 0.18),
                pants: c3(0.2, 0.25, 0.14),
                shoes: c3(0.1, 0.1, 0.08),
                skin: c3(0.85, 0.68, 0.52),
                hair: c3(0.2, 0.25, 0.14),
                belt: c3(0.1, 0.12, 0.07),
                accent: c3(0.8, 0.8, 0.4),
            },
        ),
    }
}

// retorna (nome, cor principal, cor do brilho)
fn board_skin(i: usize, t: f32) -> (&'static str, Color, Color) {
    match i % 6 {
        0 => ("Neon", c3(0.1, 0.12, 0.2), c3(0.2, 0.95, 1.0)),
        1 => ("Fogo", c3(0.25, 0.08, 0.05), c3(1.0, 0.45, 0.1)),
        2 => ("Gelo", c3(0.75, 0.9, 1.0), c3(0.4, 0.8, 1.0)),
        3 => ("Ouro", c3(0.85, 0.7, 0.2), c3(1.0, 0.9, 0.4)),
        4 => ("Rosa", c3(0.5, 0.1, 0.35), c3(1.0, 0.3, 0.8)),
        _ => (
            "Arco-íris",
            c3(0.1, 0.1, 0.12),
            c3(
                0.5 + 0.5 * (t * 2.0).sin(),
                0.5 + 0.5 * (t * 2.0 + 2.09).sin(),
                0.5 + 0.5 * (t * 2.0 + 4.19).sin(),
            ),
        ),
    }
}

// (nome, cor 1, cor 2, cor 3) para mostrar nas amostras do vestiário
fn skin_info(tab: usize, i: usize) -> (&'static str, Color, Color, Color) {
    match tab {
        0 => {
            let (n, l) = player_skin(i);
            (n, l.shirt, l.accent, l.pants)
        }
        1 => {
            let (n, l) = cop_skin(i);
            (n, l.shirt, l.accent, l.pants)
        }
        _ => {
            let (n, m, g) = board_skin(i, get_time() as f32);
            (n, m, g, g)
        }
    }
}

fn draw_board(x: f32, y: f32, z: f32, main: Color, glow: Color) {
    draw_cube(vec3(x, y + 0.1, z), vec3(0.8, 0.1, 1.9), None, main);
    draw_cube(vec3(x, y + 0.16, z), vec3(0.6, 0.02, 1.6), None, glow);
    draw_cube(vec3(x, y + 0.04, z), vec3(0.7, 0.04, 1.7), None, glow);
    draw_sphere(vec3(x, y + 0.1, z - 0.95), 0.12, None, glow);
    draw_sphere(vec3(x, y + 0.1, z + 0.95), 0.12, None, glow);
    draw_cube(
        vec3(x, 0.03, z),
        vec3(1.1, 0.01, 2.2),
        None,
        Color::new(glow.r, glow.g, glow.b, 0.18),
    );
}

// ---------- sons (gerados pelo código, sem arquivos) ----------

fn wav_bytes(samples: &[f32], rate: u32) -> Vec<u8> {
    let n = samples.len() as u32;
    let mut b = Vec::with_capacity(44 + samples.len() * 2);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + n * 2).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(n * 2).to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 30000.0) as i16;
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

fn synth<F: Fn(f32, f32) -> f32>(rate: u32, dur: f32, f: F) -> Vec<f32> {
    let n = (rate as f32 * dur) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / rate as f32;
            f(t, t / dur)
        })
        .collect()
}

fn noise(t: f32) -> f32 {
    hash((t * 22050.0) as u32, 7, 99) * 2.0 - 1.0
}

fn music(rate: u32) -> Vec<f32> {
    let step = 0.2f32;
    let steps = 32usize;
    let arp = [
        0.0f32, 3.0, 7.0, 12.0, 7.0, 3.0, 7.0, 10.0, 0.0, 3.0, 7.0, 12.0, 14.0, 12.0, 10.0, 7.0,
    ];
    let bass = [-24.0f32, -24.0, -21.0, -21.0, -19.0, -19.0, -17.0, -21.0];
    synth(rate, step * steps as f32, |t, _| {
        let s = (t / step) as usize;
        let lt = t - s as f32 * step;
        let env = (1.0 - lt / step).max(0.0);
        let fa = 220.0 * 2f32.powf(arp[s % 16] / 12.0);
        let pa = (fa * t).fract();
        let sq = if pa < 0.5 { 1.0 } else { -1.0 };
        let lead = sq * 0.07 * env.powf(1.5);
        let fb = 220.0 * 2f32.powf(bass[(s / 4) % 8] / 12.0);
        let b = (TAU * fb * t).sin() * 0.22 * (0.6 + 0.4 * env);
        let kick = if s % 4 == 0 {
            (TAU * (110.0 - 70.0 * (lt / step)) * lt).sin() * 0.35 * env
        } else {
            0.0
        };
        let hat = if s % 2 == 1 { noise(t) * 0.05 * env.powf(3.0) } else { 0.0 };
        lead + b + kick + hat
    })
}

async fn load(samples: Vec<f32>, rate: u32) -> Option<Sound> {
    load_sound_from_bytes(&wav_bytes(&samples, rate)).await.ok()
}

fn play(s: &Option<Sound>, volume: f32) {
    if let Some(s) = s {
        play_sound(s, PlaySoundParams { looped: false, volume });
    }
}

struct Sounds {
    coin: Option<Sound>,
    jump: Option<Sound>,
    roll: Option<Sound>,
    crash: Option<Sound>,
    cop: Option<Sound>,
    revive: Option<Sound>,
    board: Option<Sound>,
    board_break: Option<Sound>,
    click: Option<Sound>,
    music: Option<Sound>,
}

impl Sounds {
    async fn new() -> Self {
        let r = 22050u32;
        let coin = synth(r, 0.25, |t, p| {
            let f = if t < 0.08 { 988.0 } else { 1319.0 };
            let env = (1.0 - p).powf(2.0);
            (TAU * f * t).sin() * env * 0.45 + (TAU * f * 2.0 * t).sin() * env * 0.1
        });
        let jump = synth(r, 0.22, |t, p| {
            let ph = TAU * (300.0 * t + 0.5 * (400.0 / 0.22) * t * t);
            ph.sin() * (1.0 - p) * 0.4
        });
        let roll = synth(r, 0.3, |t, p| {
            noise(t) * (p * std::f32::consts::PI).sin() * 0.3
        });
        let crash = synth(r, 0.6, |t, p| {
            noise(t) * (1.0 - p).powf(2.0) * 0.6 + (TAU * (90.0 - 60.0 * p) * t).sin() * (1.0 - p) * 0.7
        });
        let cop = synth(r, 0.8, |t, _| {
            let f = if (t * 5.0) % 1.0 < 0.5 { 700.0 } else { 950.0 };
            let env = if t > 0.7 { (0.8 - t) * 10.0 } else { 1.0 };
            (TAU * f * t).sin() * 0.3 * env
        });
        let revive = synth(r, 0.7, |t, p| {
            let notes = [523.0f32, 659.0, 784.0, 1047.0];
            let k = ((t / 0.12) as usize).min(3);
            (TAU * notes[k] * t).sin() * 0.4 * (1.0 - p * 0.8)
        });
        let board = synth(r, 0.45, |t, p| {
            let ph = TAU * (200.0 * t + 0.5 * (1200.0 / 0.45) * t * t);
            ph.sin() * 0.35 * (1.0 - p).sqrt() + noise(t) * 0.04
        });
        let board_break = synth(r, 0.55, |t, p| {
            noise(t) * (1.0 - p).powf(2.0) * 0.5 + (TAU * (500.0 - 400.0 * p) * t).sin() * (1.0 - p) * 0.4
        });
        let click = synth(r, 0.06, |t, p| (TAU * 900.0 * t).sin() * (1.0 - p) * 0.35);

        Sounds {
            coin: load(coin, r).await,
            jump: load(jump, r).await,
            roll: load(roll, r).await,
            crash: load(crash, r).await,
            cop: load(cop, r).await,
            revive: load(revive, r).await,
            board: load(board, r).await,
            board_break: load(board_break, r).await,
            click: load(click, r).await,
            music: load(music(r), r).await,
        }
    }

    fn play_ev(&self, e: Ev) {
        match e {
            Ev::Jump => play(&self.jump, 0.5),
            Ev::Roll => play(&self.roll, 0.5),
            Ev::Coin => play(&self.coin, 0.6),
            Ev::Crash => play(&self.crash, 0.9),
            Ev::Cop => play(&self.cop, 0.7),
            Ev::Revive => play(&self.revive, 0.8),
            Ev::Board => play(&self.board, 0.7),
            Ev::BoardBreak => play(&self.board_break, 0.9),
        }
    }

    fn click(&self) {
        play(&self.click, 0.6);
    }

    fn start_music(&self) {
        if let Some(m) = &self.music {
            play_sound(m, PlaySoundParams { looped: true, volume: 0.3 });
        }
    }

    fn stop_music(&self) {
        if let Some(m) = &self.music {
            stop_sound(m);
        }
    }
}

// carrega os sons só depois do primeiro toque/tecla (os navegadores bloqueiam áudio antes disso)
struct Audio {
    s: Option<Sounds>,
}

impl Audio {
    fn new() -> Self {
        Audio { s: None }
    }
    async fn unlock(&mut self) {
        if self.s.is_none() {
            self.s = Some(Sounds::new().await);
        }
    }
    fn ok(&self) -> bool {
        self.s.as_ref().map_or(false, |s| s.music.is_some() && s.click.is_some())
    }
    fn click(&self) {
        if let Some(s) = &self.s {
            s.click();
        }
    }
    fn play_ev(&self, e: Ev) {
        if let Some(s) = &self.s {
            s.play_ev(e);
        }
    }
    fn start_music(&self) {
        if let Some(s) = &self.s {
            s.start_music();
        }
    }
    fn stop_music(&self) {
        if let Some(s) = &self.s {
            s.stop_music();
        }
    }
}

// ---------- interface ----------

// posição do toque/clique neste quadro (se houve)
fn tap_pos() -> Option<Vec2> {
    if is_mouse_button_pressed(MouseButton::Left) {
        let (x, y) = mouse_position();
        return Some(vec2(x, y));
    }
    for t in touches() {
        if t.phase == TouchPhase::Started {
            return Some(t.position);
        }
    }
    None
}

fn text_c(t: &str, cx: f32, y: f32, size: u16, color: Color) {
    let d = measure_text(t, None, size, 1.0);
    draw_text(t, cx - d.width / 2.0, y, size as f32, color);
}

fn ui_button(r: Rect, label: &str, col: Color, tap: Option<Vec2>) -> bool {
    let (mx, my) = mouse_position();
    let hover = r.contains(vec2(mx, my));
    let c = if hover {
        Color::new((col.r + 0.12).min(1.0), (col.g + 0.12).min(1.0), (col.b + 0.12).min(1.0), 1.0)
    } else {
        col
    };
    draw_rectangle(r.x + 3.0, r.y + 5.0, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.35));
    draw_rectangle(r.x, r.y, r.w, r.h, c);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 3.0, Color::new(1.0, 1.0, 1.0, 0.8));
    if !label.is_empty() {
        let size = (r.h * 0.45) as u16;
        let d = measure_text(label, None, size, 1.0);
        draw_text(
            label,
            r.x + (r.w - d.width) / 2.0,
            r.y + r.h / 2.0 + d.height / 2.0,
            size as f32,
            WHITE,
        );
    }
    tap.map_or(false, |p| r.contains(p))
}

// chave azul desenhada pelo código
fn draw_key(cx: f32, cy: f32, s: f32, col: Color, hole: Color) {
    let outline = Color::new(0.02, 0.08, 0.25, 1.0);
    draw_circle(cx - s * 0.25, cy, s * 0.3 + 2.0, outline);
    draw_rectangle(cx - s * 0.05 - 2.0, cy - s * 0.07 - 2.0, s * 0.6 + 4.0, s * 0.14 + 4.0, outline);
    draw_rectangle(cx + s * 0.3 - 2.0, cy - 2.0, s * 0.1 + 4.0, s * 0.22 + 4.0, outline);
    draw_rectangle(cx + s * 0.46 - 2.0, cy - 2.0, s * 0.1 + 4.0, s * 0.17 + 4.0, outline);
    draw_circle(cx - s * 0.25, cy, s * 0.3, col);
    draw_circle(cx - s * 0.25, cy, s * 0.12, hole);
    draw_rectangle(cx - s * 0.05, cy - s * 0.07, s * 0.6, s * 0.14, col);
    draw_rectangle(cx + s * 0.3, cy, s * 0.1, s * 0.22, col);
    draw_rectangle(cx + s * 0.46, cy, s * 0.1, s * 0.17, col);
}

fn fmt_keys(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    out
}

enum DeadChoice {
    Nothing,
    Revive,
    Again,
    Menu,
}

// tela que aparece quando você bate
fn dead_panel(game: &Game, keys: u64, tap: Option<Vec2>) -> DeadChoice {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));

    let pw = (sw - 40.0).min(440.0);
    let ph = 400.0f32.min(sh - 40.0);
    let px = (sw - pw) / 2.0;
    let py = (sh - ph) / 2.0;
    draw_rectangle(px + 4.0, py + 6.0, pw, ph, Color::new(0.0, 0.0, 0.0, 0.4));
    draw_rectangle(px, py, pw, ph, Color::new(0.38, 0.38, 0.4, 1.0));
    draw_rectangle_lines(px, py, pw, ph, 4.0, Color::new(0.2, 0.2, 0.22, 1.0));

    // chaves no canto superior direito do quadro cinza
    let kt = fmt_keys(keys);
    let kd = measure_text(&kt, None, 22, 1.0);
    let kx = px + pw - 16.0 - kd.width;
    draw_text(&kt, kx, py + 34.0, 22.0, WHITE);
    draw_key(kx - 24.0, py + 28.0, 26.0, Color::new(0.5, 0.8, 1.0, 1.0), Color::new(0.38, 0.38, 0.4, 1.0));

    text_c("VOCÊ BATEU!", sw / 2.0, py + 90.0, 44, Color::new(1.0, 0.35, 0.3, 1.0));
    text_c(
        &format!("Pontos: {}    Tempo: {}", game.score(), game.timer_text()),
        sw / 2.0,
        py + 125.0,
        24,
        WHITE,
    );

    // botão azul com a chave azul (custa 1)
    let cost = game.revive_cost;
    let ct = fmt_keys(cost);
    let cd = measure_text(&ct, None, 40, 1.0);
    let total = 58.0 + 8.0 + cd.width;
    let bwid = 200.0f32.max(total + 30.0).min(pw - 20.0);
    let br = Rect::new(sw / 2.0 - bwid / 2.0, py + 150.0, bwid, 90.0);
    let can = keys >= cost;
    let bcol = if can { c3(0.12, 0.35, 0.95) } else { c3(0.3, 0.3, 0.35) };
    let hit = ui_button(br, "", bcol, tap);
    let bx = br.x + br.w / 2.0;
    let by = br.y + br.h / 2.0;
    let start = bx - total / 2.0;
    draw_key(start + 29.0, by, 58.0, Color::new(0.55, 0.85, 1.0, 1.0), bcol);
    draw_text(&ct, start + 66.0, by + 14.0, 40.0, WHITE);
    text_c("ESPAÇO ou toque no botão: renascer", sw / 2.0, py + 265.0, 20, LIGHTGRAY);

    let bw = (pw - 60.0) / 2.0;
    let again = Rect::new(px + 20.0, py + ph - 76.0, bw, 52.0);
    let menu = Rect::new(px + 40.0 + bw, py + ph - 76.0, bw, 52.0);
    let a = ui_button(again, "RECOMEÇAR", c3(0.1, 0.6, 0.3), tap);
    let m = ui_button(menu, "MENU", c3(0.45, 0.2, 0.85), tap);

    if can && (is_key_pressed(KeyCode::Space) || hit) {
        DeadChoice::Revive
    } else if a || is_key_pressed(KeyCode::Enter) {
        DeadChoice::Again
    } else if m || is_key_pressed(KeyCode::Escape) {
        DeadChoice::Menu
    } else {
        DeadChoice::Nothing
    }
}

// boneco(s) girando em 3D (menu e vestiário)
// modo: 0 = jogador e policial, 1 = jogador, 2 = policial, 3 = jogador na prancha
fn draw_preview(mode: usize, ps: usize, cs: usize, bs: usize) {
    let t = get_time() as f32;
    let a = t * 0.6;
    let cam = Camera3D {
        position: vec3(6.0 * a.sin(), 2.4, 6.0 * a.cos()),
        target: vec3(0.0, 0.5, 0.0),
        up: vec3(0.0, 1.0, 0.0),
        fovy: 45.0f32.to_radians(),
        ..Default::default()
    };
    set_camera(&cam);
    draw_cube(vec3(0.0, -0.06, 0.0), vec3(4.0, 0.12, 4.0), None, Color::new(0.08, 0.08, 0.14, 1.0));
    draw_cube_wires(vec3(0.0, -0.06, 0.0), vec3(4.0, 0.12, 4.0), Color::new(0.2, 0.95, 1.0, 1.0));
    let run = t * 3.0;
    let pl = player_skin(ps).1;
    let cl = cop_skin(cs).1;
    match mode {
        0 => {
            draw_runner(-0.9, 0.0, 0.0, run, false, 0.0, &pl, false);
            draw_runner(0.9, 0.0, 0.0, run + 1.0, false, 0.0, &cl, true);
        }
        1 => draw_runner(0.0, 0.0, 0.0, run, false, 0.0, &pl, false),
        2 => draw_runner(0.0, 0.0, 0.0, run, false, 0.0, &cl, true),
        _ => {
            let (_, bm, bg) = board_skin(bs, t);
            let bob = (t * 2.0).sin() * 0.04;
            draw_board(0.0, 0.1 + bob, 0.0, bm, bg);
            draw_runner(0.0, 0.38 + bob, 0.0, run, true, 0.0, &pl, false);
        }
    }
    set_default_camera();
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

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Menu,
    Wardrobe,
    Playing,
}

#[macroquad::main(conf)]
async fn main() {
    rand::srand(macroquad::miniquad::date::now() as u64);
    let tex = Textures::new();
    let mut sounds = Audio::new();

    let mut screen = Screen::Menu;
    let mut tab = 0usize;
    let (mut pskin, mut cskin, mut bskin) = (0usize, 0usize, 0usize);
    let mut keys: u64 = START_KEYS;
    let mut best = 0u32;
    let mut timer_pos = 1usize;
    let mut game = Game::new(0, timer_pos, pskin, cskin, bskin);
    let mut music_on = false;
    let mut fullscreen = false;

    loop {
        let dt = get_frame_time().min(0.05);
        let sw = screen_width();
        let sh = screen_height();

        // botão de tela cheia (toque, clique ou tecla F)
        let fs_btn = fs_button_rect();
        let tap = tap_pos();
        if tap.is_some() || get_last_key_pressed().is_some() {
            sounds.unlock().await;
        }
        let fs_tapped = tap.map_or(false, |p| fs_btn.contains(p));
        if is_key_pressed(KeyCode::F) || fs_tapped {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }
        // toque para a interface (o botão de tela cheia não conta)
        let ui_tap = if fs_tapped { None } else { tap };

        match screen {
            Screen::Menu => {
                draw_sky();
                draw_preview(0, pskin, cskin, bskin);
                text_c("NEON DASH", sw / 2.0, sh * 0.09, 64, c3(0.2, 0.95, 1.0));

                // chaves no canto
                draw_key(34.0, 34.0, 30.0, Color::new(0.5, 0.8, 1.0, 1.0), Color::new(0.05, 0.05, 0.12, 1.0));
                draw_text(&fmt_keys(keys), 62.0, 42.0, 24.0, WHITE);

                let bw = 260.0f32.min(sw - 40.0);
                let play_r = Rect::new((sw - bw) / 2.0, sh * 0.66, bw, 76.0);
                let ward_r = Rect::new((sw - bw) / 2.0, sh * 0.66 + 96.0, bw, 76.0);
                let go = ui_button(play_r, "JOGAR", c3(0.1, 0.6, 0.3), ui_tap);
                let ward = ui_button(ward_r, "VESTIÁRIO", c3(0.45, 0.2, 0.85), ui_tap);
                if sounds.s.is_some() && !sounds.ok() {
                    text_c("Som indisponível neste navegador", sw / 2.0, sh * 0.66 + 200.0, 20, ORANGE);
                }
                if go || is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    sounds.click();
                    game = Game::new(best, timer_pos, pskin, cskin, bskin);
                    screen = Screen::Playing;
                    if !music_on {
                        sounds.start_music();
                        music_on = true;
                    }
                } else if ward {
                    sounds.click();
                    screen = Screen::Wardrobe;
                }
            }

            Screen::Wardrobe => {
                draw_sky();
                draw_preview(
                    match tab {
                        0 => 1,
                        1 => 2,
                        _ => 3,
                    },
                    pskin,
                    cskin,
                    bskin,
                );
                text_c("VESTIÁRIO", sw / 2.0, 40.0, 40, WHITE);

                // abas
                let labels = ["Jogador", "Policial", "Prancha"];
                let tw = (sw - 40.0 - 20.0) / 3.0;
                for i in 0..3 {
                    let r = Rect::new(20.0 + i as f32 * (tw + 10.0), 56.0, tw, 44.0);
                    let col = if tab == i { c3(0.12, 0.35, 0.95) } else { c3(0.25, 0.25, 0.3) };
                    if ui_button(r, labels[i], col, ui_tap) {
                        tab = i;
                        sounds.click();
                    }
                }

                // amostras de skins
                let sel = match tab {
                    0 => pskin,
                    1 => cskin,
                    _ => bskin,
                };
                let top = sh * 0.62;
                let bottom = sh - 80.0;
                let gap = 10.0;
                let cw = (sw - 40.0 - 2.0 * gap) / 3.0;
                let ch = ((bottom - top) - gap) / 2.0;
                for i in 0..6usize {
                    let col = i % 3;
                    let row = i / 3;
                    let r = Rect::new(
                        20.0 + col as f32 * (cw + gap),
                        top + row as f32 * (ch + gap),
                        cw,
                        ch,
                    );
                    let (name, c1, c2, c3_) = skin_info(tab, i);
                    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.1, 0.1, 0.16, 0.92));
                    let border = if sel == i { GOLD } else { Color::new(0.4, 0.4, 0.5, 1.0) };
                    draw_rectangle_lines(r.x, r.y, r.w, r.h, if sel == i { 4.0 } else { 2.0 }, border);
                    let sw3 = (r.w - 24.0) / 3.0;
                    let sh3 = (r.h * 0.45).min(50.0);
                    draw_rectangle(r.x + 8.0, r.y + 10.0, sw3, sh3, c1);
                    draw_rectangle(r.x + 12.0 + sw3, r.y + 10.0, sw3, sh3, c2);
                    draw_rectangle(r.x + 16.0 + 2.0 * sw3, r.y + 10.0, sw3, sh3, c3_);
                    text_c(name, r.x + r.w / 2.0, r.y + r.h - 12.0, 20, WHITE);
                    if ui_tap.map_or(false, |p| r.contains(p)) {
                        match tab {
                            0 => pskin = i,
                            1 => cskin = i,
                            _ => bskin = i,
                        }
                        sounds.click();
                    }
                }

                let back = Rect::new(20.0, sh - 64.0, sw - 40.0, 48.0);
                if ui_button(back, "VOLTAR", c3(0.1, 0.6, 0.3), ui_tap) || is_key_pressed(KeyCode::Escape) {
                    sounds.click();
                    screen = Screen::Menu;
                }
            }

            Screen::Playing => {
                game.update(dt);
                game.draw(&tex);
                best = best.max(game.best);
                timer_pos = game.timer_pos;

                if game.dead {
                    match dead_panel(&game, keys, ui_tap) {
                        DeadChoice::Revive => {
                            keys -= game.revive_cost;
                            game.revive();
                        }
                        DeadChoice::Again => {
                            sounds.click();
                            game = Game::new(best, timer_pos, pskin, cskin, bskin);
                        }
                        DeadChoice::Menu => {
                            sounds.click();
                            if music_on {
                                sounds.stop_music();
                                music_on = false;
                            }
                            screen = Screen::Menu;
                        }
                        DeadChoice::Nothing => {}
                    }
                }

                // sons dos eventos do jogo
                let evs: Vec<Ev> = game.events.drain(..).collect();
                for e in evs {
                    sounds.play_ev(e);
                }
            }
        }

        draw_fs_button(fs_btn, fullscreen);
        next_frame().await;
    }
}
