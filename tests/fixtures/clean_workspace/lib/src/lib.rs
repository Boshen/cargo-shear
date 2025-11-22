use rustc_hash::FxHashSet;
use serde_json::Value;
use smallvec_v1::SmallVec;

pub fn example() -> anyhow::Result<()> {
    Ok(())
}

pub fn example_hyphen() -> FxHashSet<String> {
    FxHashSet::default()
}

pub fn example_underscore() -> Value {
    Value::Null
}

pub fn example_renamed() -> SmallVec<[i32; 4]> {
    SmallVec::new()
}
