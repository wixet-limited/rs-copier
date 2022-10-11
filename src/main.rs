use std::vec;
use std::path::PathBuf;
use anyhow::Result;
use tokio::task::JoinSet;
use log::{info, debug, error};
use std::path::Path;
use clap::Parser;


async fn process_directory(source: &PathBuf, dest: &PathBuf, remove_source: bool) -> Result<Vec<PathBuf>> {
    info!("Processing: {:?}", source);
    let mut paths = tokio::fs::read_dir(&source).await?;
    tokio::fs::create_dir_all(&dest).await?;
    let mut directories = vec![];
    let mut remove_dir = true;
    while let Some(path) = paths.next_entry().await? {
        match path.file_type().await {
            Ok(file_type) => {
                if file_type.is_file() {
                    // Move file
                    let from = path.path();
                    let to = dest.join(path.file_name());
                    debug!("Copy: {:?} to {:?}", from, to);
                    if let Err(error) = tokio::fs::copy(&from, &to).await {
                        error!("Cannot copy file: {:?}: {:?}", from, error);
                    } else {
                        if remove_source {
                            if let Err(error) = tokio::fs::remove_file(&from).await {
                                error!("Cannot remove file: {:?}: {:?}", from, error);
                                remove_dir = false;
                            }
                        }
                    }
                } else {
                    directories.push(path.path());
                }
            } ,
            Err(error) => { 
                error!("Cannot get file type: {:?}", error);
            }
        }  
    }
    if remove_source && directories.is_empty() && remove_dir {
        if let Err(error) = tokio::fs::remove_dir(&source).await {
            error!("Cannot remove directory: {:?}: {:?}", source, error);
        }
    }
    Ok(directories)
}

fn setup_logger(loglevel: &str, logfile: Option<&str>) -> Result<()>{   
    let level = match loglevel {
        "INFO" => log::LevelFilter::Info,
        "DEBUG" => log::LevelFilter::Debug,
        "WARN" => log::LevelFilter::Warn,
        "ERROR" => log::LevelFilter::Error,
        &_ => log::LevelFilter::Info
    };
    let mut f = fern::Dispatch::new()
    // Perform allocation-free log formatting
    .format(|out, message, record| {
        out.finish(format_args!(
            "{}[{}][{}] {}",
            chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
            record.target(),
            record.level(),
            message
        ))
    })
    // Add blanket level filter -
    .level(level)
    .chain(std::io::stdout());

    if let Some(logfile) = logfile {
        f = f.chain(fern::log_file(logfile)?);
    }

    f.apply()?;
    Ok(())

}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
   /// Name of the person to greet
   #[clap(short, long, value_parser)]
   source: String,
   /// Name of the person to greet
   #[clap(short, long, value_parser)]
   destination: String,
}


#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    setup_logger("INFO", None::<&str>)?;

    let source = args.source;
    let dest = args.destination;
    let batch_size = 50;
    //let tasks = vec![];
    let mut set = JoinSet::new();
    let dirs = process_directory(&(&source).into(), &(&dest).into(), false).await?;
    for dir in dirs {
        
        let dir_dest = Path::new(&dest).join(&dir.strip_prefix(&source)?);
        let subdirs = process_directory(&dir, &dir_dest, false).await.unwrap();
        for subdir in subdirs {
            let dir_dest = Path::new(&dest).join(&dir.strip_prefix(&source)?);
            let sub_source = source.to_owned();
            let sub_dest = dir_dest.to_owned();
            set.spawn(async move {
                let d = sub_dest.join(subdir.strip_prefix(&sub_source).unwrap());
                process_directory(&subdir, &d, true).await.unwrap();
                d
            });
            if set.len() >= batch_size {
                // Max concurrency
                if let Some(res) = set.join_next().await {
                    match res {
                        Ok(dir) => {
                            info!("Done {:?}", dir);
                        },
                        Err(err) => {
                            error!("Error {:?}", err);
                        }
                    }
                }
            }

        }
        
            
        
        
    }
    //Pending tasks
    while let Some(res) = set.join_next().await {
        match res {
            Ok(dir) => {
                info!("Done {:?}", dir);
            },
            Err(err) => {
                error!("Error {:?}", err);
            }
        }
    }

    info!("All done");

    Ok(())
}
