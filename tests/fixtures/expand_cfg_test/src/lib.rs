pub fn add(a: u32, b: u32) -> u32 {
    a + b
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    #[test]
    fn decimal_works() {
        let d = dec!(1.23);
        assert_eq!(d.to_string(), "1.23");
    }
}
