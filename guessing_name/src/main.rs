use std::io;
use rand::Rng;

enum GuessResult {
    TooLow,
    TooHigh,
    Correct,
}
fn main() {
    println!("Welcome to Number Guessing Game!");
    play_game();
}

fn get_user_input() -> String {
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");
    input.trim().to_string()
}

impl GuessResult {
    fn message(&self) -> &'static str {
        match self {
            GuessResult::TooLow => "Too low!",
            GuessResult::TooHigh => "Too high!",
            GuessResult::Correct => "Correct!",
        }
    }
}

fn parse_guess(input: String) -> Option<u32> {
    input.parse().ok()
}

fn compare_number(guess: u32, secret_number: u32) -> GuessResult {
    if guess < secret_number {
        GuessResult::TooLow
    } else if guess > secret_number {
        GuessResult::TooHigh
    } else {
        GuessResult::Correct
    }
}

fn random_secret_number(from: u32, to: u32) -> u32 {
    rand::thread_rng().gen_range(from..=to) as u32
}

fn play_game() -> u32 {
    // init game state
    // randome secret number
    let from = 1;
    let to = 100;
    let secret_number = random_secret_number(from, to);
    let mut attemps = 0;
    println!("Guess the number ({}-{}):", from, to);

    loop {
        attemps += 1;
        let input = get_user_input();

        match parse_guess(input) {
            Some(guess) => {
                let result = compare_number(guess, secret_number);
                println!("{}", result.message());
                if matches!(result, GuessResult::Correct) {
                    break;
                }
            }
            None => {
                println!("That's not a valid number!");
            }
        }
    }
    println!("Congratulations! You've guessed the number!");
    attemps
}
