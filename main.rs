use macroquad::audio::{load_sound_from_bytes, play_sound, stop_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;
use std::cell::Cell;

const LANE_W: f32 = 2.2;
const GRAVITY: f32 = 32.0;
const JUMP_V: f32 = 11.5;
const ROLL_TIME: f32 = 0.65;
const COP_TIME: f32 = 7.0; // segundos que o policial fica atrás de você
const COP_NEAR: f32 = 3.0; // distância do policial quando ele está logo atrás de você
const CAM_BACK: f32 = 12.0; // distância da câmera atrás do jogador

// Velocidade: começa em START_SPEED e sobe SPEED_GAIN por segundo, até MAX_SPEED
const START_SPEED: f32 = 14.0;
const SPEED_GAIN: f32 = 0.06; // quanto a velocidade sobe por segundo (antes era 0.15)
const MAX_SPEED: f32 = 32.0; // (antes era 45)

// Deslizar no celular: o dedo só precisa andar essa fração da tela (menor = mais sensível)
const SWIPE_FRACTION: f32 = 0.04;

// quantidade de mapas
const MAP_COUNT: usize = 5;

// Chaves azuis (999 trilhões ao abrir o jogo)
const START_KEYS: u64 = 999_000_000_000_000;
// Raio (em metros) que é limpo ao renascer ou quando a prancha salva você
const CLEAR_RADIUS: f32 = 30.0;
// Tempo de espera para usar a prancha de novo depois que ela quebra
const BOARD_CD: f32 = 20.0;

// Distância única entre moedas vizinhas (fileiras e arcos), em metros
const COIN_GAP: f32 = 4.0; // OBRIGATÓRIO: 4.0 entre moedas consecutivas (medido ao longo do caminho)
const COIN_Y: f32 = 0.9; // altura da moeda acima da superfície onde o jogador pisa

// ---- trens ----
const TRAIN_LEN: f32 = 9.0; // comprimento do corpo do trem
const TRAIN_H: f32 = 3.0; // altura do trem normal
const TRAIN_H_LOW: f32 = 2.0; // altura do trem baixo (sequências de alturas diferentes)
const RAMP_LEN: f32 = 6.0; // comprimento da rampa (inclinação = altura / comprimento)
const STEP_UP: f32 = 1.2; // maior degrau que o jogador sobe "andando" (só a rampa cabe nisso)
const MOVING_TRAIN_V: f32 = 7.0; // velocidade dos trens que vêm na sua direção
const TRAIN_SPAWN_DIST: f32 = 75.0; // distância (m) em que o trem em movimento aparece

// ---- parede com dois túneis ----
const TUNNEL_LEN: f32 = 12.0; // profundidade da parede / comprimento dos túneis
const TUNNEL_H: f32 = 2.6; // altura livre dentro do túnel
const TUNNEL_OPEN_HALF: f32 = 1.0; // meia-largura de cada abertura
const WALL_H: f32 = 4.2; // altura da parede
const PILLAR_HALF: f32 = 0.1; // meia-largura do pilar entre as aberturas
const TUNNEL_TRAIN_V: f32 = 12.0; // velocidade do trem que sai do túnel das moedas
const TUNNEL_TRIGGER: f32 = 5.0; // o trem surge quando você está a esta distância da saída
const TUNNEL_REACT: f32 = 1.3; // segundos entre você sair do túnel e o trem te alcançar

// ---- geração procedural ----
const REACT_START: f32 = 1.7; // segundos de espaço entre padrões no começo
const REACT_END: f32 = 1.0; // ... na velocidade máxima
const MIN_GAP: f32 = 14.0; // distância mínima entre padrões (m)
const LANE_SMOOTH: f32 = 18.0; // rapidez da troca de pista

// Hitbox do jogador contra obstáculos: meia-largura (m)
const HIT_W: f32 = 0.8; // jogador 0,6 (meia 0,3) + obstáculo 1,0 (meia 0,5)
// Hitbox das moedas: meia-largura, meia-profundidade e folga vertical (m)
const COIN_HIT_X: f32 = 0.5; // jogador 0,6 + moeda 0,4
const COIN_HIT_Z: f32 = 0.5;
const COIN_HIT_Y: f32 = 0.2;
// Raio visual da moeda redonda (m)
const COIN_R: f32 = 0.25;

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
        // asfalto: manchas suaves (sem detalhe fino, para não tremer de longe)
        let asphalt = make_tex(128, 128, |x, y| {
            let n = vnoise(x as f32 / 16.0, y as f32 / 16.0, 1) * 0.6
                + vnoise(x as f32 / 6.0, y as f32 / 6.0, 2) * 0.4;
            let v = 0.68 + n * 0.22;
            Color::new(v, v, (v * 1.03).min(1.0), 1.0)
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

        // calçada: placas com rejunte (suave)
        let pavement = make_tex(64, 64, |x, y| {
            let n = vnoise(x as f32 / 8.0, y as f32 / 8.0, 9) * 0.2;
            let mut v = 0.82 + n;
            if x % 32 < 3 || y % 32 < 3 {
                v = 0.58 + n * 0.3;
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

// cor da névoa do mapa atual (muda em cada mapa)
thread_local! {
    static FOG_RGB: Cell<(f32, f32, f32)> = Cell::new((0.32, 0.18, 0.5));
}

fn set_fog(c: Color) {
    FOG_RGB.with(|x| x.set((c.r, c.g, c.b)));
}

// névoa: objetos distantes se misturam com o horizonte
fn fog(c: Color, cam_z: f32, z: f32) -> Color {
    let d = (cam_z - z).max(0.0);
    let f = (d / 120.0).clamp(0.0, 1.0).powf(1.6);
    let (fr, fg, fb) = FOG_RGB.with(|x| x.get());
    Color::new(
        c.r + (fr - c.r) * f,
        c.g + (fg - c.g) * f,
        c.b + (fb - c.b) * f,
        c.a,
    )
}

// ---------- mapas ----------

struct Theme {
    name: &'static str,
    sky_top: Color,
    sky_bot: Color, // fundo do céu nos menus
    fog: Color,     // cor do horizonte (a névoa se mistura com ela)
    road: Color,
    pave: Color,
    ground: Color,
    line: Color,
    edge: Color,
    orb: Color, // sol / lua
    orb_r: f32,
    fx: u8, // 1 = estrelas, 2 = neve, 3 = brasas
}

fn theme(i: usize) -> Theme {
    match i % MAP_COUNT {
        0 => Theme {
            name: "Cidade Neon",
            sky_top: c3(0.05, 0.02, 0.2),
            sky_bot: c3(0.85, 0.25, 0.6),
            fog: c3(0.32, 0.18, 0.5),
            road: c3(0.24, 0.24, 0.30),
            pave: c3(0.42, 0.40, 0.50),
            ground: c3(0.05, 0.05, 0.09),
            line: c3(0.2, 0.95, 1.0),
            edge: c3(1.0, 0.25, 0.7),
            orb: c3(0.98, 0.96, 0.88),
            orb_r: 22.0,
            fx: 1,
        },
        1 => Theme {
            name: "Deserto",
            sky_top: c3(0.3, 0.5, 0.9),
            sky_bot: c3(1.0, 0.75, 0.4),
            fog: c3(0.92, 0.78, 0.58),
            road: c3(0.36, 0.33, 0.31),
            pave: c3(0.86, 0.72, 0.46),
            ground: c3(0.8, 0.62, 0.36),
            line: c3(1.0, 0.95, 0.7),
            edge: c3(1.0, 0.5, 0.1),
            orb: c3(1.0, 0.95, 0.7),
            orb_r: 32.0,
            fx: 0,
        },
        2 => Theme {
            name: "Floresta",
            sky_top: c3(0.1, 0.3, 0.6),
            sky_bot: c3(0.7, 0.9, 0.7),
            fog: c3(0.55, 0.75, 0.65),
            road: c3(0.28, 0.27, 0.25),
            pave: c3(0.3, 0.55, 0.25),
            ground: c3(0.12, 0.3, 0.12),
            line: c3(1.0, 1.0, 0.8),
            edge: c3(0.9, 0.8, 0.3),
            orb: c3(1.0, 1.0, 0.85),
            orb_r: 26.0,
            fx: 0,
        },
        3 => Theme {
            name: "Neve",
            sky_top: c3(0.35, 0.45, 0.65),
            sky_bot: c3(0.9, 0.95, 1.0),
            fog: c3(0.82, 0.88, 0.95),
            road: c3(0.4, 0.42, 0.48),
            pave: c3(0.92, 0.95, 1.0),
            ground: c3(0.85, 0.9, 0.97),
            line: c3(0.3, 0.7, 1.0),
            edge: c3(0.6, 0.85, 1.0),
            orb: c3(1.0, 1.0, 1.0),
            orb_r: 20.0,
            fx: 2,
        },
        _ => Theme {
            name: "Vulcão",
            sky_top: c3(0.08, 0.02, 0.04),
            sky_bot: c3(0.6, 0.12, 0.05),
            fog: c3(0.35, 0.08, 0.06),
            road: c3(0.15, 0.13, 0.14),
            pave: c3(0.22, 0.14, 0.12),
            ground: c3(0.08, 0.05, 0.05),
            line: c3(1.0, 0.55, 0.1),
            edge: c3(1.0, 0.2, 0.1),
            orb: c3(1.0, 0.3, 0.2),
            orb_r: 26.0,
            fx: 3,
        },
    }
}

fn shade(c: Color, k: f32) -> Color {
    Color::new(c.r * k, c.g * k, c.b * k, 1.0)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Low,   // barreira baixa: pular
    High,  // placa alta: rolar
    Train, // vagão: trocar de faixa (ou subir pela rampa / correr no teto)
    Wall,  // parede com duas aberturas (túneis curtos lado a lado)
}

struct Obstacle {
    lane: i32, // Wall: faixa da abertura da esquerda (as aberturas são lane e lane+1)
    z: f32,    // centro do corpo
    kind: Kind,
    h: f32,    // altura do teto do trem
    ramp: bool, // tem rampa na frente (lado +z, de onde o jogador chega)
    vz: f32,   // velocidade própria (+z = vindo na direção do jogador)
}

impl Obstacle {
    fn new(lane: i32, z: f32, kind: Kind) -> Self {
        Obstacle { lane, z, kind, h: TRAIN_H, ramp: false, vz: 0.0 }
    }
    fn len(&self) -> f32 {
        match self.kind {
            Kind::Train => TRAIN_LEN,
            Kind::Wall => TUNNEL_LEN,
            _ => 0.6,
        }
    }
    fn ramp_len(&self) -> f32 {
        if self.ramp { RAMP_LEN } else { 0.0 }
    }
    // extremidade de trás (menor z) e da frente (maior z, inclui a rampa)
    fn lo(&self) -> f32 {
        self.z - self.len() / 2.0
    }
    fn hi(&self) -> f32 {
        self.z + self.len() / 2.0 + self.ramp_len()
    }
    // altura da rampa em z (0 na base, h no topo)
    fn ramp_h(&self, z: f32) -> f32 {
        let front = self.z + self.len() / 2.0;
        (self.h * (1.0 - (z - front) / RAMP_LEN)).clamp(0.0, self.h)
    }
    fn wall_solid(&self) -> i32 {
        if self.lane == 0 { 2 } else { 0 }
    }
    fn wall_pillar_x(&self) -> f32 {
        (lane_x(self.lane) + lane_x(self.lane + 1)) / 2.0
    }
    // dentro de uma das duas aberturas do túnel?
    fn in_opening(&self, x: f32) -> bool {
        (x - lane_x(self.lane)).abs() <= 0.8 || (x - lane_x(self.lane + 1)).abs() <= 0.8
    }
    // faixa lateral em que o obstáculo bloqueia (qualquer altura)
    fn x_blocks(&self, x: f32) -> bool {
        match self.kind {
            Kind::Wall => {
                (x - lane_x(self.wall_solid())).abs() < LANE_W / 2.0 + 0.3
                    || (x - self.wall_pillar_x()).abs() < PILLAR_HALF + 0.3
            }
            _ => (x - lane_x(self.lane)).abs() <= HIT_W,
        }
    }
    // sobreposição em z (shift = deslocamento do obstáculo, usado para o quadro anterior)
    fn z_in(&self, z: f32, shift: f32) -> bool {
        z >= self.lo() + shift - 0.3 && z <= self.hi() + shift + 0.3
    }
    // a altura do jogador bate? (x e z já conferidos)
    fn y_blocks(&self, z: f32, py: f32, top: f32) -> bool {
        match self.kind {
            Kind::Low => py < 1.0,
            Kind::High => top > 1.1,
            Kind::Train => {
                let roof = if self.ramp && z > self.z + self.len() / 2.0 { self.ramp_h(z) } else { self.h };
                py < roof - 0.35
            }
            Kind::Wall => py < WALL_H,
        }
    }
    // superfície onde se pode pisar (teto do trem ou rampa) na posição (x, z)
    fn surface(&self, x: f32, z: f32) -> Option<f32> {
        if self.kind != Kind::Train || (x - lane_x(self.lane)).abs() > 0.95 {
            return None;
        }
        let front = self.z + self.len() / 2.0;
        if z >= self.lo() && z <= front {
            Some(self.h)
        } else if self.ramp && z > front && z <= front + RAMP_LEN {
            Some(self.ramp_h(z))
        } else {
            None
        }
    }
    fn in_zone(&self, zc: f32, r: f32) -> bool {
        self.hi() >= zc - r && self.lo() <= zc + r
    }
}

struct Coin {
    x: f32,
    z: f32,
    y: f32,
    taken: bool,
}

// obstáculo que só aparece quando o jogador chega perto (trens em movimento)
struct Pending {
    trigger_z: f32,
    lane: i32,
    z: f32,
    vz: f32,
    h: f32,
    exit_z: Option<f32>, // Some = trem da saída do túnel (posição calculada na hora)
}

// (id, dificuldade mínima 0..1, peso)
const PATTERNS: [(u8, f32, f32); 15] = [
    (0, 0.0, 3.0),   // trem numa pista, caminho livre
    (1, 0.0, 1.5),   // dois trens, uma pista livre
    (2, 0.0, 1.5),   // três barreiras baixas
    (3, 0.0, 2.0),   // arco de moedas sobre barreira
    (4, 0.0, 1.5),   // duas placas altas
    (5, 0.0, 1.0),   // só moedas (troca de pista)
    (6, 0.05, 1.5),  // barreiras baixas em sequência
    (7, 0.1, 1.2),   // placas altas em sequência
    (8, 0.1, 2.5),   // trem com rampa (teto com moedas / livre)
    (9, 0.15, 1.5),  // trem + barreira
    (10, 0.12, 2.0), // parede com dois túneis, moedas e trem na mesma passagem
    (11, 0.2, 1.5),  // pulo e depois deslizada
    (12, 0.25, 2.0), // trem com rampa seguido de outro trem
    (13, 0.3, 1.5),  // trem em movimento vindo na sua direção
    (14, 0.4, 1.5),  // rampas em pistas diferentes
];

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
    Lane,
    Land,
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
    cop_y: f32,
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
    map: usize,
    speed_t: f32,        // tempo que a velocidade usa (o cronômetro pode parar, ele não)
    timer_stopped: bool, // o cronômetro para quando você pega uma moeda
    cop_fall_v: f32,     // velocidade com que o policial fica para trás
    air_jump: bool,      // bateu no ar: ainda dá pra pular uma vez
    grounded: bool,      // pisando em algo (chão, teto de trem ou rampa)
    floor_y: f32,        // altura da superfície debaixo do jogador
    pending: Vec<Pending>,
    recent: Vec<u8>,     // últimos padrões (evita repetição)
}

impl Game {
    fn new(best: u32, timer_pos: usize, pskin: usize, cskin: usize, bskin: usize, map: usize) -> Self {
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
            cop_y: 0.0,
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
            map,
            speed_t: 0.0,
            timer_stopped: false,
            cop_fall_v: 0.0,
            air_jump: false,
            grounded: true,
            floor_y: 0.0,
            pending: vec![],
            recent: vec![],
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
            1.8
        }
    }

    fn fill_world(&mut self) {
        while self.next_z > self.z - 110.0 {
            self.spawn_row();
        }
    }

    // ---------- geração procedural ----------

    // 0 (começo) .. 1 (velocidade máxima)
    fn difficulty(&self) -> f32 {
        ((self.speed - START_SPEED) / (MAX_SPEED - START_SPEED)).clamp(0.0, 1.0)
    }

    // espaço entre padrões: tempo de reação que encolhe com a dificuldade
    // (sempre dá tempo de trocar até 2 pistas e pular/deslizar)
    fn reaction_gap(&self) -> f32 {
        let react = REACT_START + (REACT_END - REACT_START) * self.difficulty();
        (self.speed * react * rand::gen_range(1.0f32, 1.25f32)).max(MIN_GAP)
    }

    // moedas ao longo de um caminho 3D (x, y, z). A distância entre moedas consecutivas
    // é COIN_GAP medida AO LONGO do caminho (retas, arcos, rampas, tetos).
    fn coin_path(&mut self, pts: &[(f32, f32, f32)]) {
        if pts.is_empty() {
            return;
        }
        self.coins.push(Coin { x: pts[0].0, y: pts[0].1, z: pts[0].2, taken: false });
        let mut since = 0.0f32; // quanto já andou desde a última moeda
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let seg = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2) + (b.2 - a.2).powi(2)).sqrt();
            if seg < 1e-6 {
                continue;
            }
            let mut pos = COIN_GAP - since;
            while pos <= seg + 1e-3 {
                let t = (pos / seg).min(1.0);
                self.coins.push(Coin {
                    x: a.0 + (b.0 - a.0) * t,
                    y: a.1 + (b.1 - a.1) * t,
                    z: a.2 + (b.2 - a.2) * t,
                    taken: false,
                });
                pos += COIN_GAP;
            }
            since = seg - (pos - COIN_GAP);
        }
    }

    fn coin_line(&mut self, lane: i32, start: f32, n: i32) {
        if n <= 0 {
            return;
        }
        let x = lane_x(lane);
        self.coin_path(&[(x, COIN_Y, start), (x, COIN_Y, start - (n - 1) as f32 * COIN_GAP)]);
    }

    // arco de moedas sobre um obstáculo baixo centrado em c
    fn arch(&mut self, lane: i32, c: f32) {
        let x = lane_x(lane);
        let half = 5.0f32;
        let peak = 1.5f32;
        let steps = 400usize;
        let mut pts: Vec<(f32, f32, f32)> = Vec::with_capacity(steps + 1);
        for k in 0..=steps {
            let t = k as f32 / steps as f32;
            pts.push((x, COIN_Y + peak * (std::f32::consts::PI * t).sin(), c + half - 2.0 * half * t));
        }
        self.coin_path(&pts);
    }

    // caminho de moedas que sobe a rampa e segue pelo teto do trem
    fn ramp_pts(&self, lane: i32, s: f32, h: f32, approach: bool) -> Vec<(f32, f32, f32)> {
        let x = lane_x(lane);
        let mut v = Vec::new();
        if approach {
            v.push((x, COIN_Y, s + 2.0));
        }
        v.push((x, COIN_Y, s));
        v.push((x, h + COIN_Y, s - RAMP_LEN));
        v.push((x, h + COIN_Y, s - RAMP_LEN - TRAIN_LEN + 1.0));
        v
    }

    // trem com rampa: a base da rampa começa em z = s
    fn ramp_train(&mut self, lane: i32, s: f32, h: f32, coins: bool) {
        let mut t = Obstacle::new(lane, s - RAMP_LEN - TRAIN_LEN / 2.0, Kind::Train);
        t.h = h;
        t.ramp = true;
        self.obstacles.push(t);
        if coins {
            let pts = self.ramp_pts(lane, s, h, true);
            self.coin_path(&pts);
        }
    }

    // trens que só aparecem quando o jogador chega perto
    fn spawn_due(&mut self) {
        let mut i = 0;
        while i < self.pending.len() {
            if self.z <= self.pending[i].trigger_z {
                let p = self.pending.remove(i);
                let z = match p.exit_z {
                    Some(ez) => {
                        // trem do túnel: aparece além da saída, na mesma pista das moedas, e vem
                        // na sua direção. A distância é calculada para o encontro acontecer
                        // TUNNEL_REACT segundos depois de você sair do túnel (dá tempo de desviar).
                        let d = (self.z - ez).max(0.5);
                        let gap = (self.speed + p.vz) * (d / self.speed + TUNNEL_REACT) - d;
                        ez - gap - TRAIN_LEN / 2.0
                    }
                    None => p.z,
                };
                let mut t = Obstacle::new(p.lane, z, Kind::Train);
                t.h = p.h;
                t.vz = p.vz;
                self.obstacles.push(t);
            } else {
                i += 1;
            }
        }
    }

    // altura da superfície sob (x, z) que o jogador alcança a partir de y
    fn floor_at(&self, x: f32, z: f32, y: f32) -> f32 {
        let mut f = 0.0f32;
        for o in &self.obstacles {
            if let Some(h) = o.surface(x, z) {
                if h <= y + STEP_UP && h > f {
                    f = h;
                }
            }
        }
        f
    }

    fn spawn_row(&mut self) {
        let s = self.next_z;
        let d = self.difficulty();
        // escolhe um padrão da biblioteca, evitando repetir os últimos
        let mut pick = 0u8;
        for _ in 0..8 {
            let total: f32 = PATTERNS.iter().filter(|p| d >= p.1).map(|p| p.2).sum();
            let mut r = rand::gen_range(0.0f32, total);
            let mut cand = 0u8;
            for p in PATTERNS.iter().filter(|p| d >= p.1) {
                if r < p.2 {
                    cand = p.0;
                    break;
                }
                r -= p.2;
            }
            pick = cand;
            if !self.recent.contains(&cand) {
                break;
            }
        }
        self.recent.push(pick);
        if self.recent.len() > 3 {
            self.recent.remove(0);
        }
        let used = match pick {
            0 => self.pat_train_side(s),
            1 => self.pat_two_trains(s),
            2 => self.pat_low_triple(s),
            3 => self.pat_arch(s),
            4 => self.pat_high_pair(s),
            6 => self.pat_low_seq(s),
            7 => self.pat_high_seq(s),
            8 => self.pat_ramp(s),
            9 => self.pat_train_barrier(s),
            10 => self.pat_tunnel(s),
            11 => self.pat_jump_roll(s),
            12 => self.pat_ramp_then_train(s),
            13 => self.pat_moving(s),
            14 => self.pat_two_ramps(s),
            _ => self.pat_coin_breather(s),
        };
        self.next_z = s - used - self.reaction_gap();
    }

    // Todos os padrões deixam pelo menos uma saída livre/viável e começam em z = s.
    // Retornam o comprimento ocupado (o espaço de reação é somado depois).

    fn pat_train_side(&mut self, s: f32) -> f32 {
        let lane = rand::gen_range(0, 3);
        let free = (lane + 1 + rand::gen_range(0, 2)) % 3;
        let mut t = Obstacle::new(lane, s - TRAIN_LEN / 2.0, Kind::Train);
        if rand::gen_range(0, 3) == 0 {
            t.h = TRAIN_H_LOW;
        }
        self.obstacles.push(t);
        if rand::gen_range(0, 2) == 0 {
            self.coin_line(free, s, 6);
        } else {
            // moedas antes do trem apontam o caminho livre
            let (xl, xf) = (lane_x(lane), lane_x(free));
            self.coin_path(&[(xl, COIN_Y, s + 9.0), (xf, COIN_Y, s + 1.5), (xf, COIN_Y, s - 14.0)]);
        }
        20.0
    }

    fn pat_two_trains(&mut self, s: f32) -> f32 {
        let free = rand::gen_range(0, 3);
        for l in 0..3 {
            if l != free {
                let mut t = Obstacle::new(l, s - TRAIN_LEN / 2.0, Kind::Train);
                if rand::gen_range(0, 4) == 0 {
                    t.h = TRAIN_H_LOW;
                }
                self.obstacles.push(t);
            }
        }
        self.coin_line(free, s, 6);
        20.0
    }

    fn pat_low_triple(&mut self, s: f32) -> f32 {
        for l in 0..3 {
            self.obstacles.push(Obstacle::new(l, s, Kind::Low));
        }
        2.0
    }

    fn pat_arch(&mut self, s: f32) -> f32 {
        let lane = rand::gen_range(0, 3);
        self.obstacles.push(Obstacle::new(lane, s, Kind::Low));
        self.arch(lane, s);
        6.0
    }

    fn pat_high_pair(&mut self, s: f32) -> f32 {
        let open = rand::gen_range(0, 3);
        for l in 0..3 {
            if l != open {
                self.obstacles.push(Obstacle::new(l, s, Kind::High));
            }
        }
        self.coin_line(open, s, 4);
        12.0
    }

    fn pat_coin_breather(&mut self, s: f32) -> f32 {
        let a = rand::gen_range(0, 3);
        let b = rand::gen_range(0, 3);
        self.coin_path(&[(lane_x(a), COIN_Y, s), (lane_x(b), COIN_Y, s - 14.0), (lane_x(b), COIN_Y, s - 28.0)]);
        28.0
    }

    // barreiras baixas em sequência (pulos encadeados, com arco de moedas em cada uma)
    fn pat_low_seq(&mut self, s: f32) -> f32 {
        let lane = rand::gen_range(0, 3);
        let sp = self.speed * 0.72 + 6.0; // tempo de um pulo + folga
        for k in 0..3 {
            let z = s - k as f32 * sp;
            self.obstacles.push(Obstacle::new(lane, z, Kind::Low));
            self.arch(lane, z);
        }
        2.0 * sp + 6.0
    }

    // placas altas em sequência: a saída livre da segunda é sempre a pista vizinha da primeira
    fn pat_high_seq(&mut self, s: f32) -> f32 {
        let free1 = rand::gen_range(0, 3);
        let free2 = if free1 == 1 {
            if rand::gen_range(0, 2) == 0 { 0 } else { 2 }
        } else {
            1
        };
        let sp = self.speed + 4.0;
        for l in 0..3 {
            if l != free1 {
                self.obstacles.push(Obstacle::new(l, s, Kind::High));
            }
            if l != free2 {
                self.obstacles.push(Obstacle::new(l, s - sp, Kind::High));
            }
        }
        self.coin_line(free1, s, 3);
        self.coin_line(free2, s - sp, 3);
        sp + 8.0
    }

    // pulo e depois deslizada: barreiras baixas nas 3 pistas, placas altas nas 3 logo depois
    fn pat_jump_roll(&mut self, s: f32) -> f32 {
        let d = self.speed + 4.0; // pulo (~0,72 s) + aterrissagem + folga para rolar
        for l in 0..3 {
            self.obstacles.push(Obstacle::new(l, s, Kind::Low));
            self.obstacles.push(Obstacle::new(l, s - d, Kind::High));
        }
        self.arch(1, s);
        d + 2.0
    }

    // trem + barreira baixa em pistas diferentes; a terceira pista fica livre
    fn pat_train_barrier(&mut self, s: f32) -> f32 {
        let a = rand::gen_range(0, 3);
        let b = (a + 1 + rand::gen_range(0, 2)) % 3;
        let c = 3 - a - b;
        self.obstacles.push(Obstacle::new(a, s - TRAIN_LEN / 2.0, Kind::Train));
        self.obstacles.push(Obstacle::new(b, s - 3.0, Kind::Low));
        self.coin_line(c, s, 5);
        20.0
    }

    // trem com rampa: teto com moedas ou livre
    fn pat_ramp(&mut self, s: f32) -> f32 {
        let lane = rand::gen_range(0, 3);
        let h = if rand::gen_range(0, 3) == 0 { TRAIN_H_LOW } else { TRAIN_H };
        let coins = rand::gen_range(0, 3) != 0;
        self.ramp_train(lane, s, h, coins);
        RAMP_LEN + TRAIN_LEN
    }

    // trem com rampa seguido de outro trem colado (alturas podem diferir: pular de teto em teto)
    fn pat_ramp_then_train(&mut self, s: f32) -> f32 {
        let lane = rand::gen_range(0, 3);
        let h1 = if rand::gen_range(0, 3) == 0 { TRAIN_H_LOW } else { TRAIN_H };
        let h2 = if self.difficulty() >= 0.4 {
            if rand::gen_range(0, 2) == 0 { TRAIN_H_LOW } else { TRAIN_H }
        } else {
            h1
        };
        let x = lane_x(lane);
        let lo1 = s - RAMP_LEN - TRAIN_LEN;
        let mut a = Obstacle::new(lane, s - RAMP_LEN - TRAIN_LEN / 2.0, Kind::Train);
        a.h = h1;
        a.ramp = true;
        self.obstacles.push(a);
        let mut b = Obstacle::new(lane, lo1 - 0.5 - TRAIN_LEN / 2.0, Kind::Train);
        b.h = h2;
        self.obstacles.push(b);
        // um caminho só: rampa -> teto 1 -> teto 2 (espaçamento contínuo de COIN_GAP)
        let mut pts = self.ramp_pts(lane, s, h1, true);
        pts.pop();
        pts.push((x, h1 + COIN_Y, lo1 + 0.5));
        pts.push((x, h2 + COIN_Y, lo1 - 1.0));
        pts.push((x, h2 + COIN_Y, lo1 - 0.5 - TRAIN_LEN + 1.0));
        self.coin_path(&pts);
        RAMP_LEN + 2.0 * TRAIN_LEN + 0.5
    }

    // rampas em pistas diferentes, uma depois da outra
    fn pat_two_ramps(&mut self, s: f32) -> f32 {
        let l1 = rand::gen_range(0, 3);
        let l2 = (l1 + 1 + rand::gen_range(0, 2)) % 3;
        let sep = self.speed * 0.9 + 4.0; // cair do teto + trocar de pista
        let s2 = s - (RAMP_LEN + TRAIN_LEN) - sep;
        self.ramp_train(l1, s, TRAIN_H, true);
        self.ramp_train(l2, s2, TRAIN_H_LOW, true);
        2.0 * (RAMP_LEN + TRAIN_LEN) + sep
    }

    // trem que aparece à frente e vem na sua direção; as outras pistas ficam livres.
    // O padrão é empurrado 30 m para frente para o trem não cruzar o padrão anterior.
    fn pat_moving(&mut self, s0: f32) -> f32 {
        let s = s0 - 30.0;
        let lane = rand::gen_range(0, 3);
        let free = (lane + 1 + rand::gen_range(0, 2)) % 3;
        self.pending.push(Pending {
            trigger_z: s + TRAIN_SPAWN_DIST,
            lane,
            z: s - TRAIN_LEN / 2.0,
            vz: MOVING_TRAIN_V,
            h: TRAIN_H,
            exit_z: None,
        });
        self.coin_line(free, s, 6);
        50.0
    }

    // PAREDE COM DOIS TÚNEIS: uma parede larga, duas aberturas lado a lado (faixas a e a+1),
    // pilar entre elas, teto e laterais. Uma passagem tem moedas; o trem fica ligado
    // à MESMA passagem (mesma pista) e aparece quando você chega à saída. A outra passagem
    // nunca recebe esse trem.
    fn pat_tunnel(&mut self, s: f32) -> f32 {
        let a = rand::gen_range(0, 2);
        let c = a + rand::gen_range(0, 2); // pista das moedas (e do trem)
        let mut w = Obstacle::new(a, s - TUNNEL_LEN / 2.0, Kind::Wall);
        w.h = WALL_H;
        self.obstacles.push(w);
        let x = lane_x(c);
        self.coin_path(&[(x, COIN_Y, s + 8.0), (x, COIN_Y, s - TUNNEL_LEN - 1.0)]);
        let exit_z = s - TUNNEL_LEN;
        self.pending.push(Pending {
            trigger_z: exit_z + TUNNEL_TRIGGER,
            lane: c,
            z: 0.0,
            vz: TUNNEL_TRAIN_V,
            h: TRAIN_H,
            exit_z: Some(exit_z),
        });
        TUNNEL_LEN + 60.0
    }

    fn move_lane(&mut self, d: i32) {
        let n = (self.lane + d).clamp(0, 2);
        if n != self.lane {
            self.lane = n;
            self.events.push(Ev::Lane);
        }
    }

    fn jump(&mut self) {
        // no chão pula normal; se bateu no ar, ainda dá pra pular uma vez
        if self.grounded || self.air_jump {
            self.air_jump = false;
            self.vy = JUMP_V;
            self.roll_t = 0.0;
            self.events.push(Ev::Jump);

        }
    }

    fn roll(&mut self) {
        // só rola de novo depois que terminar de rolar
        if self.roll_t > 0.0 {
            return;
        }
        self.roll_t = ROLL_TIME;
        self.events.push(Ev::Roll);
        if !self.grounded {
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
        self.obstacles.retain(|o| !o.in_zone(zc, CLEAR_RADIUS));
        self.pending.retain(|p| (p.trigger_z - zc).abs() > CLEAR_RADIUS);
        self.revive_cost = self.revive_cost.saturating_mul(2);
        self.dead = false;
        self.invuln = 2.5;
        self.roll_t = 0.0;
        self.vy = 0.0;
        self.y = 0.0;
        self.grounded = true;
        self.events.push(Ev::Revive);
    }

    fn handle_input(&mut self) {
        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
            self.move_lane(-1);
        }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
            self.move_lane(1);
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

        // deslizar na tela (celular): reage assim que o dedo anda um pouquinho
        let thr = (screen_width().min(screen_height()) * SWIPE_FRACTION).max(10.0);
        for t in touches() {
            match t.phase {
                TouchPhase::Started => self.swipe_start = Some(t.position),
                TouchPhase::Moved | TouchPhase::Stationary => {
                    if let Some(s) = self.swipe_start {
                        let d = t.position - s;
                        if d.length() > thr {
                            if d.x.abs() > d.y.abs() {
                                self.move_lane(if d.x > 0.0 { 1 } else { -1 });
                            } else if d.y < 0.0 {
                                self.jump();
                            } else {
                                self.roll();
                            }
                            // novo ponto de partida: dá pra encadear movimentos sem soltar o dedo
                            self.swipe_start = Some(t.position);
                        }
                    }
                }
                _ => self.swipe_start = None,
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
        self.speed_t += dt;
        if !self.timer_stopped {
            self.time += dt; // o cronômetro para quando você pega a primeira moeda
        }
        self.speed = (START_SPEED + self.speed_t * SPEED_GAIN).min(MAX_SPEED);
        self.z -= self.speed * dt;
        self.dist += self.speed * dt;
        self.run_t += dt * self.speed * 0.5;

        // troca de faixa suave
        let tx = lane_x(self.lane);
        self.x += (tx - self.x) * (1.0 - (-LANE_SMOOTH * dt).exp());

        // trens em movimento e trens que só aparecem quando você chega perto
        for o in self.obstacles.iter_mut() {
            o.z += o.vz * dt;
        }
        self.spawn_due();

        // física vertical com superfícies: chão, teto de trem, rampa (sem teletransporte)
        let fl = self.floor_at(self.x, self.z, self.y);
        if self.vy > 0.0 || self.y > fl + 0.001 {
            let prev_y = self.y;
            self.vy -= GRAVITY * dt;
            self.y += self.vy * dt;
            let fl2 = self.floor_at(self.x, self.z, prev_y);
            if self.y <= fl2 && self.vy <= 0.0 {
                if self.vy < -6.0 {
                    self.events.push(Ev::Land);
                }
                self.y = fl2;
                self.vy = 0.0;
                self.air_jump = false;
            }
        } else {
            // subindo a rampa ou correndo no teto: acompanha a superfície
            self.y = fl;
            self.vy = 0.0;
        }
        // teto baixo do túnel
        let hh = self.height();
        let mut cap: Option<f32> = None;
        for o in &self.obstacles {
            if o.kind == Kind::Wall && self.z >= o.lo() && self.z <= o.hi() && o.in_opening(self.x) {
                cap = Some(TUNNEL_H - hh);
            }
        }
        if let Some(c) = cap {
            if self.y > c {
                self.y = c;
                if self.vy > 0.0 {
                    self.vy = 0.0;
                }
            }
        }
        let fl3 = self.floor_at(self.x, self.z, self.y);
        self.floor_y = fl3;
        self.grounded = self.vy <= 0.0 && self.y <= fl3 + 0.02;
        if self.roll_t > 0.0 {
            self.roll_t -= dt;
        }

        // o policial copia a altura do jogador (pula, cai e rola junto, sem física própria)
        self.cop_y = self.y;

        // policial e prancha
        if self.invuln > 0.0 {
            self.invuln -= dt;
        }
        if self.board_cd > 0.0 {
            self.board_cd -= dt;
        }
        if self.cop_t > 0.0 {
            self.cop_t -= dt;
            // policial chega correndo e fica logo atrás de você
            self.cop_fall_v = 0.0;
            self.cop_gap += (COP_NEAR - self.cop_gap) * (1.0 - (-3.0 * dt).exp());
        } else {
            // acabou o tempo: ele vai ficando para trás, tentando correr, cada vez mais longe
            self.cop_fall_v += 7.0 * dt;
            self.cop_gap = (self.cop_gap + self.cop_fall_v * dt).min(60.0);
        }
        self.cop_x += (self.x - self.cop_x) * (1.0 - (-8.0 * dt).exp());

        self.fill_world();
        self.obstacles.retain(|o| o.lo() < self.z + 15.0);
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
                if !o.x_blocks(px) || !o.z_in(pz, 0.0) {
                    continue;
                }
                if !o.y_blocks(pz, py, top) {
                    continue;
                }
                // já estava alinhado na faixa e ainda não encostava = de frente
                let was_aligned = o.x_blocks(prev_x);
                let was_in_z = o.z_in(prev_z, -o.vz * dt);
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
            self.obstacles.retain(|o| !o.in_zone(zc, CLEAR_RADIUS));
            self.pending.retain(|p| (p.trigger_z - zc).abs() > CLEAR_RADIUS);
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
                self.cop_gap = 6.0;
                if !self.grounded {
                    self.air_jump = true; // bateu no ar: ainda dá pra pular
                }
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
            if (px - c.x).abs() < COIN_HIT_X
                && (c.z - pz).abs() < COIN_HIT_Z
                && c.y > py - COIN_HIT_Y
                && c.y < top + COIN_HIT_Y
            {
                c.taken = true;
                got += 1;
            }
        }
        if got > 0 {
            self.events.push(Ev::Coin);
            self.timer_stopped = true; // pegou moeda: o cronômetro para
        }
        self.coins_got += got;
    }

    // ---------- desenho ----------

    fn draw(&self, tex: &Textures) {
        let th = theme(self.map);
        set_fog(th.fog);
        draw_sky(&th, true);

        // câmera mais alta e mais afastada, para dar pra ver o policial atrás de você
        let cam = Camera3D {
            position: vec3(self.x * 0.55, 6.6 + self.y * 0.3, self.z + CAM_BACK),
            target: vec3(self.x * 0.45, 0.6 + self.y * 0.2, self.z - 8.0),
            up: vec3((lane_x(self.lane) - self.x) * 0.03, 1.0, 0.0),
            fovy: 52.0f32.to_radians(),
            ..Default::default()
        };
        set_camera(&cam);

        self.draw_world(tex);
        self.draw_obstacles(tex);
        self.draw_coins();
        self.draw_player();
        self.draw_cop();

        set_default_camera();
        draw_particles(&th);
        self.draw_hud();
    }

    fn draw_world(&self, t: &Textures) {
        let th = theme(self.map);
        let road_w = 3.0 * LANE_W;
        let cz = self.z + CAM_BACK; // posição da câmera (para a névoa)

        // chão, pista, calçadas e faixas em blocos de 4 m. Cada camada fica numa altura
        // bem diferente da outra (nada na mesma altura), assim o chão não pisca.
        let base = (self.z / 4.0).floor() * 4.0;
        for i in -3..29 {
            let zz = base - i as f32 * 4.0;
            let idx = (zz / 4.0) as i32;
            let k = if idx.rem_euclid(2) == 0 { 1.0 } else { 0.9 };
            // textura só perto: de longe ela fica tremendo, então vira cor lisa
            let near = cz - zz < 26.0;

            // chão largo por baixo de tudo
            draw_cube(
                vec3(0.0, -0.14, zz),
                vec3(road_w + 60.0, 0.2, 4.0),
                None,
                fog(shade(th.ground, k), cz, zz),
            );
            // asfalto
            let (rt, rk) = if near { (Some(&t.asphalt), k) } else { (None, k * 0.78) };
            draw_cube(
                vec3(0.0, 0.0, zz),
                vec3(road_w, 0.04, 4.0),
                rt,
                fog(shade(th.road, rk), cz, zz),
            );
            // calçadas
            let (pt, pk) = if near && self.map == 0 {
                (Some(&t.pavement), k)
            } else {
                (None, k * if self.map == 0 { 0.87 } else { 1.0 })
            };
            for sx in [-road_w / 2.0 - 1.5, road_w / 2.0 + 1.5] {
                draw_cube(
                    vec3(sx, 0.1, zz),
                    vec3(3.0, 0.2, 4.0),
                    pt,
                    fog(shade(th.pave, pk), cz, zz),
                );
            }
            // divisórias de faixa e bordas da pista
            for sx in [-LANE_W / 2.0, LANE_W / 2.0] {
                draw_cube(vec3(sx, 0.07, zz), vec3(0.07, 0.1, 4.0), None, fog(th.line, cz, zz));
            }
            for sx in [-road_w / 2.0 + 0.06, road_w / 2.0 - 0.06] {
                draw_cube(vec3(sx, 0.07, zz), vec3(0.1, 0.1, 4.0), None, fog(th.edge, cz, zz));
            }
        }

        // cenário dos dois lados (muda em cada mapa)
        let base_i = (self.z / 6.0).floor() as i32;
        for i in -1..21 {
            let idx = base_i - i;
            let zz = idx as f32 * 6.0;
            for (side, sx) in [(0, -10.0f32), (1, 10.0f32)] {
                let seed = idx.wrapping_mul(7919).wrapping_add(side * 104729);
                draw_scenery(self.map, sx, zz, seed, cz, t);
            }
            // postes de luz (só na cidade)
            if self.map == 0 && idx.rem_euclid(2) == 0 {
                for sign in [-1.0f32, 1.0f32] {
                    let lx = sign * (road_w / 2.0 + 0.4);
                    let pole = fog(Color::new(0.2, 0.2, 0.25, 1.0), cz, zz);
                    draw_cube(vec3(lx, 2.2, zz), vec3(0.12, 4.4, 0.12), None, pole);
                    draw_cube(vec3(lx - sign * 0.5, 4.4, zz), vec3(1.0, 0.1, 0.1), None, pole);
                    draw_sphere(
                        vec3(lx - sign * 0.95, 4.35, zz),
                        0.18,
                        None,
                        fog(Color::new(1.0, 0.92, 0.65, 1.0), cz, zz),
                    );
                }
            }
        }
    }

    fn draw_obstacles(&self, t: &Textures) {
        let cz = self.z + CAM_BACK;
        let blink = (get_time() * 6.0).sin() > 0.0;
        for o in &self.obstacles {
            if o.lo() > cz + 5.0 || o.hi() < self.z - 140.0 {
                continue;
            }
            let x = lane_x(o.lane);
            match o.kind {
                Kind::Low => {
                    // barreira de concreto com faixa de alerta
                    let conc = fog(Color::new(0.9, 0.9, 0.95, 1.0), cz, o.z);
                    draw_cube(vec3(x, 0.25, o.z), vec3(1.0, 0.5, 0.6), Some(&t.concrete), conc);
                    draw_cube(vec3(x, 0.75, o.z), vec3(0.8, 0.5, 0.4), Some(&t.concrete), conc);
                    draw_cube(
                        vec3(x, 0.7, o.z + 0.21),
                        vec3(0.82, 0.24, 0.04),
                        Some(&t.hazard),
                        fog(WHITE, cz, o.z),
                    );
                    let lamp = if blink { ORANGE } else { Color::new(0.3, 0.15, 0.05, 1.0) };
                    draw_sphere(vec3(x, 0.96, o.z), 0.08, None, fog(lamp, cz, o.z));
                    draw_cube_wires(vec3(x, 0.5, o.z), vec3(1.0, 1.0, 0.6), Color::new(0.0, 0.0, 0.0, 0.5));
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
                    let h = o.h;
                    let body = fog(Color::new(0.62, 0.38, 1.0, 1.0), cz, o.z);
                    draw_cube(vec3(x, h / 2.0, o.z), vec3(1.9, h, len), Some(&t.metal), body);
                    // teto com ventilações
                    draw_cube(vec3(x, h + 0.05, o.z), vec3(1.7, 0.12, len - 0.4), Some(&t.metal), fog(Color::new(0.75, 0.75, 0.8, 1.0), cz, o.z));
                    for k in 0..3 {
                        draw_cube(
                            vec3(x, h + 0.2, o.z - len / 2.0 + 1.5 + k as f32 * 3.0),
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
                        draw_cube(vec3(x - 0.96, h * 0.62, wz), vec3(0.04, 0.8, 1.2), None, glass);
                        draw_cube(vec3(x + 0.96, h * 0.62, wz), vec3(0.04, 0.8, 1.2), None, glass);
                    }
                    let fz = o.z + len / 2.0;
                    if o.ramp {
                        // rampa inclinada (degraus finos): sobe da base (z maior) até o teto
                        let n = 20;
                        let dz = RAMP_LEN / n as f32;
                        for k in 0..n {
                            let zc2 = fz + (k as f32 + 0.5) * dz;
                            let hh = o.ramp_h(zc2).max(0.04);
                            let tone = if k % 2 == 0 { 1.0 } else { 0.88 };
                            let deck = fog(Color::new(0.85 * tone, 0.7 * tone, 0.2 * tone, 1.0), cz, o.z);
                            draw_cube(vec3(x, hh / 2.0, zc2), vec3(1.8, hh, dz), Some(&t.metal), deck);
                            let rail = fog(Color::new(0.45, 0.45, 0.5, 1.0), cz, o.z);
                            draw_cube(vec3(x - 0.9, hh / 2.0 + 0.05, zc2), vec3(0.12, hh + 0.1, dz), None, rail);
                            draw_cube(vec3(x + 0.9, hh / 2.0 + 0.05, zc2), vec3(0.12, hh + 0.1, dz), None, rail);
                        }
                    } else {
                        // frente: faróis e para-brisa
                        let lamp = fog(Color::new(1.0, 1.0, 0.75, 1.0), cz, o.z);
                        draw_sphere(vec3(x - 0.6, 0.9, fz + 0.04), 0.17, None, lamp);
                        draw_sphere(vec3(x + 0.6, 0.9, fz + 0.04), 0.17, None, lamp);
                        draw_cube(vec3(x, h * 0.63, fz - 0.02), vec3(1.4, 1.0, 0.05), None, fog(Color::new(0.2, 0.3, 0.45, 1.0), cz, o.z));
                        draw_cube(vec3(x, h * 0.47, fz + 0.02), vec3(1.5, 0.1, 0.05), None, fog(Color::new(1.0, 0.25, 0.7, 1.0), cz, o.z));
                    }
                    draw_cube_wires(vec3(x, h / 2.0, o.z), vec3(1.9, h, len), Color::new(0.0, 0.0, 0.0, 0.5));
                }
                Kind::Wall => {
                    // parede larga com duas aberturas pequenas (túneis curtos lado a lado)
                    let len = o.len();
                    let conc = fog(Color::new(0.55, 0.55, 0.6, 1.0), cz, o.z);
                    let sl = o.wall_solid();
                    // pista fechada
                    draw_cube(vec3(lane_x(sl), WALL_H / 2.0, o.z), vec3(LANE_W, WALL_H, len), Some(&t.concrete), conc);
                    // pilar entre as duas aberturas
                    draw_cube(vec3(o.wall_pillar_x(), WALL_H / 2.0, o.z), vec3(PILLAR_HALF * 2.0 + 0.1, WALL_H, len), Some(&t.concrete), conc);
                    // lateral externa
                    let side = if o.lane == 0 {
                        lane_x(0) - TUNNEL_OPEN_HALF - 0.4
                    } else {
                        lane_x(2) + TUNNEL_OPEN_HALF + 0.4
                    };
                    draw_cube(vec3(side, WALL_H / 2.0, o.z), vec3(0.8, WALL_H, len), Some(&t.concrete), conc);
                    // teto: viga acima das duas aberturas
                    let cx = o.wall_pillar_x();
                    let beam = WALL_H - TUNNEL_H;
                    draw_cube(vec3(cx, TUNNEL_H + beam / 2.0, o.z), vec3(LANE_W * 2.0, beam, len), Some(&t.concrete), conc);
                    // faixa de alerta e luzes na entrada
                    let ez = o.z + len / 2.0 + 0.03;
                    draw_cube(vec3(cx, TUNNEL_H + 0.3, ez), vec3(LANE_W * 2.0 - 0.4, 0.3, 0.04), Some(&t.hazard), fog(WHITE, cz, o.z));
                    let lamp = if blink { ORANGE } else { Color::new(0.3, 0.15, 0.05, 1.0) };
                    for l in [o.lane, o.lane + 1] {
                        draw_sphere(vec3(lane_x(l), TUNNEL_H - 0.15, ez), 0.1, None, fog(lamp, cz, o.z));
                    }
                    draw_cube_wires(vec3(cx, WALL_H / 2.0, o.z), vec3(LANE_W * 2.0, WALL_H, len), Color::new(0.0, 0.0, 0.0, 0.5));
                }
            }
        }
    }

    fn draw_coins(&self) {
        let spin = get_time() as f32 * 4.0;
        for c in &self.coins {
            let center = vec3(c.x, c.y, c.z);
            draw_coin_disc(center, spin, COIN_R, 0.04, Color::new(0.35, 0.25, 0.0, 1.0));
            draw_coin_disc(center, spin, COIN_R, 0.045, GOLD);
            draw_coin_disc(center, spin, COIN_R * 0.62, 0.05, YELLOW);
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
        let sz = (0.95 - (self.y - self.floor_y) * 0.12).max(0.45);
        draw_cube(vec3(x, self.floor_y + 0.04, z), vec3(sz, 0.03, sz), None, Color::new(0.0, 0.0, 0.0, 0.5));

        // prancha
        let mut lift = 0.0;
        if self.board_on {
            let (_, bm, bg) = board_skin(self.bskin, get_time() as f32);
            let hover = (get_time() as f32 * 5.0).sin() * 0.03;
            draw_board(x, self.y + 0.1 + hover, z, bm, bg);
            lift = 0.28 + hover;
        }

        if self.roll_t > 0.0 {
            // rolando: o boneco normal, só que enrolado, dando uma pirueta
            let p = 1.0 - (self.roll_t / ROLL_TIME).clamp(0.0, 1.0);
            draw_roller(x, self.y + lift, z, p * TAU, &look);
            return;
        }

        let lean = (lane_x(self.lane) - self.x) * 0.18;
        if self.board_on {
            // em cima da prancha: boneco com animação própria
            draw_surfer(x, self.y + lift, z, get_time() as f32, lean, !self.grounded, &look);
        } else {
            let in_air = !self.grounded;
            draw_runner(x, self.y, z, self.run_t, in_air, lean, &look, false);
        }
    }

    fn draw_cop(&self) {
        if self.cop_gap > 7.0 {
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
        draw_cube(vec3(x, 0.09, z), vec3(2.4, 0.01, 2.4), None, glow);
        draw_cube(vec3(x, 0.055, z), vec3(0.95, 0.03, 0.95), None, Color::new(0.0, 0.0, 0.0, 0.5));

        // quando está ficando para trás ele corre mais rápido (tentando alcançar)
        let cadence = 1.0 + (self.cop_fall_v * 0.15).min(0.6);
        draw_runner(x, self.cop_y, z, self.run_t * cadence + 1.0, self.cop_y > 0.01, 0.0, &look, true);

        // sirene no boné
        let siren = if on { RED } else { BLUE };
        draw_sphere(vec3(x, 2.2 + self.cop_y, z), 0.1, None, siren);
    }

    fn draw_timer(&self) {
        let r = self.timer_rect();
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.5));
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(0.2, 0.95, 1.0, 1.0));
        let col = if self.timer_stopped { Color::new(0.4, 1.0, 0.5, 1.0) } else { WHITE };
        draw_text(&self.timer_text(), r.x + 12.0, r.y + 31.0, 34.0, col);
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
        // FPS (canto superior esquerdo)
        draw_text(&format!("FPS: {}", get_fps()), 12.0, 28.0, 26.0, Color::new(0.6, 1.0, 0.6, 1.0));

        // moedas (canto superior direito, ao lado do botão de tela cheia)
        let txt = format!("{}", self.coins_got);
        let d = measure_text(&txt, None, 34, 1.0);
        let fs = fs_button_rect();
        let w = d.width + 56.0;
        let x = fs.x - 10.0 - w;
        draw_rectangle(x, fs.y, w, fs.h, Color::new(0.0, 0.0, 0.0, 0.5));
        draw_rectangle_lines(x, fs.y, w, fs.h, 2.0, GOLD);
        let cx = x + 24.0;
        let cy = fs.y + fs.h / 2.0;
        draw_circle(cx, cy, 13.0, GOLD);
        draw_circle(cx, cy, 8.0, YELLOW);
        draw_text(&txt, x + 46.0, fs.y + 31.0, 34.0, WHITE);

        self.draw_timer();
        self.draw_board_button();
    }
}

// boneco articulado: coxa + canela + tênis, tronco, ombros, braço + antebraço + mão, pescoço e cabeça
// moeda redonda: disco girando em torno do eixo vertical
fn draw_coin_disc(center: Vec3, spin: f32, r: f32, half_t: f32, color: Color) {
    const N: usize = 24;
    let right = vec3(spin.cos(), 0.0, spin.sin());
    let up = vec3(0.0, 1.0, 0.0);
    let n = vec3(-spin.sin(), 0.0, spin.cos());
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    for side in 0..2 {
        let off = n * (if side == 0 { half_t } else { -half_t });
        let c = center + off;
        vertices.push(Vertex::new(c.x, c.y, c.z, 0.0, 0.0, color));
    }
    for i in 0..N {
        let a = i as f32 / N as f32 * TAU;
        let p = right * (a.cos() * r) + up * (a.sin() * r);
        for side in 0..2 {
            let off = n * (if side == 0 { half_t } else { -half_t });
            let q = center + p + off;
            vertices.push(Vertex::new(q.x, q.y, q.z, 0.0, 0.0, color));
        }
    }
    for i in 0..N {
        let j = (i + 1) % N;
        let f0 = (2 + i * 2) as u16;
        let b0 = f0 + 1;
        let f1 = (2 + j * 2) as u16;
        let b1 = f1 + 1;
        indices.extend_from_slice(&[0, f0, f1]);
        indices.extend_from_slice(&[1, b1, b0]);
        indices.extend_from_slice(&[f0, b0, b1, f0, b1, f1]);
    }
    draw_mesh(&Mesh { vertices, indices, texture: None });
}

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

// boneco enrolado dando uma pirueta: as mesmas peças do boneco normal, só que encolhido
fn draw_roller(x: f32, y: f32, z: f32, ang: f32, l: &Look) {
    let (s, c) = ang.sin_cos();
    let cy = y + 0.58;
    // gira em volta do eixo lateral; a frente é -z
    let rot = |dy: f32, dz: f32| -> (f32, f32) { (dy * c + dz * s, dz * c - dy * s) };
    let cube = |dx: f32, dy: f32, dz: f32, sx: f32, sy: f32, sz: f32, col: Color| {
        let (ry, rz) = rot(dy, dz);
        draw_cube(vec3(x + dx, cy + ry, z + rz), vec3(sx, sy, sz), None, col);
    };
    let ball = |dx: f32, dy: f32, dz: f32, r: f32, col: Color| {
        let (ry, rz) = rot(dy, dz);
        draw_sphere(vec3(x + dx, cy + ry, z + rz), r, None, col);
    };

    // mochila e tronco
    cube(0.0, 0.1, 0.34, 0.46, 0.38, 0.2, Color::new(0.1, 0.1, 0.13, 1.0));
    ball(0.0, 0.1, 0.45, 0.06, l.accent);
    cube(0.0, 0.1, 0.06, 0.62, 0.44, 0.4, l.shirt);
    ball(0.0, 0.3, 0.02, 0.08, l.accent);

    // pernas dobradas: joelhos no peito, canelas para baixo, tênis
    for sx in [-0.17f32, 0.17f32] {
        cube(sx, 0.02, -0.06, 0.2, 0.2, 0.42, l.pants);
        cube(sx, -0.14, -0.26, 0.18, 0.36, 0.18, l.pants);
        cube(sx, -0.36, -0.2, 0.2, 0.14, 0.34, l.shoes);
    }

    // braços abraçando as pernas
    for sx in [-0.4f32, 0.4f32] {
        cube(sx, 0.08, -0.1, 0.16, 0.3, 0.2, l.shirt);
        ball(sx * 0.88, -0.1, -0.3, 0.1, l.skin);
    }

    // cabeça encolhida para a frente
    ball(0.0, 0.2, -0.3, 0.24, l.skin);
    ball(0.0, 0.27, -0.25, 0.255, l.hair);
}

// boneco em cima da prancha: joelhos dobrados, balança o corpo e os braços
fn draw_surfer(x: f32, y: f32, z: f32, t: f32, lean: f32, air: bool, l: &Look) {
    let sway = (t * 3.0).sin();
    let sway2 = (t * 3.0 + 1.6).sin();
    let cr = 0.12 + 0.05 * (t * 6.0).sin() + if air { 0.1 } else { 0.0 }; // quanto agacha
    let tx = x + lean * 0.5 + sway * 0.04;
    let hx = x + lean + sway * 0.07;

    // pernas: um pé na frente e outro atrás, joelhos dobrados
    for (sx, zf) in [(-0.14f32, -0.3f32), (0.14f32, 0.3f32)] {
        let hip = vec3(x + sx, y + 0.82 - cr, z + zf * 0.25);
        let knee = vec3(x + sx, y + 0.44 - cr * 0.3, z + zf * 0.6 - 0.12);
        let foot = vec3(x + sx, y + 0.07, z + zf);
        draw_cube(
            vec3(hip.x, (hip.y + knee.y) / 2.0, (hip.z + knee.z) / 2.0),
            vec3(0.22, 0.4, 0.26),
            None,
            l.pants,
        );
        draw_cube(
            vec3(knee.x, (knee.y + foot.y) / 2.0, (knee.z + foot.z) / 2.0),
            vec3(0.19, 0.4, 0.22),
            None,
            l.pants,
        );
        draw_cube(vec3(foot.x, foot.y, foot.z - 0.05), vec3(0.22, 0.14, 0.38), None, l.shoes);
    }

    // tronco, ombros, cinto, listra e mochila
    draw_cube(vec3(tx, y + 1.12 - cr, z - 0.03), vec3(0.62, 0.62, 0.34), None, l.shirt);
    draw_cube(vec3(tx, y + 1.42 - cr, z - 0.03), vec3(0.8, 0.18, 0.36), None, l.shirt);
    draw_cube(vec3(tx, y + 0.82 - cr, z), vec3(0.64, 0.1, 0.36), None, l.belt);
    draw_cube(vec3(tx, y + 1.3 - cr, z + 0.15), vec3(0.62, 0.06, 0.03), None, l.accent);
    draw_cube(vec3(tx, y + 1.12 - cr, z + 0.25), vec3(0.46, 0.52, 0.2), None, Color::new(0.1, 0.1, 0.13, 1.0));
    draw_cube(vec3(tx, y + 1.12 - cr, z + 0.36), vec3(0.3, 0.06, 0.02), None, l.accent);

    // braços abertos balançando para equilibrar
    for (sd, ph) in [(-1.0f32, sway), (1.0f32, sway2)] {
        let up = ph * 0.14;
        draw_cube(vec3(tx + sd * 0.62, y + 1.34 - cr + up * 0.5, z), vec3(0.3, 0.16, 0.2), None, l.shirt);
        draw_cube(vec3(tx + sd * 0.88, y + 1.26 - cr + up, z - 0.04), vec3(0.28, 0.13, 0.16), None, l.skin);
        draw_sphere(vec3(tx + sd * 1.06, y + 1.22 - cr + up * 1.2, z - 0.06), 0.09, None, l.skin);
    }

    // pescoço e cabeça
    draw_cube(vec3(hx, y + 1.58 - cr, z), vec3(0.14, 0.12, 0.14), None, l.skin);
    draw_sphere(vec3(hx, y + 1.78 - cr, z - 0.02), 0.24, None, l.skin);
    draw_sphere(vec3(hx, y + 1.83 - cr, z + 0.02), 0.255, None, l.hair);
    draw_cube(vec3(hx, y + 1.8 - cr, z + 0.02), vec3(0.5, 0.07, 0.5), None, l.accent);
}

// cenário dos dois lados da pista (muda em cada mapa)
fn draw_scenery(map: usize, sx: f32, zz: f32, seed: i32, cz: f32, t: &Textures) {
    let dir = if sx < 0.0 { -1.0f32 } else { 1.0f32 };
    let f = |c: Color| fog(c, cz, zz);
    // número "aleatório" de 0 a 1 que sempre dá o mesmo valor para o mesmo pedaço de cenário
    let r = |k: i32| -> f32 { seed.wrapping_mul(31).wrapping_add(k * 977).rem_euclid(100) as f32 / 100.0 };

    match map % MAP_COUNT {
        // cidade: prédios com janelas acesas
        0 => {
            let h = 5.0 + seed.rem_euclid(12) as f32;
            let tint = seed.rem_euclid(5) as f32 * 0.05;
            let near = cz - zz < 45.0;
            let k = if near { 1.0 } else { 0.75 };
            let wall = f(Color::new((0.38 + tint) * k, (0.34 + tint * 0.5) * k, (0.55 + tint) * k, 1.0));
            draw_cube(
                vec3(sx, h / 2.0, zz),
                vec3(5.0, h, 5.2),
                if near { Some(&t.concrete) } else { None },
                wall,
            );
            // cobertura do prédio
            draw_cube(vec3(sx, h + 0.15, zz), vec3(5.2, 0.3, 5.4), None, f(Color::new(0.3, 0.28, 0.4, 1.0)));
            if cz - zz < 80.0 {
                let fx = sx - dir * 2.56;
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
                            vec3(0.1, 1.0, 0.9),
                            None,
                            f(col),
                        );
                    }
                }
            }
        }
        // deserto: dunas, mesas de pedra, cactos e pedras
        1 => {
            let x = sx + dir * r(1) * 3.0;
            draw_cube(vec3(sx + dir, 0.3, zz), vec3(7.0, 0.6, 5.6), None, f(Color::new(0.9, 0.72, 0.45, 1.0)));
            match seed.rem_euclid(5) {
                0 => {
                    let h = 4.0 + r(2) * 5.0;
                    draw_cube(vec3(x, h / 2.0, zz), vec3(4.6, h, 4.6), None, f(Color::new(0.78, 0.42, 0.25, 1.0)));
                    draw_cube(vec3(x, h * 0.35, zz), vec3(4.9, 0.45, 4.9), None, f(Color::new(0.9, 0.62, 0.4, 1.0)));
                    draw_cube(vec3(x, h * 0.7, zz), vec3(4.9, 0.45, 4.9), None, f(Color::new(0.6, 0.3, 0.18, 1.0)));
                    draw_cube(vec3(x, h + 0.15, zz), vec3(5.0, 0.3, 5.0), None, f(Color::new(0.7, 0.4, 0.25, 1.0)));
                }
                1 | 2 => {
                    let g = f(Color::new(0.2, 0.55, 0.25, 1.0));
                    let h = 2.0 + r(3) * 1.5;
                    draw_cube(vec3(x, 0.6 + h / 2.0, zz), vec3(0.55, h, 0.55), None, g);
                    draw_cube(vec3(x - 0.55, 0.6 + h * 0.55, zz), vec3(0.55, 0.3, 0.3), None, g);
                    draw_cube(vec3(x - 0.75, 0.6 + h * 0.55 + 0.5, zz), vec3(0.3, 1.0, 0.3), None, g);
                    draw_cube(vec3(x + 0.55, 0.6 + h * 0.4, zz), vec3(0.55, 0.3, 0.3), None, g);
                    draw_cube(vec3(x + 0.75, 0.6 + h * 0.4 + 0.45, zz), vec3(0.3, 0.9, 0.3), None, g);
                }
                3 => {
                    draw_cube(vec3(x, 1.1, zz), vec3(1.6, 1.0, 1.4), None, f(Color::new(0.55, 0.4, 0.3, 1.0)));
                    draw_cube(vec3(x + 0.9, 0.9, zz + 0.5), vec3(1.0, 0.6, 0.9), None, f(Color::new(0.65, 0.5, 0.38, 1.0)));
                }
                _ => {
                    draw_sphere(vec3(x, 1.0, zz), 0.4, None, f(Color::new(0.55, 0.4, 0.2, 1.0)));
                }
            }
        }
        // floresta: árvores e moitas
        2 => {
            let x = sx + dir * (r(1) * 2.5 - 1.0);
            let h = 2.2 + r(2) * 1.6;
            let leaf = f(Color::new(0.1 + r(3) * 0.1, 0.42 + r(4) * 0.2, 0.14, 1.0));
            draw_cube(vec3(x, h / 2.0, zz), vec3(0.55, h, 0.55), None, f(Color::new(0.4, 0.26, 0.12, 1.0)));
            draw_sphere(vec3(x, h + 0.6, zz), 1.5, None, leaf);
            draw_sphere(vec3(x + 0.5, h + 1.6, zz + 0.2), 1.1, None, leaf);
            draw_sphere(vec3(x - 0.7, h + 0.2, zz - 0.4), 1.0, None, leaf);
            draw_sphere(vec3(sx - dir * 2.2, 0.45, zz + 2.2), 0.6, None, f(Color::new(0.08, 0.35, 0.12, 1.0)));
        }
        // neve: pinheiros nevados e bonecos de neve
        3 => {
            let x = sx + dir * (r(1) * 2.0 - 0.5);
            if seed.rem_euclid(7) == 0 {
                let white = f(Color::new(0.97, 0.98, 1.0, 1.0));
                draw_sphere(vec3(x, 0.6, zz), 0.65, None, white);
                draw_sphere(vec3(x, 1.5, zz), 0.45, None, white);
                draw_sphere(vec3(x, 2.15, zz), 0.32, None, white);
                draw_cube(vec3(x - dir * 0.4, 2.15, zz), vec3(0.3, 0.08, 0.08), None, f(Color::new(1.0, 0.5, 0.1, 1.0)));
                draw_cube(vec3(x, 2.55, zz), vec3(0.4, 0.3, 0.4), None, f(Color::new(0.1, 0.1, 0.12, 1.0)));
            } else {
                let s = 0.9 + r(2) * 0.5;
                draw_cube(vec3(x, 0.5, zz), vec3(0.45, 1.0, 0.45), None, f(Color::new(0.35, 0.22, 0.1, 1.0)));
                for i in 0..3 {
                    let w = (2.8 - i as f32 * 0.8) * s;
                    let y0 = 1.0 + i as f32;
                    draw_cube(vec3(x, y0 + 0.5, zz), vec3(w, 1.0, w), None, f(Color::new(0.08, 0.3, 0.22, 1.0)));
                    draw_cube(
                        vec3(x, y0 + 1.0, zz),
                        vec3(w + 0.25, 0.25, w + 0.25),
                        None,
                        f(Color::new(0.95, 0.97, 1.0, 1.0)),
                    );
                }
            }
        }
        // vulcão: montanhas escuras com lava
        _ => {
            let x = sx + dir * r(1) * 2.0;
            let h = 3.0 + r(2) * 8.0;
            let n = 4;
            let step = h / n as f32;
            for i in 0..n {
                let w = 5.2 * (1.0 - i as f32 * 0.2);
                let v = 0.14 + 0.03 * (i % 2) as f32;
                draw_cube(
                    vec3(x, step * (i as f32 + 0.5), zz),
                    vec3(w, step, w),
                    None,
                    f(Color::new(v + 0.04, v, v, 1.0)),
                );
            }
            let pulse = 0.75 + 0.25 * (get_time() as f32 * 2.0 + zz).sin();
            draw_sphere(vec3(x, h + 0.2, zz), 0.55, None, f(Color::new(pulse, 0.4 * pulse, 0.05, 1.0)));
            // rio de lava ao lado da calçada
            draw_cube(vec3(sx * 0.72, 0.04, zz), vec3(1.2, 0.1, 6.0), None, f(Color::new(pulse, 0.35 * pulse, 0.05, 1.0)));
        }
    }
}

// neve e brasas na frente de tudo
fn draw_particles(th: &Theme) {
    if th.fx < 2 {
        return;
    }
    let sw = screen_width();
    let sh = screen_height();
    let now = get_time() as f32;
    for i in 0..70u32 {
        let px = hash(i, 5, 31) * sw + (now * 0.8 + i as f32).sin() * 12.0;
        let sp = 30.0 + hash(i, 6, 32) * 70.0;
        let rad = 1.2 + hash(i, 7, 33) * 2.2;
        if th.fx == 2 {
            let py = (hash(i, 8, 34) * sh + now * sp) % sh;
            draw_circle(px, py, rad, Color::new(1.0, 1.0, 1.0, 0.85));
        } else {
            let py = sh - (hash(i, 8, 34) * sh + now * sp * 0.8) % sh;
            draw_circle(px, py, rad * 0.8, Color::new(1.0, 0.55, 0.1, 0.8));
        }
    }
}

// cartão de um mapa na tela de mapas
fn draw_map_card(r: Rect, i: usize, selected: bool) {
    let th = theme(i);
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.1, 0.1, 0.16, 0.95));
    let pr = Rect::new(r.x + 6.0, r.y + 6.0, r.w - 12.0, r.h - 40.0);

    // céu
    let bands = 12;
    for b in 0..bands {
        let t = b as f32 / bands as f32;
        let col = Color::new(
            th.sky_top.r + (th.fog.r - th.sky_top.r) * t,
            th.sky_top.g + (th.fog.g - th.sky_top.g) * t,
            th.sky_top.b + (th.fog.b - th.sky_top.b) * t,
            1.0,
        );
        draw_rectangle(pr.x, pr.y + t * pr.h * 0.5, pr.w, pr.h * 0.5 / bands as f32 + 1.0, col);
    }
    // chão e pista em perspectiva
    let hy = pr.y + pr.h * 0.5;
    draw_rectangle(pr.x, hy, pr.w, pr.h * 0.5, th.ground);
    let cx = pr.x + pr.w / 2.0;
    let by = pr.y + pr.h;
    let bw = pr.w * 0.5;
    draw_triangle(vec2(cx - 3.0, hy), vec2(cx + 3.0, hy), vec2(cx + bw, by), th.road);
    draw_triangle(vec2(cx - 3.0, hy), vec2(cx + bw, by), vec2(cx - bw, by), th.road);
    draw_line(cx - 1.0, hy, cx - bw * 0.33, by, 2.0, th.line);
    draw_line(cx + 1.0, hy, cx + bw * 0.33, by, 2.0, th.line);

    // cenário em miniatura
    for side in [-1.0f32, 1.0f32] {
        for k in 0..2 {
            let px = cx + side * pr.w * (0.28 + 0.15 * k as f32);
            let hh = pr.h * (0.28 - 0.08 * k as f32);
            match i % MAP_COUNT {
                0 => {
                    draw_rectangle(px - 9.0, hy - hh, 18.0, hh, c3(0.38, 0.34, 0.55));
                    draw_rectangle(px - 4.0, hy - hh + 6.0, 4.0, 4.0, c3(1.0, 0.9, 0.6));
                }
                1 => draw_rectangle(px - 12.0, hy - hh * 0.8, 24.0, hh * 0.8, c3(0.78, 0.42, 0.25)),
                2 => draw_triangle(vec2(px, hy - hh), vec2(px - 10.0, hy), vec2(px + 10.0, hy), c3(0.12, 0.45, 0.18)),
                3 => {
                    draw_triangle(vec2(px, hy - hh), vec2(px - 10.0, hy), vec2(px + 10.0, hy), c3(0.08, 0.3, 0.22));
                    draw_triangle(vec2(px, hy - hh), vec2(px - 4.0, hy - hh * 0.65), vec2(px + 4.0, hy - hh * 0.65), WHITE);
                }
                _ => {
                    draw_triangle(vec2(px, hy - hh), vec2(px - 14.0, hy), vec2(px + 14.0, hy), c3(0.2, 0.14, 0.14));
                    draw_circle(px, hy - hh, 3.0, c3(1.0, 0.45, 0.1));
                }
            }
        }
    }

    let border = if selected { GOLD } else { Color::new(0.4, 0.4, 0.5, 1.0) };
    draw_rectangle_lines(r.x, r.y, r.w, r.h, if selected { 4.0 } else { 2.0 }, border);
    text_c(th.name, r.x + r.w / 2.0, r.y + r.h - 12.0, 22, WHITE);
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

fn draw_sky(th: &Theme, in_game: bool) {
    clear_background(th.sky_top);
    set_default_camera();
    let sw = screen_width();
    let sh = screen_height();
    // no jogo o céu só aparece acima do horizonte: o degradê termina na cor da névoa
    let end = if in_game { th.fog } else { th.sky_bot };
    let span = if in_game { 0.245 } else { 1.0 };
    let bands = 48;
    for i in 0..bands {
        let y0 = i as f32 / bands as f32;
        let t = (y0 / span).min(1.0);
        let col = Color::new(
            th.sky_top.r + (end.r - th.sky_top.r) * t,
            th.sky_top.g + (end.g - th.sky_top.g) * t,
            th.sky_top.b + (end.b - th.sky_top.b) * t,
            1.0,
        );
        draw_rectangle(0.0, y0 * sh, sw, sh / bands as f32 + 1.0, col);
    }
    let now = get_time() as f32;
    if th.fx == 1 {
        for i in 0..60u32 {
            let sx = hash(i, 1, 11) * sw;
            let sy = hash(i, 2, 12) * sh * 0.45;
            let tw = 0.5 + 0.5 * (now * 2.0 + i as f32).sin();
            draw_circle(sx, sy, 0.8 + hash(i, 3, 13) * 1.2, Color::new(1.0, 1.0, 1.0, 0.25 + 0.5 * tw));
        }
    }
    // sol / lua
    draw_circle(sw * 0.78, sh * 0.12, th.orb_r * 1.6, Color::new(th.orb.r, th.orb.g, th.orb.b, 0.12));
    draw_circle(sw * 0.78, sh * 0.12, th.orb_r, Color::new(th.orb.r, th.orb.g, th.orb.b, 0.9));
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
        vec3(x, 0.075, z),
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
    lane: Option<Sound>,
    land: Option<Sound>,
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
        // trocar de faixa: um "uush" curtinho
        let lane = synth(r, 0.12, |t, p| {
            noise(t) * (p * std::f32::consts::PI).sin() * 0.2 + (TAU * (350.0 + 350.0 * p) * t).sin() * (1.0 - p) * 0.08
        });
        // cair no chão: "tum" grave
        let land = synth(r, 0.16, |t, p| {
            (TAU * (130.0 - 70.0 * p) * t).sin() * (1.0 - p).powf(2.0) * 0.6 + noise(t) * (1.0 - p).powf(4.0) * 0.12
        });

        Sounds {
            coin: load(coin, r).await,
            jump: load(jump, r).await,
            roll: load(roll, r).await,
            crash: load(crash, r).await,
            cop: load(cop, r).await,
            revive: load(revive, r).await,
            board: load(board, r).await,
            board_break: load(board_break, r).await,
            lane: load(lane, r).await,
            land: load(land, r).await,
            click: load(click, r).await,
            music: load(music(r), r).await,
        }
    }

    fn play_ev(&self, e: Ev) {
        match e {
            Ev::Jump => play(&self.jump, 0.8),
            Ev::Roll => play(&self.roll, 0.8),
            Ev::Coin => play(&self.coin, 0.9),
            Ev::Crash => play(&self.crash, 1.0),
            Ev::Cop => play(&self.cop, 0.9),
            Ev::Revive => play(&self.revive, 1.0),
            Ev::Board => play(&self.board, 0.9),
            Ev::BoardBreak => play(&self.board_break, 1.0),
            Ev::Lane => play(&self.lane, 0.7),
            Ev::Land => play(&self.land, 0.8),
        }
    }

    fn click(&self) {
        play(&self.click, 0.8);
    }

    fn start_music(&self) {
        if let Some(m) = &self.music {
            play_sound(m, PlaySoundParams { looped: true, volume: 0.5 });
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
    tries: u8,
}

impl Audio {
    fn new() -> Self {
        Audio { s: None, tries: 0 }
    }
    async fn unlock(&mut self) {
        // carrega os sons no primeiro toque/tecla; se algum falhar, tenta de novo (até 3 vezes)
        let precisa = match &self.s {
            None => true,
            Some(s) => !(s.music.is_some() && s.click.is_some()) && self.tries < 3,
        };
        if precisa {
            self.tries += 1;
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
            draw_surfer(0.0, 0.38 + bob, 0.0, t, 0.0, false, &pl);
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
    Maps,
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
    let mut map = 0usize;
    let mut keys: u64 = START_KEYS;
    let mut best = 0u32;
    let mut timer_pos = 1usize;
    let mut game = Game::new(0, timer_pos, pskin, cskin, bskin, map);
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
                draw_sky(&theme(map), false);
                draw_preview(0, pskin, cskin, bskin);
                text_c("NEON DASH", sw / 2.0, sh * 0.09, 64, c3(0.2, 0.95, 1.0));
                text_c(&format!("Mapa: {}", theme(map).name), sw / 2.0, sh * 0.09 + 34.0, 26, WHITE);

                // chaves no canto
                draw_key(34.0, 34.0, 30.0, Color::new(0.5, 0.8, 1.0, 1.0), Color::new(0.05, 0.05, 0.12, 1.0));
                draw_text(&fmt_keys(keys), 62.0, 42.0, 24.0, WHITE);

                let bw = 260.0f32.min(sw - 40.0);
                let y0 = sh * 0.6;
                let play_r = Rect::new((sw - bw) / 2.0, y0, bw, 64.0);
                let maps_r = Rect::new((sw - bw) / 2.0, y0 + 76.0, bw, 64.0);
                let ward_r = Rect::new((sw - bw) / 2.0, y0 + 152.0, bw, 64.0);
                let go = ui_button(play_r, "JOGAR", c3(0.1, 0.6, 0.3), ui_tap);
                let maps_b = ui_button(maps_r, "MAPAS", c3(0.9, 0.5, 0.1), ui_tap);
                let ward = ui_button(ward_r, "VESTIÁRIO", c3(0.45, 0.2, 0.85), ui_tap);
                if sounds.s.is_some() && !sounds.ok() {
                    text_c("Som indisponível neste navegador", sw / 2.0, sh - 20.0, 20, ORANGE);
                }
                if go || is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    sounds.click();
                    game = Game::new(best, timer_pos, pskin, cskin, bskin, map);
                    screen = Screen::Playing;
                    if !music_on {
                        sounds.start_music();
                        music_on = true;
                    }
                } else if maps_b {
                    sounds.click();
                    screen = Screen::Maps;
                } else if ward {
                    sounds.click();
                    screen = Screen::Wardrobe;
                }
            }

            Screen::Maps => {
                draw_sky(&theme(map), false);
                text_c("MAPAS", sw / 2.0, 46.0, 44, WHITE);

                let top = 70.0;
                let bottom = sh - 80.0;
                let gap = 12.0;
                let rows = ((MAP_COUNT + 1) / 2) as f32;
                let cw = (sw - 40.0 - gap) / 2.0;
                let ch = (((bottom - top) - gap * (rows - 1.0)) / rows).min(cw * 1.1);
                for i in 0..MAP_COUNT {
                    let col = (i % 2) as f32;
                    let row = (i / 2) as f32;
                    let r = Rect::new(20.0 + col * (cw + gap), top + row * (ch + gap), cw, ch);
                    draw_map_card(r, i, map == i);
                    if ui_tap.map_or(false, |p| r.contains(p)) {
                        map = i;
                        sounds.click();
                    }
                }

                let back = Rect::new(20.0, sh - 64.0, sw - 40.0, 48.0);
                if ui_button(back, "VOLTAR", c3(0.1, 0.6, 0.3), ui_tap) || is_key_pressed(KeyCode::Escape) {
                    sounds.click();
                    screen = Screen::Menu;
                }
            }

            Screen::Wardrobe => {
                draw_sky(&theme(map), false);
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
                            game = Game::new(best, timer_pos, pskin, cskin, bskin, map);
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


#[cfg(test)]
mod tests {
    use super::*;

    fn dist(a: &Coin, b: &Coin) -> f32 {
        ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
    }

    #[test]
    fn coin_gap_is_four() {
        assert_eq!(COIN_GAP, 4.0);
        let mut g = Game::new(0, 0, 0, 0, 0, 0);
        g.coins.clear();
        g.coin_line(1, -50.0, 6);
        assert_eq!(g.coins.len(), 6);
        for w in g.coins.windows(2) {
            assert!((dist(&w[0], &w[1]) - 4.0).abs() < 1e-3);
        }
    }

    #[test]
    fn tunnel_train_leaves_reaction_time() {
        let mut g = Game::new(0, 0, 0, 0, 0, 0);
        g.obstacles.clear();
        g.pending.clear();
        g.pat_tunnel(-100.0);
        let p = &g.pending[0];
        let ez = p.exit_z.unwrap();
        g.z = p.trigger_z;
        g.speed = START_SPEED;
        g.spawn_due();
        let t = g.obstacles.iter().find(|o| o.kind == Kind::Train).unwrap();
        let front = t.z + TRAIN_LEN / 2.0;
        assert!(front < ez, "o trem tem que nascer depois da saída, fora da parede");
        let t_meet = (g.z - front) / (g.speed + t.vz);
        let t_exit = (g.z - ez) / g.speed;
        assert!((t_meet - t_exit - TUNNEL_REACT).abs() < 0.05);
    }

    #[test]
    fn ramp_is_continuous() {
        let mut o = Obstacle::new(1, -20.0, Kind::Train);
        o.ramp = true;
        let front = o.z + o.len() / 2.0;
        assert!(o.ramp_h(front).abs() < 1e-4);
        assert!((o.ramp_h(front + RAMP_LEN) - o.h).abs() < 1e-4);
        assert_eq!(o.surface(lane_x(1), front - 0.1), Some(o.h));
    }
}
