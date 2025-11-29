#[test]
fn test_misplaced() {
    let _ = fastrand::u8(..);
}

#[test]
fn test_misplaced_renamed() {
    let _ = ryu_v1::Buffer::new();
}

#[cfg(feature = "misplaced-optional")]
#[test]
fn test_misplaced_optional() {
    let _ = smallvec::SmallVec::<[u8; 0]>::new();
}

#[cfg(feature = "misplaced-optional-feature")]
#[test]
fn test_misplaced_optional_feature() {
    let _ = hashbrown::HashMap::<u8, u8>::new();
}

#[cfg(feature = "misplaced-optional-weak")]
#[test]
fn test_misplaced_optional_weak() {
    let _ = ahash::AHasher::default();
}

#[test]
fn test_misplaced_suppressed() {
    let _ = slab::Slab::<u8>::new();
}
