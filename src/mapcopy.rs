extern crate xml; 
extern crate minidom;
extern crate regex;
use std::env;
use std::time::SystemTime;
use std::time::Duration;
//use std::fs::{self,File,DirEntry};
use std::fs::{self};
use std::path::{Path,PathBuf};
use std::process::Command;
use std::collections::HashMap;
use std::io; //::{stdin,Result};
use minidom::Element;
use regex::*;
//#####################################################
//# part 1. 
//#   - should of cp-ed the base and specific base_variant dir into the swap dir
//#   - re-created the failsafe default http website pages 
//#   - NO chown should be done yet as Vagrant ( or Docker ) does not have permission to chown the 'troy' user owned files 
//#
//# part 2 - this file 
//#   - chown the swap/temp dir to correct ACLs etc 
//#   - use the XML spec file for lookup 
//#
//#to be run on PROD/AWS EC2 or Vagrant only
//
//#perform rsync or similar on all the required dirs/files into the target (normally LIVE/PROD) server!!!
//#####################################################

//#Hardcoded value to prefix the target destination for testing
const TEST_PREFIX: &'static str = "/home/troy/Downloads/mapcopy_test";
//const TEST_PREFIX: &'static str = "";
const VERSION: &'static str ="0.1.2"; 

struct RunParams<'a> { 
dry_run: bool, 
force_yes: bool,
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
    default_file_mode:  String 
}

pub fn run<'a>() -> i32 {//{{{

    let mut g = RunParams{
        dry_run: false, 
        force_yes: false,
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

    let args: Vec<String> = env::args().collect();

    // The first argument is the path that was used to call the program.
    println!("-------------------------------------------------------------");
    println!("----------------project_tree (Rust)--------------------------"); 
    println!("-------------------------------------------------------------");
    println!("Version: {}", VERSION); 
    println!("run: working path: {}.", args[0]);

    // The rest of the arguments are the passed command line parameters.
    // Call the program like this:
    //   $ ./args arg1 arg2
    println!("run: arguments: {:?}.",  &args[1..]);


    /*
     * mapcopy.exe [development|live] [force_yes:yes]
     * dry_run when NOT live
     */

    let arg_len = args.len() - 1; 

    g.mode = if arg_len >= 1 {
            &args[1]
        }
        else{
            ""
        };

    g.force_yes = if arg_len == 2 { 
                "yes" == &args[2]
            }
            else{
                false
            };

    g.dry_run = if "live" == g.mode {
                false
            }else{
                true
            };



    let fields = get_base();
    g.swap_dir = &fields["swapdir"];
    g.target = &fields["target"];
    g.build_name = &fields["buildname"]; 
    let tmp_backup_dir = format!("{}/base_backup_BUILD_{}", &g.swap_dir, &g.target);
    g.backup_dir = tmp_backup_dir.as_str();
    g.build_dir = &fields["build_dir"];
    let tmp_logfile_dir = format!("{}/rsync_log", &g.swap_dir);
    g.logfile_dir = tmp_logfile_dir.as_str();
    g.source_dir = &g.build_dir;
    g.config_dir = &fields["configdir"]; 


    let web_owner = if is_platform("Alpine") {
       "apache"  
    }else {
        "http"
    };

    println!("___________________________________________________"); 
    println!("Forcing a 'yes' entry for any user-input?: {}", g.force_yes);
    if g.dry_run {
        println!("Running in DRY-RUN mode for rsync, no changes saved!!!");
    }
    println!("mode: {}", g.mode); 
    println!("swap_dir: {}", g.swap_dir); 
    println!("target: {}", g.target); 
    println!("build_name: {}", g.build_name); 
    println!("backup_dir: {}", g.backup_dir); 
    println!("build_dir: {}", g.build_dir); 
    println!("logfile_dir: {}", g.logfile_dir); 
    println!("source_dir: {}", g.source_dir); 
    println!("config_dir: {}", g.config_dir); 
    println!("web/http area owner: {}", web_owner) ;
    println!("___________________________________________________"); 

    if g.target == "NULL" {
        println!( "ERROR: target is NULL. ");
        return 1; 
    }

    //return 100; 
    clean_backup_dir(&g); 
    setup_logfile_dir(&g); 

    //3rd param: boolean: delete extra files in target, 
    simple_copy(&mut g, "/var/www/html/sites/default" , false, web_owner, web_owner );  
    simple_copy(&mut g, "/var/www/html/sites/default_http" , false, web_owner, web_owner );  
    simple_copy(&mut g, "/var/www/html/sites/default_https" , false, web_owner, web_owner );  

    map_copy(&mut g, "/etc/httpd/conf" , false );  
    map_copy(&mut g, "/etc/apache2" , false );  
    map_copy(&mut g, "/etc/postfix" , false );  
    map_copy(&mut g, "/etc/postgresql" , false );  
    map_copy(&mut g, "/etc/php" , false );  
    map_copy(&mut g, "/etc/php8" , false );  
    map_copy(&mut g, "/var/lib/postgres" , false );  
    map_copy(&mut g, "/var/lib/postgresql" , false );  
    map_copy(&mut g, "/root", false );  
    map_copy(&mut g, "/home/vagrant", false );  
    map_copy(&mut g, "/home/arch", false );  
    map_copy(&mut g, "/home/alpine", false );  
    map_copy(&mut g, "/etc/logrotate.d" , false );  

    //TODO: all these mapcopy params can be sourced when the text config file
    simple_copy(&mut g, "/etc/redis.conf" , false, "redis", "redis" );  
    //TODO: all these mapcopy params can be sourced when the text config file
    simple_copy(&mut g, "/etc/ssl_self" , false , "root", "root" );  
    //TODO: all these mapcopy params can be sourced when the text config file
    simple_copy(&mut g, "/etc/letsencrypt" , false , "root", "root" ); 

    return 0;

}//end fn}}}

fn run_command(command : &str ) -> (bool, String ) {//{{{
    println!("running command: '{}' " , command); 
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
            |e| { panic!("PANIC! run_command: Failed to execute process: '{}' ", e)
        });

    let result:bool ; 
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
    println!("clean_backup_dir: backup_dir: '{}'", g.backup_dir); 
    if g.backup_dir == "/" {
        println!("ERROR: backup_dir is root! exiting now.");
        return false;
    }

    let dry_run_failsafe = if g.dry_run {
        println!("NOTE: running in dry_run mode, adding 'echo' before command call!");
        "echo " 
    }else {
        ""
    };

    let cmd = format!("{}rm -rf {}", dry_run_failsafe, g.backup_dir );
    let (result , raw_output) = run_command(&cmd);

    if result {
        println!("clean_backup_dir: remove dir result: '{}'", raw_output);

    } else {
        println!("clean_backup_dir: failed to remove dir result: '{}'", raw_output);
        return false;
    }
        
    let cmd = format!("{}mkdir -p {}", dry_run_failsafe,  g.backup_dir );
    let (result , raw_output) = run_command(&cmd);

    if result {
        println!("clean_backup_dir: mkdir result: '{}'", raw_output);

    } else {
        println!("clean_backup_dir: failed to mkdir result: '{}' ", raw_output);
        return false;
    }

    return true
}//}}}

fn setup_logfile_dir( g: &RunParams ) -> bool {//{{{
    println!("setup_logfile_dir: logfile_dir: {}" , g.logfile_dir) ;

    let dry_run_failsafe  = if g.dry_run { "echo " } else { "" };

    //CAUTION: Openbsd does NOT have -v for mkdir command!
    let cmd = format!("{}mkdir -p {}", dry_run_failsafe,  g.logfile_dir );

    let (result, raw_output) = run_command(&cmd); 

    println!("setup_logfile_dir: result: '{}' " , raw_output); 
    result
}//}}}

fn is_platform( test_platform : &str) -> bool{//{{{

    let cmd_uname = "uname"; 
    let output = Command::new(cmd_uname)
                    .arg("-a")
                    .output()
                    .unwrap_or_else(
                            |e| { panic!("failed to execute process: {}", e)
                        });
    let raw_output = 
        if output.status.success() {
                String::from_utf8_lossy(&output.stdout)
        } else {
                String::from_utf8_lossy(&output.stderr)
        };

    println!( "is_platform: uname -a: '{}' ", raw_output); 
    let result = if raw_output.contains(test_platform)  {
        println!( "is_platform: '{}' is true ", test_platform );
        true
    }else{
        println!( "is_platform: '{}' is false ", test_platform );
        false
    };
    result
}//}}}

fn get_base<'a>() -> HashMap<String, String> {//{{{
// the bash script must output the var as 
// foo: value
// foo: value
// ...and this code will parse that

//   my %_fields; 
    let mut fields: HashMap<String,String> = HashMap::new(); 

    let cmd = "./base_setup.sh";
    if Path::new(cmd).exists() == false {
        println!("get_base: ERROR: 'base_setup.sh' files does not exist."); 
        return fields; 

    }

    let (ok , raw_output) = run_command(&cmd);

    if ok {
        let re = Regex::new("^(.*?): (.*)$").unwrap();
        for line in raw_output.lines(){
            let caps = re.captures(&line).unwrap();
            let key = String::from ( caps.get(1).map_or("", |m| m.as_str()) ); 
            let value = String::from( caps.get(2).map_or("", |m| m.as_str()) );
            fields.insert(key, value); 
        }
        for (key,val)  in fields.iter() {
            println!("field key: '{}' , val: '{}' " , key, val ) ; 
        }
    }
    else{
        println!("setup not okay"); 
    }

    fields 
}//}}}

fn scan_tree<'a >( //{{{
    g: &'a mut RunParams, 
    cur_path: String, 
    cur_dir : &'a Element , 
    file_level: i32 , 
    parent_default_file_user: String, 
    parent_default_file_group: String, 
    parent_default_file_mode: String ) {
// Recurse/iterate into each dir and create a hashtable of all the files/dirs to 
// compare against the filesystem candidate to be uploaded. 
// Either it's own settings or go to the parents value. ...so it trickles down.

    println!("scan_tree: cur_path: '{}'" , &cur_path); 
    println!("scan_tree: cur_dir: '{}'" , cur_dir.name() ); 
    println!("scan_tree: file_level: '{}'" , file_level ); 
    println!("scan_tree: parent_default_file_user: '{}'" , parent_default_file_user ); 
    println!("scan_tree: parent_default_file_group: '{}'" , parent_default_file_group ); 
    println!("scan_tree: parent_default_file_mode: '{}'" , parent_default_file_mode ); 

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

    let default_file_mode = if cur_dir.attr("default_file_mode").is_none() { 
                                parent_default_file_mode.clone()
                            } else { 
                                cur_dir.attr("default_file_mode").unwrap().to_string().clone()
                            };

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

    let file_mode = if cur_dir.attr("mode").is_none() {
                                "0".to_string()
                            } else {
                                cur_dir.attr("mode").unwrap().to_string()
                            };

    let file_mode = file_mode.parse::<i32>().unwrap_or_default();

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


    g.file_map.insert(cur_path.to_string(), file_data); 

    for node in cur_dir.children() {
        
        let new_cur_path = format!( "{}/{}",cur_path,node.attr("name").unwrap()); 

        if node.name() == "directory" {
            scan_tree(
                g, 
                new_cur_path,
                &node, 
                file_level + 1 , 
                default_file_user.clone(), 
                default_file_group.clone(), 
                default_file_mode.clone());
                
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

            g.file_map.insert( new_cur_path , file_data ); 

        } else {

            println!("ERROR Unexpected node name: '{}' ", node.name() );
        }

    }

}//end fn}}}

fn simple_copy( //{{{
    g: &mut RunParams, 
    path_dir: &str ,
    delete: bool , 
    file_user: &str, 
    file_group: &str 
) -> bool {
// simple rsync version just for default-website for e.g , no xml tree etc 
    println!("simple_copy: path_dir: '{}'", path_dir); 
    println!("simple_copy: delete: '{}'", delete); 
    println!("simple_copy: file_user: '{}'", file_user); 
    println!("simple_copy: file_group: '{}'", file_group); 

    let mut source = format!("{}{}", g.source_dir , path_dir); 
    println!( "simple_copy: source: '{}' " , source ) ; 
    let target = format!("{}{}", TEST_PREFIX , path_dir);
    
    if Path::new(String::as_str(&source)).exists() == false {
        println!( "simple_copy: Error: '{}' does not exist!", source);
        return false;
    }

    let dry_run_failsafe = if g.dry_run { "echo " } else { "" }; 

    if Path::new(String::as_str(&source)).is_dir() {
        // Add the slash to start copying the contents that follows the end dir and NOT the dir itself
        source = format!("{}/", &source); 
        //Caution: Openbsd does not have -v argument for mkdir
        let command = format!("{}mkdir -p {}" , dry_run_failsafe, target) ; 
        let (ok,result) = run_command(&command); 
        if !ok {
            println!("simple_copy: ERROR: mkdir failed: '{}'", result); 
            return false
        }
    }

    let mut logfile_part = path_dir.to_string();
    logfile_part = logfile_part.replace("/", "_");

    let chown = if !file_user.is_empty() && !file_group.is_empty() {
                    format!(" --chown={}:{} ", file_user, file_group)
                } else { "".to_string() } ;

    let rsync_delete = if delete {
       " --delete ".to_string()  
    } else {
        "".to_string()
    };

    let rsync_dryrun = if g.dry_run { "--dry-run" } else { "" }; 


    let rsync_backup = format!( " --backup --backup-dir={}{} ", g.backup_dir, path_dir );
    let default_duration = Duration::ZERO;
    let seconds_now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or(default_duration).as_secs();
    let rsync_logfile = format!( " --log-file={}/{}_{}.log ", g.logfile_dir, logfile_part, seconds_now );
    let rsync_switches = format!(" {} -v -a --human-readable  {} {} {} {}", 
                                 rsync_dryrun, rsync_delete, chown, rsync_backup, rsync_logfile ); 
    let rsync = format!("rsync {} {} {} ", rsync_switches, source, target);

    println!("Executing rsync call: '{}' ", rsync); 
    let (ok, raw_output) = run_command(rsync.as_str()); 
    if ok {
        println!("simple_copy: rsync result: '{}' " , raw_output); 
    }

    return true;

}//end fn}}}
 
fn scan_source( //{{{
    g: &mut RunParams,  
    path_dir: &str ){
//#create hashtable for the filesystem structure to then do a acl/mode comparision against .
    println!("scan_source: path_dir: '{}'", path_dir) ; 
    let result = scan_source_dir(g, path_dir, 0);
    if result.is_err() {
        println!("scan_source: ERROR in scan_source_dir call: '{}'", result.unwrap_err()); 
    }
    let ret = show_prelim(false, g);
    if ret.is_err(){
        println!("scan_source: ERROR from show_prelim: '{}'" , ret.unwrap_err()); 

    }
}//}}}

fn get_parent_perms<'a>(//{{{
        g: &'a RunParams, 
        /*file_map: &'a mut HashMap<String, FileData >, */
        key_path : &'a str) -> Result<(FileData, String), String > {
//this filepath does NOT exist in the XML Treepath, 
//so do up a level and get the default values. 

    println!("get_parent_perms: keypath:'{}' ",  key_path);

    //let dirs: Vec<&str> = key_path.split('/').collect();
    let last_dir_pos = key_path.rfind('/').unwrap_or(0);
    let last_dir = if last_dir_pos > 0 {
        let tmp = &key_path[0..last_dir_pos];
        tmp
    }else {
        ""
    };

    println!("get_parent_perms: last_dir: '{}' ", last_dir); 

    let default_file_data = FileData{ 
        file_level:  -1, 
        file_type: char::REPLACEMENT_CHARACTER, 
        file_mode: 0, 
        file_user: "".to_string(), 
        file_group: "".to_string(), 
        default_file_user: "".to_string(), 
        default_file_group: "".to_string(),
        default_file_mode: "".to_string(), 
    };

    if !g.file_map.contains_key(last_dir){
        let err = format!( "ERROR: get_parent_perms: there is no key in the XML spec tree for '{}'", last_dir).clone(); 
        println!("{}", &err); 
        println!("Adjust XML spec or similar"); 
        return Err(err);
    }
    
    let item = g.file_map.get(last_dir).unwrap_or(&default_file_data); 
    let ret_item = item.clone();
    
    Ok((ret_item, last_dir.to_string()))


}//}}}

fn map_copy( //{{{
    g: &mut RunParams, 
    path_dir: &str , 
    delete: bool ) -> i32 {
//# open a xml tree spec to get mode/user/group etc 
//# recurse into all directory elements to get all file elements etc 
//# populate the hash tree with the full file path for easy lookup 
//#pass over to copysourcefiles with delete param for rsync to decide if to rm extra files NOT in source dir.  
    println!("map_copy: delete: '{}'", delete); 

    if !Path::new(path_dir).exists() {
        println!( "################################################################");
        println!( "map_copy: 'path_dir' parameter '{}' not found on filesystem. ", path_dir);
        println!( "Not performing map_copy!");
        println!( "################################################################");
        return -1; 
    }

    //replace / . with _ chars for filename component. 
    let mut file_part = path_dir.to_string().clone(); 
    file_part = file_part.replace('/', "_");
    file_part = file_part.replace('.', "_");

    //let null_value = "NOT_FOUND";
    let file_name = format!("{}/base_TREE_SPECS/spec{}.xml", g.config_dir, file_part);

    println!( "map_copy: XML Spec Treefile: '{}' ",  file_name );
    
    if !Path::new(&file_name).exists(){
        println!("map_copy: File spec '{}' not found.", file_name); 
        return -1;
    }


    let res = fs::read_to_string(&file_name); 
    if !res.is_ok(){
        return -1; 
    }
    let buffer = res.unwrap();

    let root: Element = buffer.parse().unwrap();
    for child in  root.children(){
        //should only be ONE MAIN DIR/ROOT DIR
        if child.name() == "directory" {
            let name = child.attr("name").unwrap_or("NOT_FOUND");
            scan_tree(g, name.to_string() , child, 0, "".to_string() ,"".to_string(),"".to_string() );
            //now scan source file dir created hashtable. 
            //recusrse into real build directory and cross-ref the mode/user/group from the hashtable. 

            scan_source(g, path_dir); 
            let ret = copy_source_files(g, path_dir, delete);
            if ret == false {
                println!("map_copy: ERROR from copy_source_files."); 
            }

        }

    }


    return 0;

} //end fn}}}

fn copy_source_files( //{{{
    g: &mut RunParams, 
    path_dir: &str, 
    delete: bool) -> bool {
//re-chmods the files/dirs that are in the preset TMP dir --NOT the target files 
//re-chowns the '' '' ''
//THEN rsync that dir structure across.

    println!("copy_source_files: path_dir: '{}'", path_dir); 
    println!("copy_source_files: delete: '{}'", delete); 

    for (key, item) in &g.file_source_map {

        let dry_run_failsafe = if g.dry_run { "echo " } else { "" } ;

        let source_file = format!("{}{}",  &g.source_dir ,  key); 
        println!("copy_source_files: source_file: '{}'", source_file); 

        let cmd_chown = format!("{}chown {}:{} {}" , dry_run_failsafe, item.file_user, item.file_group, source_file); 
        let (ok,raw_output) = run_command(&cmd_chown); 
        if !ok {
            println!("copy_source_files: ERROR: chown failed: '{}'", raw_output); 
            return false; 
        }
        let cmd_chmod = format!("{}chmod {} {}" , dry_run_failsafe, item.file_mode, source_file); 
        let (ok,raw_output) = run_command(&cmd_chmod); 
        if !ok {
            println!("copy_source_files: ERROR: chmod failed: '{}'", raw_output); 
            return false; 
        }
    }

    let logfile_part = path_dir.replace('/',"_");
    let rsync_dryrun = if g.dry_run { "--dry-run" } else { "" }; 
    let rsync_switches = format!("{} -a --human-readable --verbose  ", rsync_dryrun );
    let rsync_backup = format!( " --backup --backup-dir={}{} ", g.backup_dir, path_dir);
    let default_duration = Duration::ZERO;
    let seconds_now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or(default_duration).as_secs();
    let rsync_logfile = format!(" --log-file={}/{}_{}.log ", g.logfile_dir, logfile_part, seconds_now );
    let rsync_delete = if delete { " --delete " } else { "" };

    //prefix normally /home/foo/Downloads/perl_test to safeguard against overcopy.
    let mut target_dir = String::from(TEST_PREFIX); 
    target_dir.push_str(path_dir);

    //Caution: Openbsd does not have -v argument for mkdir
    let mkdir_target = format!( "mkdir -p {} ", target_dir);
    let (ok , raw_output) = run_command(&mkdir_target);
    if !ok {
        println!("copy_source_files: failed to run mkdir: '{}' " , raw_output);
        return false; 
    }

    //IMPORTANT! use the trailing  '/' at end of rsync source to avoid starting at the dir, ..so to get contents of the dir.
    let rsync = format!( "rsync {} {} {} {} {}{}/ {} ", 
                            rsync_switches, rsync_delete, rsync_backup, rsync_logfile , g.source_dir, path_dir, target_dir );
    println!( "copy_source_files: rsync command: '{}' " ,  rsync );

    let (r_ok, r_raw_output) = run_command(&rsync); 
    if !r_ok {
        println!("copy_source_files: FAILED: rsync: '{}'", r_raw_output); 
        return false; 
    }

    println!("copy_source_files: rsync result: '{}' " , r_raw_output); 
    true

}//end fn}}}

fn show_prelim(//{{{
    re_show: bool, 
    g: &mut RunParams, 
    ) -> Result<i32,String> {
    //show to user What will happen re file Mode, Missing etc   
    //iterate the xmltree first then the filesys source tree 
    println!("show_prelim: re_show: '{}'", re_show);
    println!("show_prelim: force_yes: '{}'", g.force_yes);
    println!("-------------------------------------------------------------------------------");
    println!( "XML Tree spec map: ");
    println!( "Definition: ??? - File missing from XML spec master file.");

    for (key,item) in &g.file_map {
        let tag = if g.file_source_map.contains_key(key) {
            "   ".to_string()
        }else{
            "???".to_string()
        };

        println!( "{} File: (lv {})({}) {} {}:{}  {}",
                  tag, item.file_level, item.file_type, key, item.file_user, item.file_group, item.file_mode);

    }

    println!( "Filesystem source map...");
    println!("??? = File not mentioned in XML Tree spec. ");
    println!("XXX = File's mode will be overridden to match the XML file spec. ");
    let mut new_file_source_map_items: HashMap<String,FileData> = HashMap::new(); 

    for (key, item) in &g.file_source_map {

        if g.file_map.contains_key(key){
            //it exists in the XML treemap...
            //the fileSourceMap CANNOT really have the target user/group as it is coming from a dev machine anyway. 
            let mut msg = String::new();
            let mut tag = String::new(); 
            let mut tmp = item.clone(); 
            //RESET value to match the XML spec...
            tmp.file_user = g.file_map[key].file_user.clone();
            tmp.file_group = g.file_map[key].file_group.clone();
            tmp.file_mode = g.file_map[key].file_mode.clone();

            if g.file_map[key].file_mode != item.file_mode {
                tag = "XXX".to_string();
                msg = format!("**Override** {} --> {} ", item.file_mode, g.file_map[key].file_mode );
            } else {
                //missing file
                //get last dir / go up a dir and get the default perms for that file. 
                let result = get_parent_perms(g, &key); 
                if result.is_ok() {
                    let (perms, last_dir) = result.unwrap();  
                    tag = "???".to_string();
                    msg = format!( "**Missing** (owner dir: {} )", last_dir );
                    tmp.file_user = perms.default_file_user;
                    tmp.file_group = perms.default_file_group;

                    let new_file_mode: i32 = perms.default_file_mode.parse().unwrap_or_default();
                    if new_file_mode == 0 {
                        println!("show_prelim: file_mode parse error: default_file_mode did not parse to int:'{}'", 
                                 perms.default_file_mode); 
                       return Err("filemode is zero".to_string()); 
                    }
                    tmp.file_mode = new_file_mode;
                };
            }

            new_file_source_map_items.insert(key.to_string(), tmp); 

            println!( "{} File: (lv {})({}) {} {}:{} {} {} ", 
                        tag, 
                        item.file_level, 
                        item.file_type, 
                        key, 
                        item.file_user,
                        item.file_group, 
                        item.file_mode, 
                        msg
                      );
        }//end contains 
    }//end for

    for (key, item) in new_file_source_map_items{
       g.file_source_map.insert(key, item); 
    }


    println!( "tree spec count: {} ", g.file_map.len());
    println!( "file source count {} ", g.file_source_map.len()); 


    if g.force_yes {
        println!( "FORCING a 'Yes' for all would-be user input!");
    }else {
        println!( "Considering all above, proceed with the file copy tasks? y/N");

        let mut buffer = String::new();
        let stdin = io::stdin(); // We get `Stdin` here.
        let res = stdin.read_line(&mut buffer);
        if !res.is_ok() {
            return Err("bad read_line".to_string()); 
        }
        if  buffer == "y" || buffer == "Y" || buffer == "" {
            if !re_show {
                let ret = show_prelim(true, g);
                if ret.is_err(){
                    let str_err = ret.unwrap_err(); 
                    let str_err = format!("show_prelim (recursive return): ERROR: '{}'", &str_err ) ;
                    return Err(str_err); 
                }
            }
            println!("Processing...");
        } else if buffer == "N" {
            println!("Ending now."); 
            return Ok(-1); 
        } else {
            println!( "Couldn't understand response. Terminating now. ");
            return Ok(-2);
        }
    }

    return Ok(1); 

} //end fn}}}

fn get_mode(uri: &str) -> i32 {//{{{
//do a file stat to get the Mode. 
//the perl chmod NEEDS an octal value input! 
//fyi: at THIS stage, it seems the result is bitmasked and output for the decimal output etc 
//but please note the octal printout format AND the bitwise mask 
    if uri != "" {
        let cmd = format!("stat -c %a {}" , uri) ;
        let (ok, raw_output) = run_command(cmd.as_str());
        if ok {
            let parsed = raw_output.parse::<i32>();
            if parsed.is_ok(){
                return parsed.unwrap(); 
            }else {
                return 0;
            }
        }
    }

    return 0;

}//end fn}}}

fn scan_source_dir( //{{{
        g: &mut RunParams, 
        cur_dir: &str,
        level: i32 ) -> Result<bool,String> {
//recusive scan into filesystem sourcedir to create hashmap of filesdirs
//to crossref with xml trees version 

    let mut full_dir: PathBuf = PathBuf::new();
    full_dir.push(g.source_dir);
    full_dir.push(cur_dir);

    let full_dir_mode = get_mode(full_dir.to_str().get_or_insert("") ); 
    if full_dir_mode == 0 {
        let err = format!("scan_source_dir: full_dir '{}' mode was zero!", full_dir.to_str().get_or_insert(""));
        println!("{}", err);
        return Err(err);
    }
    let file_data = FileData{
        file_level : level, 
        file_type : 'd', 
        file_mode : full_dir_mode, 
        file_user: "NULL".to_string(), 
        file_group: "NULL".to_string(), 
        default_file_mode : "".to_string(), 
        default_file_user: "".to_string(), 
        default_file_group: "".to_string()
    };

    // this was in original perl script as comment...    
    // #[$level, $curdir, getMode($fulldir) ];
    g.file_source_map.insert(cur_dir.to_string() , file_data);

    if let Ok(entries) = fs::read_dir(&full_dir) {
        for entry in entries {
            let dir = entry.unwrap();
            //NOTE: read_dir should skip/ignore the . and .. entries Perl included them 

            //let full_name = format!("{}/{}", &full_dir , &dir.path().as_str() );
            let mut this_path: PathBuf = full_dir.clone(); 
            this_path.push(dir.path());
            //let path = Path::new(&full_name); 
            if this_path.is_file(){
                //let hash_key = format!("{}/{}", cur_dir, dir.path().as_str()? );
                let str_file_name: &str = this_path.to_str().get_or_insert(""); 
                let hash_key = &str_file_name; 
                let this_file_mode = get_mode(str_file_name);
                if this_file_mode == 0 {
                    let err = format!("scan_source_dir: file name '{}' mode was zero", str_file_name); 
                    return Err(err);
                }
                let file_data = FileData{
                    file_level : level, 
                    file_type : 'f', 
                    file_user : "NULL".to_string(), 
                    file_group : "NULL".to_string(), 
                    file_mode : this_file_mode, 
                    default_file_mode: "".to_string(), 
                    default_file_user: "".to_string(), 
                    default_file_group: "".to_string()
                };
                g.file_source_map.insert(hash_key.to_string() , file_data);
            } else if this_path.is_dir() {
                let mut scan_dir_path = PathBuf::new(); 
                scan_dir_path.push(cur_dir); 
                scan_dir_path.push(this_path); 
                let p = scan_dir_path.as_path().to_string_lossy(); //.get_or_insert(""); 
                let res = scan_source_dir(g , &p , level + 1);
                if res.is_err(){
                   return Err("ERROR: early termination for scan_source_dir call".to_string()); 
                }
            }
            
            println!("{:?}", dir.path());
        }

    } else {
        let err = format!("ERROR: cannot read dir: '{}'" , &full_dir.display()); 
        println!("{}", err); 
        return Err(err); 
    }
    Ok(true)
}//}}}

