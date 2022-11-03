use std::env;
use log::error;
//use log::debug;

mod mapcopy;

fn main() {
    //was in vec! 
    //WriteLogger::new(LevelFilter::Info, Config::default(), File::create("simplelog.log").unwrap()),

    let args: Vec<String> = env::args().collect();

    let result = mapcopy::run( &args );
    if result.is_ok() {
        println!("FINISHED OKAY"); 
    }else {
        let err = result.unwrap_err(); 
        error!("{}", err); 
        println!("ERROR response!"); 
    }

}
