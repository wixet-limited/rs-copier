//! Only two parameteres are required: source and destination. Apart from that, you can specify if you want to remove the source (move the files) and the concurrency level. For example:
//!
//!```
//!rs-copier --source data_origin --destination data_destination --delete-source true --concurrency 20
//!```
//!
//!But you can always run with `--help` to get more details

use std::vec;
use std::path::PathBuf;
use anyhow::Result;
use tokio::task::JoinSet;
use log::{info, debug, error};
use std::path::Path;
use clap::Parser;

/// Copy all the files of the directory from source to dest. Remove the source files if remove_source = true
/// Then list all the directories and return them.
async fn process_directory(source: &Path, dest: &Path, remove_source: bool) -> Result<Vec<PathBuf>> {
    info!("Processing dir: {:?}", source);
    let mut paths = tokio::fs::read_dir(&source).await?;
    tokio::fs::create_dir_all(&dest).await?;
    let mut directories = vec![];
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
    Ok(directories)
}

/// Logger configuration
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

/// Arguments parser
/// `--source` the source directory
/// `--destination` the destination directory
/// `--delete-source` to act like moving (first copy and the remove the source file)
/// `--concurrency` to set the maximum concurrency
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
   /// Name of the person to greet
   #[clap(short, long, value_parser)]
   source: String,
   /// Name of the person to greet
   #[clap(short, long, value_parser)]
   destination: String,
   /// Delete source or not
   #[clap(long, value_parser, default_value = "false")]
   delete_source: bool,
   /// Concurrency
   #[clap(long, value_parser, default_value = "10")]
   concurrency: usize,
}


#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    setup_logger("INFO", None::<&str>)?;

    let base_source = PathBuf::from(args.source);
    let base_dest = PathBuf::from(args.destination);
    let delete_source = args.delete_source;
    let batch_size = args.concurrency;
    if delete_source {
        info!("Source files will be deleted once copied");
    }
    
    if !base_source.exists() {
        return Err(anyhow::anyhow!("Source directory does not exist"));
    }

    info!("The concurrency is set to {batch_size}");

    let mut set = JoinSet::new();
    let mut dirs = process_directory(&base_source.clone(), &base_dest.clone(), delete_source).await?;
    
    while let Some(dir) = dirs.pop() {
        let dest = base_dest.join(dir.strip_prefix(&base_source).unwrap());
        set.spawn(async move {            
            process_directory(&dir, &dest, delete_source).await.unwrap()
        });

        if set.len() >= batch_size {
            // Max concurrency
            if let Some(res) = set.join_next().await {
                match res {
                    Ok(mut new_dirs) => {
                        dirs.append(&mut new_dirs);
                    },
                    Err(err) => {
                        error!("Error {:?}", err);
                    }
                }
            }
        }
    }

    // Remove source (which is only the directory structure empty of files)
    if delete_source {
        tokio::fs::remove_dir_all(base_source).await?;
    }
    info!("All done");

    Ok(())
}


#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use super::process_directory;

    const BASE_DIR: &str = "/tmp/test";

    async fn init(name: &str) -> PathBuf {
        let base_dir = PathBuf::from(BASE_DIR).join(name);
        
        if base_dir.exists() {
            tokio::fs::remove_dir_all(&base_dir).await.unwrap();
        }
        tokio::fs::create_dir_all(&base_dir).await.unwrap();
        
        base_dir
    }

    #[tokio::test]
    async fn empty_directory() {
        let base_dir = init("empty_directory").await;
        
        let source = base_dir.join("source");
        let dest = base_dir.join("dest");
        assert_eq!(dest.exists(), false);
        tokio::fs::create_dir_all(&source).await.unwrap();
        process_directory(&source, &dest, false).await.unwrap();

        assert_eq!(source.exists(), true);
        assert_eq!(dest.exists(), true);
    }

    #[tokio::test]
    async fn only_files() {
        let base_dir = init("only_files").await;

        let source = base_dir.join("source");
        tokio::fs::create_dir_all(&source).await.unwrap();
        let dest = base_dir.join("dest");
        tokio::fs::write(source.join("file1"), "text").await.unwrap();
        tokio::fs::write(source.join("file2"), "text").await.unwrap();
        assert_eq!(dest.exists(), false);
        process_directory(&source, &dest, false).await.unwrap();

        assert_eq!(source.join("file1").exists(), true);
        assert_eq!(source.join("file2").exists(), true);
        
        assert_eq!(dest.exists(), true);
        assert_eq!(dest.join("file1").exists(), true);
        assert_eq!(dest.join("file2").exists(), true);
    }

    #[tokio::test]
    async fn only_files_delete() {
        let base_dir = init("only_files_delete").await;

        let source = base_dir.join("source");
        tokio::fs::create_dir_all(&source).await.unwrap();
        tokio::fs::write(source.join("file1"), "text").await.unwrap();
        tokio::fs::write(source.join("file2"), "text").await.unwrap();
        let dest = base_dir.join("dest");
        assert_eq!(dest.exists(), false);
        let res = process_directory(&source, &dest, true).await.unwrap();

        assert_eq!(source.join("file1").exists(), false);
        assert_eq!(source.join("file2").exists(), false);
        
        assert_eq!(dest.exists(), true);
        assert_eq!(dest.join("file1").exists(), true);
        assert_eq!(dest.join("file2").exists(), true);
        assert_eq!(res.len(), 0);
    }


    #[tokio::test]
    async fn nested() {
        let base_dir = init("nested").await;

        let source = base_dir.join("source");
        tokio::fs::create_dir_all(&source).await.unwrap();
        let dest = base_dir.join("dest");
        tokio::fs::write(source.join("file1"), "text").await.unwrap();
        tokio::fs::write(source.join("file2"), "text").await.unwrap();
        let nested = source.join("nested");
        tokio::fs::create_dir_all(source.join("nested")).await.unwrap();
        tokio::fs::write(nested.join("file3"), "text").await.unwrap();
        tokio::fs::write(nested.join("file4"), "text").await.unwrap();

        assert_eq!(dest.exists(), false);
        let res = process_directory(&source, &dest, false).await.unwrap();

        assert_eq!(source.join("file1").exists(), true);
        assert_eq!(source.join("file2").exists(), true);
        
        
        
        assert_eq!(dest.exists(), true);
        assert_eq!(dest.join("file1").exists(), true);
        assert_eq!(dest.join("file2").exists(), true);
        
        assert_eq!(res.len(), 1);
        assert_eq!(res[0], base_dir.join("source").join("nested"));

        let nested_dest = base_dir.join("dest").join("nested");
        
        let res = process_directory(&res[0], &nested_dest, false).await.unwrap();
        
        assert_eq!(res.len(), 0);
        
        
        assert_eq!(nested_dest.exists(), true);
        assert_eq!(nested_dest.join("file3").exists(), true);
        assert_eq!(nested_dest.join("file4").exists(), true);
    }

    
}