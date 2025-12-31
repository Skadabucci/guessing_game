use std::cmp::Ordering;
use std::sync::OnceLock;

use image::{DynamicImage, ImageBuffer, Rgb};
use piston_window::{
    EventLoop, PistonWindow, Texture, TextureSettings, UpdateEvent, WindowSettings, clear,
};
use plotters::prelude::*;
use rand::Rng;
use rand::prelude::*;
use rand_chacha::ChaCha20Rng;
use tabled::{
    Table, Tabled,
    settings::{Alignment, Style, object::Columns},
};

const MIN_GUESS_RANGE: u32 = 1;
const MAX_GUESS_RANGE: u32 = 100_000_000;
const GAMES_TO_PLAY: u32 = 500_000;
const WINDOW_WIDTH: u32 = (3840 as f64 / 1.75) as u32;
const WINDOW_HEIGHT: u32 = (2160 as f64 / 1.75) as u32;

#[derive(Tabled)]
struct GuessStatistics {
    pub max_guess_range: u32,
    pub total_attempts: u32,
    pub average_attempts: f64,
    pub median_attempts: u32,
    #[tabled(display_with = "display_duration")]
    pub elapsed_time: std::time::Duration,
}

static THEORETICAL_CURVE: OnceLock<Vec<(u32, f64)>> = OnceLock::new();

fn display_duration(duration: &std::time::Duration) -> String {
    format!("{:.2?}", duration)
}

fn get_random_number_in_range<R: Rng>(min: u32, max: u32, rand: &mut R) -> u32 {
    let random_number: u32 = rand.random_range(min..=max);
    random_number
}

fn play_automatically<R: Rng>(max_range: u32, rand: &mut R) -> u32 {
    let secret_number = get_random_number_in_range(MIN_GUESS_RANGE, max_range, rand);
    let mut attempts: u32 = 0;
    let mut min: u32 = MIN_GUESS_RANGE;
    let mut max: u32 = max_range;
    loop {
        attempts += 1;
        let guess: u32 = get_random_number_in_range(min, max, rand);
        match guess.cmp(&secret_number) {
            Ordering::Less => {
                min = guess + 1;
            }
            Ordering::Greater => {
                max = guess - 1;
            }
            Ordering::Equal => {
                break;
            }
        }
    }
    attempts
}

fn print_final_table(runs: &Vec<GuessStatistics>) {
    let mut table = Table::new(runs);
    table.with(Style::modern());
    table.modify(Columns::first(), Alignment::right());
    println!("{table}");
}

fn calculate_max_y(runs: &[GuessStatistics], window_size: usize) -> f64 {
    let max_val = runs
        .iter()
        .rev()
        .take(window_size)
        .map(|r| r.average_attempts as u32)
        .max(); // Find the highest value among the recent runs

    max_val.unwrap_or(0) as f64 + 1.0
}

fn draw_chart(
    root: &DrawingArea<BitMapBackend<'_>, plotters::coord::Shift>,
    runs: &[GuessStatistics],
) {
    let max_y = calculate_max_y(&runs, 2);

    let x_range = 0u32..(runs.len() as u32 + 5);
    let x_range_secondary = 0u32..(runs.len() as u32 + 5);
    let y_range = 0f64..(max_y + 5.0);
    let y_range_secondary = 0.0f64
        ..(runs
            .iter()
            .map(|r| r.elapsed_time.as_secs_f64())
            .fold(0. / 0., f64::max)
            + 1.0);
    let mut chart = ChartBuilder::on(&root)
        .margin(7)
        .set_left_and_bottom_label_area_size(20)
        .right_y_label_area_size(20)
        .build_cartesian_2d(x_range, y_range)
        .unwrap()
        .set_secondary_coord(x_range_secondary, y_range_secondary);

    chart
        .configure_mesh()
        .y_desc("Avg & Med Attempts")
        .draw()
        .unwrap();

    chart
        .configure_secondary_axes()
        .y_desc("Elapsed Time (s)")
        .draw()
        .unwrap();

    chart
        .draw_series(LineSeries::new(
            runs.iter()
                .enumerate()
                // FIX 2: Ensure data is cast to u32 to match the axes
                .map(|(i, r)| (i as u32, r.average_attempts)),
            &RED,
        ))
        .unwrap()
        .label("Average Attempts")
        .legend(|(x, y)| Rectangle::new([(x - 15, y + 1), (x, y)], RED));

    chart
        .draw_series(LineSeries::new(
            runs.iter()
                .enumerate()
                // FIX 2: Ensure data is cast to u32 to match the axes
                .map(|(i, r)| (i as u32, r.median_attempts as f64)),
            &BLUE,
        ))
        .unwrap()
        .label("Median Attempts")
        .legend(|(x, y)| Rectangle::new([(x - 15, y + 1), (x, y)], BLUE));

    let take_amount = match runs.len() as u32 {
        0 => 0,
        n => n - 1,
    }; // Take this amount so it does not exceed the x axis range

    chart
        .draw_series(LineSeries::new(
            THEORETICAL_CURVE
                .get()
                .unwrap()
                .iter()
                .take(take_amount as usize)
                .copied(),
            &GREEN,
        ))
        .unwrap()
        .label("2 * ln(x)")
        .legend(|(x, y)| Rectangle::new([(x - 15, y + 1), (x, y)], GREEN));

    chart
        .draw_secondary_series(LineSeries::new(
            runs.iter()
                .enumerate()
                .map(|(i, r)| (i as u32, r.elapsed_time.as_secs_f64())),
            &BLACK,
        ))
        .unwrap()
        .label("Elapsed Time (s)")
        .legend(|(x, y)| Rectangle::new([(x - 15, y + 1), (x, y)], BLACK));

    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .margin(20)
        .legend_area_size(5)
        .border_style(BLUE)
        .background_style(BLUE.mix(0.1))
        .label_font(("Calibri", 20))
        .draw()
        .unwrap();
}

fn draw_2d_with_chart(
    window: &mut PistonWindow,
    event: &piston_window::Event,
    runs: &[GuessStatistics],
) {
    let mut texture_context = window.create_texture_context();

    window.draw_2d(event, |c, g, _device| {
        clear([1.0; 4], g);
        let mut pixel_buffer = vec![0u8; (WINDOW_WIDTH * WINDOW_HEIGHT * 3) as usize];

        {
            let root = BitMapBackend::with_buffer(&mut pixel_buffer, (WINDOW_WIDTH, WINDOW_HEIGHT))
                .into_drawing_area();
            root.fill(&WHITE).unwrap();
            draw_chart(&root, runs);
        }

        // 2. Convert RGB Buffer to RGBA Texture
        // Piston requires RGBA (Alpha channel), but Plotters gave us RGB.
        let img_buffer =
            ImageBuffer::<Rgb<u8>, _>::from_raw(WINDOW_WIDTH, WINDOW_HEIGHT, pixel_buffer).unwrap();

        // Use DynamicImage to convert RGB -> RGBA8
        let rgba_image = DynamicImage::ImageRgb8(img_buffer).to_rgba8();

        // 3. Upload to GPU
        let texture = Texture::from_image(
            &mut texture_context,
            &rgba_image, // Pass the RGBA image, not the buffer
            &TextureSettings::new(),
        )
        .unwrap();

        // 4. Draw
        piston_window::image(&texture, c.transform, g);
    });
}

fn handle_game_loop(
    current_max_range: u32,
    runs: &mut Vec<GuessStatistics>,
    rng: &mut ChaCha20Rng,
) {
    let mut attempts_list: Vec<u32> = vec![];
    let run_start_time = std::time::Instant::now();

    for _ in 0..GAMES_TO_PLAY {
        let current_attempts = play_automatically(current_max_range, rng);
        attempts_list.push(current_attempts);
    }

    attempts_list.sort();
    let total_attempts: u32 = attempts_list.iter().sum::<u32>();

    runs.push(GuessStatistics {
        max_guess_range: current_max_range,
        total_attempts,
        average_attempts: total_attempts as f64 / GAMES_TO_PLAY as f64,
        median_attempts: attempts_list[attempts_list.len() / 2],
        elapsed_time: run_start_time.elapsed(),
    });
}

fn main() {
    // --- 1. GAME CONFIGURATION ---
    let mut runs: Vec<GuessStatistics> = vec![];
    let mut current_max_range = 1;
    let mut rng = ChaCha20Rng::from_os_rng();
    let mut finished = false;

    // --- 2. WINDOW SETUP ---
    let mut window: PistonWindow = WindowSettings::new("Live Stats", [WINDOW_WIDTH, WINDOW_HEIGHT])
        .samples(4)
        .exit_on_esc(true)
        .build()
        .unwrap();

    window.set_max_fps(60);

    // Precompute theoretical curve for reference
    THEORETICAL_CURVE.get_or_init(|| {
        (1..MAX_GUESS_RANGE)
            .map(|x| (x, 2.0 * (x as f64).ln()))
            .collect()
    });

    while let Some(event) = window.next() {
        if let Some(_args) = event.update_args() {
            if current_max_range < MAX_GUESS_RANGE {
                handle_game_loop(current_max_range, &mut runs, &mut rng);
                current_max_range += 1;
            } else if !finished {
                print_final_table(&runs);
                finished = true;
            }
        }
        draw_2d_with_chart(&mut window, &event, &runs);
    }
}
