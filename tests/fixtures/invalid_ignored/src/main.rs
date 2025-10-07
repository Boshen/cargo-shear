use serde::Serialize;

#[derive(Serialize)]
struct Test {
    value: i32,
}

fn main() {
    let t = Test { value: 42 };
    println!("{:?}", t);
}
