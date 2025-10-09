// Declare the top-level modules in the hexagonal architecture.
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;

#[tokio::main]
async fn main() {
    // This is the entry point of our application.
    // For now, it just prints a message.
    // In the future, this will initialize dependencies and start the application logic.
    println!("Hello, GitHub Trending Summary!");
}
