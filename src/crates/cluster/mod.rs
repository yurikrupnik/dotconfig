mod actions;
mod model;

pub struct Data<'a> {
    name: &'a str,
    das: String,
}