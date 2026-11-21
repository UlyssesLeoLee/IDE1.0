mod math;
mod service;

fn main() {
    println!("Mock Project Running");
    let sum = math::add(20, 22);
    let prod = math::multiply(6, 7);
    println!("Sum: {}, Prod: {}", sum, prod);
    println!("Status: {}", service::status());
}
