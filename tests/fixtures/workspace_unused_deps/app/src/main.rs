use clap::{Arg, Command};

fn main() {
    let matches = Command::new("test-app")
        .arg(Arg::new("input")
            .short('i')
            .long("input")
            .help("Input file"))
        .get_matches();

    let data = lib::get_data();
    println!("App running with data: {}", data);
}