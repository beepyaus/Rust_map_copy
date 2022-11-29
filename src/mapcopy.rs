
extern crate simplelog;
extern crate xml; 
extern crate minidom;
extern crate regex;

//But I AM using these...but not directly. 
//use std::fs::DirEntry; 
//use std::fs::File;
use std::time::SystemTime;
use std::time::Duration;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::collections::HashMap;
use std::io; 
use log::debug;
use log::error;
use simplelog::*;
use minidom::Element;
use regex::*;

//#####################################################
//# part 1. 
//#   - should of copied ('cp -some-arguments') to the base and specific base_variant dir into the swap dir
//#   - re-created the failsafe default http website pages 
//#   - NO chown should be done yet as Vagrant ( or Docker ) does not have permission to chown the 'troy' user owned files 
//#
//# part 2 - this file 
//#   - chown the swap/temp dir to correct ACLs etc 
//#   - use the XML spec file for lookup 
//#
//# NOTE: to be run on PROD/AWS EC2 or VirtualBox or Vagrant only
//
//#perform rsync or similar on all the required dirs/files into the target (normally LIVE/PROD) server!!!
//#####################################################

//#Hardcoded value to prefix the target destination for testing
const TEST_PREFIX: &'static str = "/home/troy/Downloads/mapcopy_test";
//const TEST_PREFIX: &'static str = "";

#[derive(PartialEq)]
enum Platform {
    Alpine,
    OpenBSD,
    Other
}

struct RunParams<'a> { 
dry_run: bool, 
force_yes: bool,
platform: Platform, 
mode: &'a str, 
swap_dir: &'a str, 
target: &'a str, 
build_name: &'a str, 
backup_dir: &'a str, 
build_dir: &'a str, 
logfile_dir: &'a str, 
source_dir: &'a str, 
config_dir: &'a str, 
file_map: HashMap<String, FileData >, 
file_source_map: HashMap<String, FileData >
}

#[derive(Debug, Clone)]
pub struct FileData {
    file_level: i32, 
    file_type: char, 
    file_user: String, 
    file_group: String, 
    file_mode:  i32, 
    default_file_user: String, 
    default_file_group: String, 
    default_file_mode: i32 
}

fn show_help(){
    println!("You are running the MapCopy / project_tree Rust script"); 
    println!(" - 'Part One' Shell script is assumed to be called before this call."); 
    println!(r" - This code cycles through the XML Tree Specification files and creates a 
        HashMap for all the dirs/files they reference -with their user/group/mode and then 
        the 'sectioned-off' dir area for the corresponding files that will get copied (rsynced) 
        over to the (live) Server and update their file mode/user/group info.
        It MAY or MAY-NOT delete extra files not tracked by the XML Spec. 
        Also the un-tracked files in the managed dirs may have their file modes changed etc."); 
    println!(); 
    println!(r"
            -h | --help : show help 
            -v | --version : show version
            -f | --force-yes : force a 'yes' when asking for user input (stdin) 
            -d | --dry-run : prefix echo to shell commands and --dry-run for rsync 
            -m | --mode : dev or live or other value
            ");
}

fn show_version() -> String {
    env!( "CARGO_PKG_VERSION" ).to_string() 
}

fn set_logger_level(level: &str) {

    let log_level = match level {
        "error" => LevelFilter::Error, 
        "warn" => LevelFilter::Warn, 
        "info" => LevelFilter::Info, 
        "debug" => LevelFilter::Debug, 
        "trace" => LevelFilter::Trace, 
        "off" => LevelFilter::Off, 
        _ => LevelFilter::Error, 
    }; 

    CombinedLogger::init(
        vec![
            TermLogger::new(log_level, Config::default(), TerminalMode::Stdout, ColorChoice::Auto)
        ]
    ).unwrap();

}


fn line(){
    println!("-----------------------------------------------------------------------------"); 
}

pub fn run<'a>(args: &'a Vec<String> ) -> Result<bool, String> {//{{{

    let mut g = RunParams{
        dry_run: false, 
        force_yes: false,
        platform: Platform::Other , 
        mode: "", 
        swap_dir: "", 
        target: "", 
        build_name: "", 
        backup_dir: "", 
        build_dir: "", 
        logfile_dir: "", 
        source_dir: "", 
        config_dir: "", 
        //recursive added map of each file and dir filesystem version
        file_map: HashMap::new(),
        file_source_map: HashMap::new()
    };

    let mut i = 0; 
    while i <= args.len() -1 {
        //not shifting or skipping over the set, 
        //but when the val is not in match shouldnt matter
        let flag = &args[i]; 
        let val = if i+1 <= args.len() -1 { &args[i+1] } else { "" }; 
        match flag.as_str() {
            "-h" | "--help" => { show_help(); return Ok(false) } , 
            "-v" | "--version" => { println!("Version: {}" , show_version()); return Ok(false) },
            "-f" | "--force-yes" =>  g.force_yes = true, 
            "-d" | "--dry-run" => g.dry_run = true, 
            "-m" | "--mode" => g.mode = val, 
            "-l" | "--loglevel" => set_logger_level(&val), 
            _ =>  if i == 0 && args.len() == 1 { show_help(); return Ok(false) } 
        }
        i = i+1; 
        //return //WHY here typeo?
    };

    if g.mode == "" {
        let err = format!("ERROR: No 'mode' set!");
        error!("{}", err); 
        show_help();
        return Err(err); 
    }

    // The first argument is the path that was used to call the program.
    line();
    line();
println!(r"
     ccee88oo
  C8O8O8Q8PoOb o8oo
 dOB69QO8PdUOpugoO9bD
CgggbU8OU qOp qOdoUOdcb
    6OuU  /p u gcoUodpP
      \\\//  /douUP
        \\\////  
         |||/\
         |||\/
         ||)
   .....//||||\.... project_tree v{} (written in Rust). 
", show_version()
);
    println!(); 
    debug!("run: working path: '{}' ", &args[0]);

    // The rest of the arguments are the passed command line parameters.
    // Call the program like this:
    //   $ ./args arg1 arg2
    debug!("run: arguments: '{:?}'", &args[1..]);


    let result = get_base(&args[0]); 
    if result.is_err(){
        let err = format!("run: get_base call failure: {} ", result.unwrap_err()); 
        error!("{}", err); 
        return Err(err);
    }

    let fields = result.unwrap(); 
    g.swap_dir = match &fields.get("swapdir") { Some(x) => x , _ => ""} ;
    g.target = match &fields.get("target") { Some(x) => x , _ => ""} ;
    g.build_name = match &fields.get("buildname") { Some(x) => x , _ => ""} ;
    let tmp_backup_dir = format!("{}/base_backup_BUILD_{}", &g.swap_dir, &g.target);
    g.backup_dir = tmp_backup_dir.as_str();
    g.build_dir = match &fields.get("build_dir") { Some(x) => x , _ => ""} ;
    let tmp_logfile_dir = format!("{}/rsync_log", &g.swap_dir);
    g.logfile_dir = tmp_logfile_dir.as_str();
    g.source_dir = &g.build_dir;
    g.config_dir = match &fields.get("configdir") { Some(x) => x , _ => ""} ;


    let (platform, ok) = get_platform() ; 
    if !ok {
        let err = format!("get_platform: ERROR: {} ", platform); 
        error!("{}", err);
        return Err(err);
    }
    let platform = platform.to_string(); 
    let web_owner = if platform.contains("OpenBSD") {
        g.platform = Platform::OpenBSD; 
        "www"
    }else if platform.contains("alpine") {
        g.platform = Platform::Alpine; 
        "apache" 
    }else {
        g.platform = Platform::Other; 
        "http"
    };

    line(); 
    debug!("Forcing a 'yes' entry for any user-input?: {}", g.force_yes);
    if g.dry_run {
        println!("Running in DRY-RUN mode for rsync, no changes saved!!!");
    }
    debug!("mode: {}", g.mode); 
    debug!("dry_run: {}", g.dry_run); 
    debug!("swap_dir: {}", g.swap_dir); 
    debug!("target: {}", g.target); 
    debug!("build_name: {}", g.build_name); 
    debug!("backup_dir: {}", g.backup_dir); 
    debug!("build_dir: {}", g.build_dir); 
    debug!("logfile_dir: {}", g.logfile_dir); 
    debug!("source_dir: {}", g.source_dir); 
    debug!("config_dir: {}", g.config_dir); 
    debug!("web/http area owner: {}", web_owner) ;
    line();

    if g.target == "" 
        || g.swap_dir == "" 
        || g.target == "" 
        || g.build_dir == ""
        || g.build_name == ""
        || g.backup_dir == ""
        || g.logfile_dir == ""
        || g.source_dir == ""
        || g.config_dir == ""
        || web_owner == "" {
        let err = format!( "ERROR: Some get_base fields are empty. ");
        error!("{}", err);
        return Err(err);
    }

    clean_backup_dir(&g); 
    setup_logfile_dir(&g); 

    //3rd param: boolean: delete extra files in target, 
    //TODO: all these mapcopy params can be sourced in the text config file
    //TODO: as website owner is kinda special, treat it as a TAG in the TBA settings file. 
    let _res = simple_copy(&mut g, "/var/www/html/sites/default" , false, web_owner, web_owner );  
    let _res = simple_copy(&mut g, "/var/www/html/sites/default_http" , false, web_owner, web_owner );  
    let _res = simple_copy(&mut g, "/var/www/html/sites/default_https" , false, web_owner, web_owner );  
    
    let _res = map_copy(&mut g, "/etc/httpd/conf" , false );  
    let _res = map_copy(&mut g, "/etc/apache2" , false );  
    let _res = map_copy(&mut g, "/etc/postfix" , false );  
    let _res = map_copy(&mut g, "/etc/postgresql" , false );  
    let _res = map_copy(&mut g, "/etc/php" , false );  
    let _res = map_copy(&mut g, "/etc/php8" , false );  
    let _res = map_copy(&mut g, "/var/lib/postgres" , false );  
    let _res = map_copy(&mut g, "/var/lib/postgresql" , false );  
    let _res = map_copy(&mut g, "/root", false );  
    let _res = map_copy(&mut g, "/home/vagrant", false );  
    let _res = map_copy(&mut g, "/home/arch", false );  
    let _res = map_copy(&mut g, "/home/alpine", false );  
    let _res = map_copy(&mut g, "/etc/logrotate.d" , false );  

    let _res = simple_copy(&mut g, "/etc/redis.conf" , false, "redis", "redis" );  
    let _res = simple_copy(&mut g, "/etc/ssl_self" , false , "root", "root" );  
    let _res = simple_copy(&mut g, "/etc/letsencrypt" , false , "root", "root" ); 

    return Ok(true); 

}//end fn}}}

fn run_command(command : &str ) -> (bool, String ) {//{{{
    debug!("");
    debug!("run_command: '{}'" , command); 
    let parts: Vec<&str> =  command.split(' ').collect(); 
    if parts.len() < 1 {
        return (false, "array length less than 1.".to_string()) 
    }

    let mut cmd = Command::new(&parts[0]); 
    if parts.len() > 1 {
        for p in &parts[1..] {
            cmd.arg(p); 
        }
    }

    let output = cmd.output().unwrap_or_else(
            |e| { panic!("run_command: PANIC! Failed to execute process: '{}' ", e)
        });

    let result:bool; 
    let raw_output = 
        if output.status.success() {
                result = true; 
                String::from_utf8_lossy(&output.stdout).to_string()
        } else {
                result = false;
                String::from_utf8_lossy(&output.stderr).to_string()
        };

    (result, raw_output)

}//}}}

fn clean_backup_dir( g: &RunParams ) -> bool {//{{{
//setup and clean out backup dir for next processing...
    line();
    debug!("clean_backup_dir: backup_dir: '{}'", g.backup_dir); 
    if g.backup_dir == "/" {
        error!("ERROR: backup_dir is root! exiting now.");
        line();
        return false;
    }

    let dry_run_failsafe = if g.dry_run { debug!("NOTE: running in dry_run mode, adding 'echo' before command call!"); "echo " }
            else { "" };

    let cmd = format!("{}rm -rf {}", dry_run_failsafe, g.backup_dir );
    let (result , raw_output) = run_command(&cmd);

    if result {
        debug!("clean_backup_dir: remove dir result: '{}'", raw_output);

    } else {
        debug!("clean_backup_dir: failed to remove dir result: '{}'", raw_output);
        line();
        return false;
    }
        
    let cmd = format!("{}mkdir -p {}", dry_run_failsafe,  g.backup_dir );
    let (result , raw_output) = run_command(&cmd);

    if result {
        debug!("clean_backup_dir: mkdir result: '{}'", raw_output);

    } else {
        debug!("clean_backup_dir: failed to mkdir result: '{}' ", raw_output);
        line(); 
        return false;
    }

    line();
    return true
}//}}}

fn setup_logfile_dir( g: &RunParams ) -> bool {//{{{
    line();
    debug!("setup_logfile_dir: logfile_dir: {}" , g.logfile_dir) ;

    let dry_run_failsafe  = if g.dry_run { "echo " } else { "" };

    //CAUTION: Openbsd does NOT have -v for mkdir command!
    let cmd = format!("{}mkdir -p {}", dry_run_failsafe,  g.logfile_dir );

    let (result, raw_output) = run_command(&cmd); 

    debug!("setup_logfile_dir: result: '{}' " , raw_output); 
    line();
    result
}//}}}

fn get_platform() -> (String, bool) {//{{{

    let cmd_uname = "uname"; 
    let output = Command::new(cmd_uname)
                    .arg("-a")
                    .output()
                    .unwrap_or_else(
                            |e| { panic!("failed to execute process: {}", e)
                        });
    let ok; 
    let raw_output : String = 
        if output.status.success() {
                ok = true; 
                String::from_utf8_lossy(&output.stdout).to_string()
        } else {
                ok = false;
                String::from_utf8_lossy(&output.stderr).to_string()
        };
    

    (raw_output, ok)
    
}//}}}

fn get_base<'a>(self_path : &str ) -> Result<HashMap<String, String>,String> {//{{{
// the bash script must output the var as 
// foo: value
// foo: value
// ...and this code will parse that


    let cmd = "./base_setup.sh";
    if Path::new(cmd).exists() == false {
        let err = format!("get_base: ERROR: 'base_setup.sh' files does not exist."); 
        error!("{}", err); 
        return Err(err)
    }

    let mut fields: HashMap<String,String> = HashMap::new(); 

    //-s param for the shell script to -know- what dir path it is in .
    //  ...subst the BASH_SOURCE[0] call!
    let cmd = format!("{} -s {} ", cmd, self_path) ; 
    let (ok , raw_output) = run_command(&cmd);
    if ok {
        let re = Regex::new("^(.*?): (.*)$").unwrap();
        for line in raw_output.lines(){
            let caps = re.captures(&line).unwrap();
            let key = String::from ( caps.get(1).map_or("", |m| m.as_str()) ); 
            let value = String::from( caps.get(2).map_or("", |m| m.as_str()) );
            debug!(" KEY, VALUE = '{}' , '{}' " , key, value);
            let result = fields.insert(key.clone(), value); 
            if result.is_some() {
                let err = format!("Strange! the key: {} was already in the hashmap!", &key); 
                warn!("{}", err); 
            }
        }
        for (key,val)  in fields.iter() {
            debug!("base_setup.sh field key: '{}' , val: '{}' " , key, val ) ; 
        }
        if raw_output.lines().count() != fields.len() {
           let err = format!("Something's wrong: shell script lines : '{}' , hashmap length: '{}'", 
                             raw_output.lines().count(), 
                             fields.len() ); 
           error!("{}", err); 
           return Err(err); 
        }
    }
    else{
        let err = format!("get_base: not okay: '{}' ", raw_output); 
        error!("{}", err); 
        return Err(err); 
    }

    Ok(fields)
}//}}}

fn scan_tree<'a >( //{{{
    g: &'a mut RunParams, 
    cur_path: String, 
    cur_dir : &'a Element , 
    file_level: i32 , 
    parent_default_file_user: String, 
    parent_default_file_group: String, 
    parent_default_file_mode: i32 ) -> Result<bool,String> {
// Recurse/iterate into each dir and create a hashtable of all the files/dirs to 
// compare against the filesystem candidate to be uploaded. 

    println!();
    line(); 
    let cur_dir_name = cur_dir.attr("name").unwrap_or_default(); 

    debug!("scan_tree: cur_path: '{}'" , &cur_path); 
    debug!("scan_tree: cur_dir_name (xml prop): '{}'" , cur_dir_name ); 
    debug!("scan_tree: file_level: '{}'" , file_level ); 
    debug!("scan_tree: parent_default_file_user: '{}'" , parent_default_file_user ); 
    debug!("scan_tree: parent_default_file_group: '{}'" , parent_default_file_group ); 
    debug!("scan_tree: parent_default_file_mode: '{}'" , parent_default_file_mode ); 

    // Either it's own settings or go to the parents value. ...so it trickles down.
    let default_file_user = if cur_dir.attr("default_file_user").is_none() {
                                parent_default_file_user.clone()
                            } else {
                                cur_dir.attr("default_file_user").unwrap().to_string().clone()
                            };

    let default_file_group = if cur_dir.attr("default_file_group").is_none() {
                                parent_default_file_group.clone()
                            } else {
                                cur_dir.attr("default_file_group").unwrap().to_string().clone()
                            };

    let default_file_mode :i32 = if cur_dir.attr("default_file_mode").is_none() { 
                                parent_default_file_mode.clone()
                            } else { cur_dir.attr("default_file_mode").unwrap().to_string().parse().unwrap_or_default() };

    let file_user = if cur_dir.attr("user").is_none() {
                                "".to_string()
                            } else {
                                cur_dir.attr("user").unwrap().to_string().clone()
                            };

    let file_group = if cur_dir.attr("group").is_none() {
                                "".to_string()
                            } else {
                                cur_dir.attr("group").unwrap().to_string().clone()
                            };

    let file_mode : i32 = if cur_dir.attr("mode").is_none() { 0 } 
                    else { cur_dir.attr("mode").unwrap().to_string().parse::<i32>().unwrap_or_default() };

    let file_data = FileData{
                        file_level,
                        file_type: 'd', 
                        file_user,
                        file_group,
                        file_mode ,
                        default_file_user: default_file_user.clone() , 
                        default_file_group: default_file_group.clone() ,
                        default_file_mode: default_file_mode.clone()
                    };

    g.file_map.insert(cur_path.clone(), file_data); 

    for node in cur_dir.children() {
        
        debug!("child node name (property) = {} " , node.attr("name").unwrap()); 
        let new_cur_path = format!( "{}/{}",cur_path, node.attr("name").unwrap()); 
        debug!("new_cur_path (cur_path + node) (for NEXT recursion) : '{}'", new_cur_path);

        if node.name() == "directory" {
            let result = scan_tree(
                            g, 
                            new_cur_path,
                            &node, 
                            file_level + 1 , 
                            default_file_user.clone(), 
                            default_file_group.clone(), 
                            default_file_mode.clone());

            if result.is_err() {
                let err = format!("scan_tree: level: {} (recursive result): '{}'" ,file_level,  result.unwrap_err()); 
                error!("{}", err); 
                line(); 
                return Err(err); 
            }
                
        }else if node.name() == "file" {
            
            let file_user = node.attr("user").unwrap().to_string();
            let file_group = node.attr("group").unwrap().to_string();
            let file_mode = node.attr("mode").unwrap().to_string();
            let file_mode: i32 = file_mode.parse().unwrap_or_default();
                
            let file_data = FileData{
                file_level,
                file_type: 'f', 
                file_user, 
                file_group,
                file_mode,
                default_file_user: default_file_user.clone(),
                default_file_group: default_file_group.clone(),  
                default_file_mode: default_file_mode.clone()
            };

            debug!("inserting new_cur_path: {} into file_map (hashmap) ", new_cur_path); 
            g.file_map.insert( new_cur_path , file_data ); 

        } else {
            let err = format!("scan_tre: ERROR Unexpected node name: '{}' ", node.name() );
            error!("{}", err); 
            line(); 
            return Err(err); 
        }
    }
    line(); 
    Ok(true)
}//end fn}}}

fn simple_copy( //{{{
    g: &mut RunParams, 
    path_dir: &str ,
    delete: bool , 
    file_user: &str, 
    file_group: &str 
) -> Result<bool,String> {
// simple rsync version just for default-website for e.g , no xml tree etc 
    println!();
    line(); 
    debug!("simple_copy: path_dir: '{}'", path_dir); 
    debug!("simple_copy: delete: '{}'", delete); 
    debug!("simple_copy: file_user: '{}'", file_user); 
    debug!("simple_copy: file_group: '{}'", file_group); 

    let mut source = format!("{}{}", g.source_dir , path_dir); 
    debug!( "simple_copy: source: '{}'" , source ) ; 
    let target = format!("{}{}", TEST_PREFIX , path_dir);
    
    if Path::new(String::as_str(&source)).exists() == false {
        let err = format!( "simple_copy: ERROR: '{}' does not exist!", source);
        error!("{}", err); 
        line(); 
        return Err(err); 
    }

    let dry_run_failsafe = if g.dry_run { "echo " } else { "" }; 

    if Path::new(String::as_str(&source)).is_dir() {
        // Add the slash to start copying the contents that follows the end dir and NOT the dir itself
        source = format!("{}/", &source); 
        //Caution: Openbsd does not have -v argument for mkdir
        let command = format!("{}mkdir -p {}" , dry_run_failsafe, target) ; 
        let (ok,result) = run_command(&command); 
        if ok == false {
            let err = format!("simple_copy: ERROR: mkdir failed: '{}'", result); 
            error!("{}", err); 
            line(); 
            return Err(err); 
        }
    }

    let mut logfile_part = path_dir.to_string();
    logfile_part = logfile_part.replace("/", "_");

    let chown = if !file_user.is_empty() && !file_group.is_empty() {
                    format!(" --chown={}:{}", file_user, file_group)
                } else { "".to_string() } ;

    let rsync_delete = if delete {
       " --delete".to_string()  
    } else {
        "".to_string()
    };

    let default_duration = Duration::ZERO;
    let seconds_now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or(default_duration).as_secs();
    let rsync_dryrun = if g.dry_run { "--dry-run " } else { "" }; 
    let rsync_backup = format!(" --backup --backup-dir={}{}", g.backup_dir, path_dir );
    let rsync_logfile = format!( " --log-file={}/{}_{}.log", g.logfile_dir, logfile_part, seconds_now );
    let rsync_switches = format!("{}-v -a --human-readable{}{}{}{}", 
                                 rsync_dryrun, 
                                 rsync_delete, 
                                 chown, 
                                 rsync_backup, 
                                 rsync_logfile ); 

    let rsync = format!("rsync {} {} {}", 
                        rsync_switches, 
                        source, 
                        target);

    debug!("Executing rsync call: '{}' ", rsync); 
    let (ok, raw_output) = run_command(rsync.as_str()); 
    if ok == false {
        let err = format!("simple_copy: rsync result: '{}' " , raw_output); 
        error!("{}", err); 
        line(); 
        return Err(err); 
    }
    line();
    return Ok(true); 

}//end fn}}}
 
fn scan_source( //{{{
    g: &mut RunParams,  
    path_dir: &str ) -> Result<bool, String> {
//create hashtable for the filesystem structure to then do a acl/mode comparision against .
    println!(); 
    line(); 
    debug!("scan_source: path_dir: '{}'", path_dir) ; 

    let result = scan_source_dir(g, path_dir, 0);
    if result.is_err() {
        let err = format!("scan_source: From scan_source_dir call: {}", result.unwrap_err());
        error!("{}", &err); 
        line();
        return Err(err); 
    }
    let ret = show_prelim(false, g);
    if ret.is_err(){
        let err = format!("scan_source: From show_prelim: {}" , ret.unwrap_err()); 
        debug!("{}", &err); 
        line(); 
        return Err(err); 
    }

    line();
    return Ok(true);
} //}}}

fn get_parent_perms<'a>(//{{{
        g: &'a RunParams, 
        key_path : &'a str) -> Result<(FileData, String), String > {
//this filepath does NOT exist in the XML Treepath, 
//so do up a level and get the default values. 

    debug!("get_parent_perms: keypath:'{}' ",  key_path);

    //let dirs: Vec<&str> = key_path.split('/').collect();
    let last_dir_pos = key_path.rfind('/').unwrap_or(0);
    let last_dir = if last_dir_pos > 0 {
        let tmp = &key_path[0..last_dir_pos];
        tmp
    }else {
        let err = String::from("get_parent_perms: ERROR: empty last_dir (from: key_path)"); 
        return Err(err); 
    };

    debug!("get_parent_perms: last_dir: '{}' ", last_dir); 

    let default_file_data = FileData{ 
        file_level:  -1, 
        file_type: char::REPLACEMENT_CHARACTER, 
        file_mode: 0, 
        file_user: "".to_string(), 
        file_group: "".to_string(), 
        default_file_user: "".to_string(), 
        default_file_group: "".to_string(),
        default_file_mode: 0 , 
    };

    if !g.file_map.contains_key(last_dir){
        let err = format!( "ERROR: get_parent_perms: there is no key in the XML spec tree for '{}'", last_dir).clone(); 
        error!("{}", &err); 
        error!("get_parent_perms: Adjust XML spec or similar"); 
        return Err(err);
    }
    
    let item = g.file_map.get(last_dir).unwrap_or(&default_file_data); 
    let ret_item = item.clone();
    
    Ok((ret_item, last_dir.to_string()))


}//}}}

fn map_copy( //{{{
    g: &mut RunParams, 
    path_dir: &str , 
    delete: bool ) -> Result<bool,String> {
//# open a xml tree spec to get mode/user/group etc 
//# recurse into all directory elements to get all file elements etc 
//# populate the hash tree with the full file path for easy lookup 
//#pass over to copysourcefiles with delete param for rsync to decide if to rm extra files NOT in source dir.  
    println!(); 
    line(); 
    debug!("map_copy: path_dir: '{}'", path_dir); 
    debug!("map_copy: delete: '{}'", delete); 

    if !Path::new(path_dir).exists() {
       let err = format!( "map_copy: 'path_dir' parameter not found on filesystem.\n
                          NOT performing map_copy as a precaution.");
        error!("{}",err);
        line();
        return Err(err); 
    }

    //replace / . with _ chars for filename component. 
    let mut file_part  = String::from(path_dir) ; 
    file_part = file_part.replace('/', "_");
    file_part = file_part.replace('.', "_");

    let file_name = format!("{}/tree_definitions/spec{}.xml", g.config_dir, file_part);

    println!( "map_copy: XML Spec Treefile: '{}' ",  file_name );
    
    if !Path::new(&file_name).exists(){
        let err = format!("map_copy: File spec '{}' not found.", file_name); 
        error!("{}", err); 
        line();
        return Err(err);
    }


    let res = fs::read_to_string(&file_name); 
    if !res.is_ok(){
        let err = format!("map_copy: ERROR: could not read '{}' into string" , &file_name); 
        error!("{}", err);
        line();
        return Err(err); 
    }

    let buffer = res.unwrap();
    let root: Element = buffer.parse().unwrap();

    for child in root.children(){
        //should only be ONE MAIN DIR/ROOT DIR! ...else it's an XML grammar error anyways. 
        if child.name() == "directory" {
            let name = child.attr("name").unwrap_or_default(); 
            if name.is_empty() {
                let err = format!("map_copy: directory element did NOT have name attribute!"); 
                error!("{}", err); 
                line();
                return Err(err); 
            }

            let result = scan_tree(g, name.to_string() , child, 0, "".to_string() ,"".to_string(), 0 );
            if result.is_err(){
                let err = result.unwrap_err(); 
                let err = format!("calling scan_tree: {}", err); 
                error!("{}",err); 
                line();
                return Err(err);
            }
        }
        else{
            let err = format!("map_copy: child of XML tree was NOT a directory: '{}'", child.name()); 
            error!("{}", err); 
            line();
            return Err(err);
        }

        //end is directory. 
    }//end forloop

    //now scan source file dir created hashtable. 
    //recusrse into real build directory and cross-ref the mode/user/group from the hashtable. 
    let result = scan_source(g, path_dir); 
    if result.is_err() {
        let err = format!("map_copy: scan_source error: '{}' ", result.unwrap_err()); 
        error!("{}", err); 
        line();
        return Err(err); 
    }

    let ret = copy_source_files(g, path_dir, delete);
    if ret.is_err() {
        let err = format!("map_copy: ERROR from copy_source_files = {} ", ret.unwrap_err() ); 
        error!("{}", err); 
        line();
        return Err(err); 
    }

    line();
    return Ok(true); 
} //end fn}}}

fn copy_source_files( //{{{
    g: &mut RunParams, 
    path_dir: &str, 
    delete: bool) -> Result<bool,String> {
//re-chmods the files/dirs that are in the preset TMP dir --NOT the target files 
//re-chowns the '' '' ''
//THEN rsync that dir structure across.

    println!();
    line();
    debug!("copy_source_files: path_dir: '{}'", path_dir); 
    debug!("copy_source_files: delete: '{}'", delete); 

    for (key, item) in &g.file_source_map {

        let dry_run_failsafe = if g.dry_run { "echo " } else { "" } ;

        debug!("copy_source_files: dry_run_failsafe: '{}' ", dry_run_failsafe) ; 
        debug!("copy_source_files: &key : '{}' ", &key); 
        debug!("copy_source_files: &g.source_dir : '{}' ", &g.source_dir); 
        let source_file = format!("{}{}",  &g.source_dir ,  key); 
        debug!("copy_source_files: source_file: '{}'", source_file); 

        let cmd_chown = format!("{}chown {}:{} {}" , dry_run_failsafe, item.file_user, item.file_group, source_file); 
        let (ok,raw_output) = run_command(&cmd_chown); 
        if !ok {
            let err = format!("copy_source_files: ERROR: chown failed: '{}'", raw_output); 
            error!("{}", &err); 
            line();
            return Err(err);
        }
        let cmd_chmod = format!("{}chmod {} {}" , dry_run_failsafe, item.file_mode, source_file); 
        let (ok,raw_output) = run_command(&cmd_chmod); 
        if !ok {
            let err = format!("copy_source_files: ERROR: chmod failed: '{}'", raw_output); 
            error!("{}", &err); 
            line();
            return Err(err); 
        }
    }

    let logfile_part = path_dir.replace('/',"_");
    let rsync_dryrun = if g.dry_run { "--dry-run " } else { "" }; 
    let rsync_switches = format!("{}-a --human-readable --verbose", rsync_dryrun );
    let rsync_backup = format!( " --backup --backup-dir={}{}", g.backup_dir, path_dir);
    let default_duration = Duration::ZERO;
    let seconds_now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or(default_duration).as_secs();
    let rsync_logfile = format!(" --log-file={}/{}_{}.log", g.logfile_dir, logfile_part, seconds_now );
    let rsync_delete = if delete { " --delete" } else { "" };

    //prefix normally /home/foo/Downloads/perl_test to safeguard against overcopy.
    let mut target_dir :String = String::from(TEST_PREFIX);
    target_dir.push_str(path_dir); 
    debug!("target_dir (concat from TEST_PREFIX) : {}", &target_dir); 


    //Caution: Openbsd does not have -v argument for mkdir
    let mkdir_target = format!( "mkdir -p {}", &target_dir );
    debug!("mkdir_target: {}", &mkdir_target); 
    let (ok, raw_output) = run_command(&mkdir_target);
    if !ok {
        let err = format!("copy_source_files: FAILED to run mkdir: '{}'" , raw_output);
        error!("{}",err);
        line();
        return Err(err); 
    }

    //IMPORTANT! use the trailing  '/' at end of rsync source to avoid starting at the dir, ..so to get contents of the dir.
    let mut final_source_dir : String = String::from( g.source_dir); 
    final_source_dir.push_str(path_dir);
    final_source_dir.push_str("/"); 

    //VERY IMPORTANT!!!
    //NOTE: for rsync to WORK and NOT get an main.c(1492) source missing blah blah error, 
    //there CANNOT be more than ONE space char between the arguments. 
    let rsync: String = format!("rsync {}{}{}{} {} {}", 
                            rsync_switches, 
                            rsync_delete, 
                            rsync_backup, 
                            rsync_logfile , 
                            final_source_dir, 
                            target_dir );

    debug!( "copy_source_files: &rsync command: '{}' " , &rsync );

    let (r_ok, r_raw_output) = run_command(rsync.as_str()); 
    if !r_ok {
        let err = format!("copy_source_files: FAILED: rsync: '{}'", r_raw_output); 
        error!("{}", err); 
        line();
        return Err(err); 
    }

    debug!("copy_source_files: rsync result (OK): '{}' " , r_raw_output); 
    line();
    Ok(true) 
}//end fn}}}

fn show_prelim(//{{{
    re_show: bool, 
    g: &mut RunParams, 
    ) -> Result<bool,String> {
    //show to user What will happen re file Mode, Missing etc   
    //iterate the xmltree first then the filesys source tree 
    println!(); 
    line();
    debug!("show_prelim: re_show: '{}'", re_show);
    debug!("show_prelim: force_yes: '{}'", g.force_yes);
    println!("===XML Tree spec map===");
    println!("Definition: ??? - File missing from XML spec master file.");
    println!();  

    for (key,item) in &g.file_map {
        let alert = if g.file_source_map.contains_key(key) {
            "   ".to_string()
        }else{
            "???".to_string()
        };

        let ftype = match item.file_type {
                'f' => "<FILE>", 
                'd' => "<DIR> ", 
                _ => "UNKNOWN"
        }; 

        println!( "{} {} L-{} '{}' -->  {}:{} {}",
                  alert, ftype, item.file_level,key,  item.file_user, item.file_group, item.file_mode);
    }

    println!(); 
    println!("===Filesystem source map===");
    println!("??? = File not mentioned in XML Tree spec. ");
    println!("XXX = File's mode will be overridden to match the XML file spec. ");
    println!(); 

    let mut new_file_source_map_items: HashMap<String,FileData> = HashMap::new(); 

    for (key, item) in &g.file_source_map {

        let mut tmp = item.clone(); 
        let mut msg = String::new();
        let mut alert = String::from("   "); 

        if g.file_map.contains_key(key){
            //file exists in the XML treemap...
            //the fileSourceMap CANNOT really have the target user/group as it is coming from a dev machine anyway. 

            //RESET value to match the XML spec...
            tmp.file_user = g.file_map[key].file_user.clone();
            tmp.file_group = g.file_map[key].file_group.clone();
            tmp.file_mode = g.file_map[key].file_mode.clone();

            if g.file_map[key].file_mode != item.file_mode {
                alert = "XXX".to_string();
                msg = format!("**OVERRIDE** {} --> {}", item.file_mode, g.file_map[key].file_mode );
            } 

        } else {
            //missing file
            //get last dir / go up a dir and get the default perms for that file. 
            let result = get_parent_perms(g, &key); 
            if result.is_ok() {
                let (perms, last_dir) = result.unwrap();  
                alert = "???".to_string();
                msg = format!( "**MISSING** (owner dir: '{}')", last_dir);
                tmp.file_user = perms.default_file_user;
                tmp.file_group = perms.default_file_group;

                let new_file_mode: i32 = perms.default_file_mode;
                if new_file_mode == 0 {
                    let result = format!("show_prelim: ERROR : default_file_mode was zero ! ");
                    error!("{}", result) ;
                    line();
                    return Err(result); 
                }
                tmp.file_mode = new_file_mode;
            } 
            else {
                let err = result.unwrap_err(); 
                let err = format!("show_prelim: ERROR in get_parent_perms: '{}'", err ) ;
                line();
                return Err(err); 
            };
        }

        new_file_source_map_items.insert(key.to_string(), tmp); 

        let ftype = match item.file_type {
                    'f' => "<FILE>", 
                    'd' => "<DIR> ", 
                    _ =>   "UNKNOWN"
                   }; 

    
        println!("{} {} L-{} '{}' {}:{} {} {}", alert, ftype, item.file_level, key, item.file_user, item.file_group, item.file_mode, msg );

    }//end for

    for (key, item) in new_file_source_map_items{
       g.file_source_map.insert(key, item); 
    }

    println!("XML tree spec count: {} ", g.file_map.len());
    println!("File source count {} ", g.file_source_map.len()); 

    if g.force_yes {
        println!( "FORCING a 'YES' for all would-be user input!");
    }else {
        println!( "Considering all above, proceed with the file copy tasks? y/N");

        let mut buffer = String::new();
        let stdin = io::stdin(); // We get `Stdin` here.
        let res = stdin.read_line(&mut buffer);
        if !res.is_ok() {
            line();
            return Err("bad Stdin read_line".to_string()); 
        }
        buffer = buffer.replace("\n", ""); 
        if &buffer == "y" || &buffer == "Y" {
            if !re_show {
                let ret = show_prelim(true, g);
                if ret.is_err(){
                    let str_err = ret.unwrap_err(); 
                    let str_err = format!("show_prelim (recursive return): ERROR: '{}'", &str_err ) ;
                    line();
                    return Err(str_err); 
                }
            }
            println!("Processing...");
        } else if &buffer == "N" || &buffer == "n" || &buffer == "" {
            println!("You have bailed out! Ending now."); 
            line();
            return Err("User terminated function.".to_string());
        } else {
            println!( "Couldn't understand response. Terminating now : '{}' ", buffer );
            line();
            return Err("Could not understand response".to_string()); 
        }
    }

    line();
    return Ok(true); 

} //end fn}}}

fn get_mode(platform: &Platform,  uri: &str) -> i32 {//{{{
//do a file stat to get the Mode. 
//the perl chmod NEEDS an octal value input! 
//fyi: at THIS stage, it seems the result is bitmasked and output for the decimal output etc 
//but please note the octal printout format AND the bitwise mask 
    println!(); 
    debug!("get_mode: uri: '{}'", uri); 
    if uri == "" {
        debug!("get_mode: uri was blank, exiting!"); 
        return 0; 
    }

    let cmd; 
    if *platform == Platform::Alpine {
        cmd = format!("stat -c %a {}" , uri) ;
    }else if *platform == Platform::OpenBSD{
        cmd = format!("stat -n -f %OLp {}" , uri) ;
    }else {
        //TODO: check other linux sys return 
        cmd = format!("stat -c %a {}" , uri) ;
    }

    let (ok, raw_output) = run_command(cmd.as_str());
    if ok {
        let parsed = raw_output.parse::<i32>();
        if parsed.is_ok(){
            let result = parsed.unwrap(); 
            debug!("get_mode: OK: parsed mode is: '{}' " , result) ;
            return result; 
        }else {
            error!("get_mode: ERROR: cannot parse '{}' ", raw_output); 
            return 0;
        }
    } else {
        error!("get_mode: NOT OK return: '{}': " , raw_output); 
    }

    return 0;

}//end fn}}}

fn scan_source_dir( //{{{
        g: &mut RunParams, 
        cur_dir: &str,
        level: i32 ) -> Result<bool,String> {
//recusive scan into filesystem sourcedir to create hashmap of filesdirs
//to crossref with xml trees version 
    println!(); 
    line();
    debug!("scan_source_dir: cur_dir: '{}'" , cur_dir); 
    debug!("scan_source_dir: level: '{}'" , level); 
    debug!("scan_source_dir: g.source_dir: {}", g.source_dir); 

    //CAUTION: Path.join() REPLACES when a root directory!
    let mut full_dir : String = String::from(g.source_dir); 
    full_dir.push_str(cur_dir); 
    debug!("scan_source_dir: full_dir (source_dir + cur_dir) : '{}' ", &full_dir ); 

    let full_dir_mode = get_mode(&g.platform, &full_dir ); 
    if full_dir_mode == 0 {
        let err = format!("scan_source_dir: full_dir '{}' mode was zero!", &full_dir);
        error!("{}", err);
        line();
        return Err(err);
    }

    let file_data = FileData{
        file_level : level, 
        file_type : 'd', 
        file_mode : full_dir_mode, 
        file_user: "".to_string(), 
        file_group: "".to_string(), 
        default_file_mode : 0, 
        default_file_user: "".to_string(), 
        default_file_group: "".to_string()
    };

    g.file_source_map.insert(cur_dir.to_string() , file_data);

    //NOTE: read_dir SHOULD skip/ignore the . and '..' entries. 
    //Perl included them by default, so explicit removal in that script. 
    if let Ok(entries) = fs::read_dir(&full_dir) {

        for entry in entries {
            let this_dir = entry.unwrap();

            let res_this_dir = &this_dir.file_name().into_string();
            let str_this_dir : String ; 
            if res_this_dir.is_ok() {
                str_this_dir = res_this_dir.as_ref().unwrap().to_string(); 
            }else {
               let err = format!("scan_source_dir: could not convert file_name() into String " ); 
               error!("{}", err); 
               line(); 
               return Err(err); 
            }

            debug!("scan_source_dir: (entry) str_this_dir : {}  ", &str_this_dir ); 
            debug!("scan_source_dir: &full_dir : {}  ", &full_dir );

            let this_full_path = Path::new( &full_dir ).join( &str_this_dir );
            debug!("scan_source_dir: this_full_path.display() : '{}' ", this_full_path.display()); 

            let hash_key = Path::new(cur_dir).join(&str_this_dir); 
            let hash_key = hash_key.to_str(); 
            if hash_key.is_none(){
                let err = "hash_key did NOT convert to utf8 string correctly!"; 
                error!("{}", &err); 
                line();
                return Err(err.to_string()); 
            }
            let hash_key : String = hash_key.unwrap().to_string(); 

            debug!("&hash_key is '{}'", &hash_key); 

            if this_full_path.is_file(){
                debug!("scan_source_dir: this_full_path IS file."); 

                let this_file_mode: i32; 
                let str_file_name = this_full_path.to_str(); 
                if str_file_name.is_some(){
                    this_file_mode = get_mode(&g.platform, str_file_name.unwrap() );
                    if this_file_mode == 0 {
                        let err = format!("scan_source_dir: file name '{}' mode returned zero.", str_file_name.unwrap( ) ); 
                        error!("{}", err); 
                        line();
                        return Err(err);
                    }
                }else{
                    line(); 
                    return Err("this_full_path NOT a correct utf-8 string.".to_string()); 
                }

                let file_data = FileData{
                    file_level : level, 
                    file_type : 'f', 
                    file_user : "".to_string(), 
                    file_group : "".to_string(), 
                    file_mode : this_file_mode, 
                    default_file_mode: 0 , 
                    default_file_user: "".to_string(), 
                    default_file_group: "".to_string()
                };

                g.file_source_map.insert(hash_key.clone() , file_data);

            } else if this_full_path.is_dir() {
                debug!("scan_source_dir: this_full_path IS directory."); 
                let res = scan_source_dir(g , &hash_key , level + 1);
                if res.is_err(){
                    let err = format!("scan_source_dir: ERROR: scan_source_dir raised err: {} ", res.unwrap_err() );
                    error!("{}", &err); 
                    line();
                    return Err(err); 
                }
            }else {
                error!("Do NOT know what type of file this is!"); 
            }
            
        }//end for-loop entries. 

    } else {
        let err = format!("scan_source_dir: ERROR: cannot read dir: '{}'" , &full_dir); 
        error!("{}", err); 
        line();
        return Err(err); 
    }
    line();
    Ok(true)
}//}}}

