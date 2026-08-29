//! Thin binary wrapper: all modules and the application bootstrap live in
//! the library crate (`lib.rs`, `infiltrator_iced::run`).

fn main() -> iced::Result {
    infiltrator_iced::run()
}
