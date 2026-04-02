//! Example demonstrating system capability checking functionality

use swaybeam_doctor::check_all;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Miracast Doctor - System Check");
    println!("==============================");

    let report = check_all()?;
    report.print();

    if report.all_ok() {
        println!("\nSystem ready for Miracast streaming!");
        Ok(())
    } else {
        println!("\nSystem not fully ready for Miracast");
        std::process::exit(1);
    }
}
