//! Read-only market-data verification. Prints no credentials.
#[tokio::main]
async fn main(){
    match massive_feed::investment::daily_purchases("AAPL",10_000_000.).await{
        Ok(r)=>println!("{}",serde_json::to_string(&r).unwrap()),
        Err(e)=>{eprintln!("{e}");std::process::exit(1);}
    }
}
