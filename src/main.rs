use std::env;

mod mapcopy;

fn main() {
    //was in vec! 
    //WriteLogger::new(LevelFilter::Info, Config::default(), File::create("simplelog.log").unwrap()),

    let args: Vec<String> = env::args().collect();


    let result = mapcopy::run( &args );

    println!("run: {}", result); 
    println!("FINISHED.");
}
