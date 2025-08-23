use anyhow::Result;
use core::Entity;

#[tokio::main]
async fn main() -> Result<()> {
    let entity = Entity::new("test".to_string());
    println!("Created entity: {}", entity.name);
    
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    Ok(())
}