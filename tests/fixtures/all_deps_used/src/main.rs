use serde::{Serialize, Deserialize};
use regex::Regex;

#[derive(Serialize, Deserialize)]
struct Data {
    email: String,
}

fn main() {
    let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    
    let data = Data {
        email: "test@example.com".to_string(),
    };
    
    if email_regex.is_match(&data.email) {
        println!("Valid email: {}", data.email);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_file_operations() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "test content").unwrap();
        // tempfile is used in dev-dependencies
    }
}