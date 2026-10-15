#[tokio::main]
async fn main(){match massive_feed::investment::daily_purchases_universe().await{Ok(v)=>println!("Active US symbols: {} · first {:?}",v.len(),&v[..v.len().min(5)]),Err(e)=>{eprintln!("{e}");std::process::exit(1);}}}
