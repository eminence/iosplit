use std::time::Instant;

use rand::prelude::*;

fn main() {
    let start = Instant::now();
    let mut rng = rand::rng();

    let mut stdout_counter = 0;
    let mut stderr_counter = 0;

    let msgs = ["hello", "hello world", "lorem ipsum dolor sit amet"];

    while start.elapsed().as_secs() < 100 {
        let sleep_time = rng.random_range(100..700);
        std::thread::sleep(std::time::Duration::from_millis(sleep_time));

        // maybe send message to stdout?
        if rng.random_bool(0.5) {
            println!("stdout message {} {}", stdout_counter, msgs.choose(&mut rng).unwrap());
            stdout_counter += 1;
        }
        // maybe send message to stderr?
        if rng.random_bool(0.5) {
            eprintln!("stderr message {} {}", stderr_counter, msgs.choose(&mut rng).unwrap());
            stderr_counter += 1;
        }

    }
    println!("Done and exiting");
}
