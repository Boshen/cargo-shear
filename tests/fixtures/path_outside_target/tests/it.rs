#[path = "../shared/helper.rs"]
mod helper;

#[test]
fn it() {
    helper::check().unwrap();
}
