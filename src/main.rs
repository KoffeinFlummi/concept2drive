use std::collections::HashMap;
use std::io::Write;
use std::path::{Path,PathBuf};

use colored::*;
use docopt::Docopt;
use serde::Deserialize;

mod api;
mod drive;
mod error;
mod native;
mod workouts;

use api::*;
use drive::*;
use error::*;

const VERSION: &'static str = "v0.1";
const USAGE: &'static str = "
Usage:
    concept2drive info <device>
    concept2drive init <device> [<username>]
    concept2drive list-workouts <device> [-n <num>]
    concept2drive show-workouts <device> [<workout>]
    concept2drive update-firmware <device> [--beta]
    concept2drive (-h | --help)
    concept2drive --version

Commands:
    info                Show general information about the flash drive.
    init                Set up a new drive at the given path. If no user name
                        is given, $USER is used. Name must be <= 6 characters.
    list-workouts       List the workouts stored on the drive.
    show-workout        Show detailed information about a specific workout.
                        The workout can be identified either with the ID listed
                        in the output of list-workouts, or by date.
                        If no workout is given, the last one is displayed.
    update-firmware     Update firmwares on the drive.

Options:
    -h --help           Show usage information.
    --version           Show version.
    -n --last=<num>     Only show <num> latest workouts.
    --beta              Include beta firmwares.
";

#[derive(Debug, Deserialize)]
struct Args {
    cmd_info: bool,
    cmd_init: bool,
    cmd_list_workouts: bool,
    cmd_show_workouts: bool,
    cmd_update_firmware: bool,
    arg_device: Option<String>,
    arg_username: Option<String>,
    flag_last: Option<usize>,
    flag_beta: bool,
}

#[derive(Debug, Default)]
pub struct CliError {
    msg: String
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for CliError {}

macro_rules! error_from {
    ( $t:ty ) => {
        impl From<$t> for CliError {
            fn from(error: $t) -> Self {
                CliError { msg: format!("{}", error) }
            }
        }
    }
}

error_from!(ParserError);
error_from!(std::io::Error);
error_from!(reqwest::Error);
error_from!(xdg::BaseDirectoriesError);

/// Download firmware file to target path while printing progress bar.
async fn download_file_progress(
    file: &FirmwareFile,
    target_path: PathBuf
) -> Result<(),CliError> {
    let client = reqwest::Client::new();

    // Send HEAD request to get file size
    let resp = client.head(&file.path).send().await?;
    let size: usize = resp.headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|ct_len| ct_len.to_str().ok())
        .and_then(|ct_len| ct_len.parse().ok())
        .unwrap();

    // Setup progress bar
    let mut template = "{spinner:.bold.green} ".to_string();
    template += &format!("{:47}", file.name);
    template += " [{bar:40.bold.green/white}] {bytes}/{total_bytes} ({eta})";

    let pb = indicatif::ProgressBar::new(size as u64);
    pb.set_style(indicatif::ProgressStyle::default_bar()
         .template(&template)
         .progress_chars("##-"));

    // GET file using a wrapped reader to update progress bar
    let resp = client.get(&file.path).send().await?;
    let bytes = resp.bytes().await?;
    let mut reader = pb.wrap_read(&*bytes);
    let mut target = std::fs::File::create(target_path)?;

    std::io::copy(&mut reader, &mut target)?;
    pb.finish();

    Ok(())
}

/// Check available versions and download those not present in the local cache
/// already.
fn update_firmware_cache() -> Result<Vec<FirmwareVersion>,CliError> {
    let mut rt = tokio::runtime::Runtime::new().unwrap();

    // Request list of versions
    let versions = rt.block_on(FirmwareVersions::download())?;

    let mut updated = false;
    for version in &versions.data {
        // Get default file of firmware version
        let file = version.files.iter().find(|f| f.default);
        if file.is_none() {
            continue;
        }

        let file = file.unwrap();

        // Get cache path
        let local_path = xdg::BaseDirectories::new()?
            .place_cache_file(Path::new("concept2drive")
            .join("firmware")
            .join(&file.name))?;

        // Skip file if present already
        if local_path.is_file() {
            continue;
        }

        if !updated {
            println!("Downloading firmwares...");
            updated = true;
        }

        // Download firmware
        rt.block_on(download_file_progress(file, local_path))?;
    }

    println!("Firmware cache up-to-date.");

    Ok(versions.data)
}

/// Ask user for confirmation.
fn confirm(msg: String) -> Result<bool,CliError> {
    let mut stdout = std::io::stdout();
    print!("{} ({}/{}): ", msg, "y".bold().green(), "N".bold().red());
    stdout.flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.to_lowercase() == "y\n")
}

/// info command
fn cmd_info(args: Args) -> Result<(),CliError> {
    let mut drive = Drive::new(args.arg_device.unwrap(), false)?;

    let (user_id, user_name) = drive.user()?;
    let workouts = drive.workouts()?;
    let firmwares = drive.firmwares()?;

    // TODO: include personal bests?

    println!("{:<24}{}", "User Name:".bold().green(), user_name);
    println!("{:<24}{}", "User ID:".bold().green(), user_id);
    println!("{:<24}{}", "Workouts:".bold().green(), workouts.len());
    println!("{:<24}{}", "Lifetime Meters:".bold().green(), workouts.iter().map(|w| w.total_distance).sum::<u32>());
    println!("{:<24}{:.3}", "Lifetime kWh:".bold().green(), workouts.iter().map(|w| w.watts() * w.total_work_duration.as_secs() as f64 / 3600000.0).sum::<f64>());
    println!("{:<24}{:.0}", "Lifetime kcal:".bold().green(), workouts.iter().map(|w| w.cal_hr() * w.total_work_duration.as_secs() as f64 / 3600.0).sum::<f64>());

    if workouts.len() > 0 {
        println!("{:<24}{}", "First Workout:".bold().green(), workouts[0].datetime.format("%Y-%m-%d %H:%M"));
        println!("{:<24}{}", "Last Workout:".bold().green(), workouts[workouts.len()-1].datetime.format("%Y-%m-%d %H:%M"));
    }

    if firmwares.len() == 0 {
        println!("{:<24}{}", "Installed Firmwares:".bold().green(), "none");
    }

    for (i, firmware) in firmwares.iter().enumerate() {
        println!("{:<24}- {}", if i == 0 { "Installed Firmwares:" } else { "" }.bold().green(), firmware);
    }

    Ok(())
}

/// init command
fn cmd_init(args: Args) -> Result<(),CliError> {
    let mut name = args.arg_username;
    if name.is_none() {
        name = std::env::var("SUDO_USER").ok();
    }
    if name.is_none() {
        name = std::env::var("USER").ok();
    }

    let device = args.arg_device.unwrap();

    println!("About to overwrite all data on {}!", &device);

    if !confirm("Proceed?".to_string())? {
        println!("Aborted.");
        return Ok(());
    }

    Drive::init(&device, name.unwrap())?;

    println!("\n{}", "Successfully initialized drive.".bold().green());
    Ok(())
}

/// list-workouts command
fn cmd_list_workouts(args: Args) -> Result<(),CliError> {
    let mut drive = Drive::new(args.arg_device.unwrap(), false)?;

    let workouts = drive.workouts()?;

    // TODO: highlight personal bests?

    println!("{}", format!("{:>3} {:16} {:17} {:5} {:9} {:9} {:>3} {:>6} {:>3} {:>3} {:>6}",
        "#", "Date", "Type", "Dist.", "Work Time", "Rest Time", "SPM", "Pace",
        "HR", "W", "kcal/h").bold().green());
    println!("{}", String::from_utf8(vec![b'='; 90]).unwrap().truecolor(0x7f,0x7f,0x7f));

    let last = args.flag_last.unwrap_or(workouts.len()) as usize;
    for (i, workout) in workouts[workouts.len()-last..].iter().enumerate() {
        println!("{:>3} {:16} {:17} {:>5} {:>9} {:>9} {:>3} {:>6} {:>3} {:>3.0} {:>6.0}",
            i + (workouts.len() - last) + 1,
            workout.datetime.format("%Y-%m-%d %H:%M"),
            workout.workout_type.to_string(),
            workout.total_distance,
            workout.work_duration_string(),
            workout.rest_duration_string(),
            workout.spm.map(|s| s.to_string()).unwrap_or_default(),
            workout.pace_string(),
            workout.heart_rate().map(|h| h.to_string()).unwrap_or_default(),
            workout.watts(),
            workout.cal_hr(),
        );
    }

    Ok(())
}

fn select_latest_versions(versions: Vec<FirmwareVersion>, beta: bool) -> Vec<FirmwareVersion> {
    let mut latest: HashMap<String,FirmwareVersion> = HashMap::new();

    for version in versions {
        if !(version.status == "public" || (beta && version.status == "beta")) {
            continue;
        }

        let monitor = version.monitor.to_lowercase();

        if latest.contains_key(&monitor) && version.version < latest[&monitor].version {
            continue;
        }

        latest.insert(monitor, version);
    }

    latest.values().cloned().collect()
}

fn cmd_update_firmware(args: Args) -> Result<(),CliError> {
    let versions = update_firmware_cache()?;

    let mut drive = Drive::new(args.arg_device.unwrap(), true)?;
    let mut firmwares = drive.firmwares()?;
    firmwares.sort();

    if firmwares.len() == 0 {
        println!("\nFirmwares currently stored on drive: none");
    } else {
        println!("\nFirmwares currently stored on drive:");
        for firmware in firmwares {
            println!("    - {}", firmware);
        }
    }

    // filter firmwares, selecting only the most recent versions for each monitor
    let mut to_install: Vec<String> = select_latest_versions(versions, args.flag_beta).iter()
        // only consider pm5 firmwares
        .filter(|v| &v.monitor.to_lowercase()[0..3] == "pm5")
        // skip pm5v3 for now because i'm not sure what's up with that
        .filter(|v| v.monitor.len() < 5 || &v.monitor.to_lowercase()[0..5] != "pm5v3")
        // find the default file for firmware
        .map(|v| v.files.iter().find(|f| f.default))
        .filter(|f| f.is_some())
        .map(|f| f.unwrap().name.clone())
        .collect();
    to_install.sort();

    println!("\nAbout to clear currently stored firmwares and install the following ones:");
    for firmware in &to_install {
        println!("    - {}", firmware);
    }

    if !confirm("\nProceed?".to_string())? {
        println!("Aborted.");
        return Ok(());
    }

    println!("\nClearing firmwares...");
    drive.clear_firmwares()?;
    println!("Writing firmwares...");
    for firmware in &to_install {
        let mut template = "{spinner:.bold.green} ".to_string();
        template += &format!("{:47}", firmware);
        template += " [{bar:40.bold.green/white}] {bytes}/{total_bytes} ({eta})";

        let pb = indicatif::ProgressBar::new(1);
        pb.set_style(indicatif::ProgressStyle::default_bar()
         .template(&template)
         .progress_chars("##-"));

        let local_path = xdg::BaseDirectories::new()?
            .place_cache_file(Path::new("concept2drive").join("firmware").join(&firmware))?;

        drive.write_firmware_callback(local_path, |written, total| {
            pb.set_position(written as u64);
            pb.set_length(total as u64);
        })?;

        pb.finish();
    }

    Ok(())
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .map(|d| d.version(Some(VERSION.into())))
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let result = if args.cmd_info {
        cmd_info(args)
    } else if args.cmd_init {
        cmd_init(args)
    } else if args.cmd_list_workouts {
        cmd_list_workouts(args)
    } else if args.cmd_update_firmware {
        cmd_update_firmware(args)
    } else {
        Ok(())
    };

    if let Err(e) = result {
        println!("{} {}", "error:".bold().red(), e.msg);
        std::process::exit(1);
    }
}
