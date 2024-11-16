use anyhow::{anyhow, Error as Anyhow};
use rand::{distributions::Uniform, thread_rng, Rng};
use std::{collections::HashSet, fs::read_dir, path::PathBuf, process::ExitCode};
use terong::{
    hubyte::HuByte,
    rf::{Key, Tree, TreeBuilder},
};

fn main() -> ExitCode {
    let mut args = std::env::args();

    let name = args.next().unwrap();

    match (args.next(), args.next(), args.next()) {
        (None, ..) => {
            eprintln!(
                "Select random file.\nUsage: {} <path> [min-size=0k [count=1]]",
                name
            );
            ExitCode::FAILURE
        }
        (Some(path), min_size, count) => {
            match min_size
                .map(|x| {
                    x.parse()
                        .map_err(|_| anyhow!("unexpected min-size: expecting <number><unit> where unit is one of k, m, or g"))
                })
                .transpose()
                .and_then(|min_size| {
                    count
                        .map(|x| {
                            x.parse()
                                .map_err(|err| anyhow!("unexpected count: {}", err))
                        })
                        .transpose()
                        .and_then(|x| match x {
                            Some(x) if x < 1 => Err(anyhow!("expecting count > 0")),
                            _ => Ok(x),
                        })
                        .map(|count| (min_size, count))
                })
                .and_then(|(min_size, count)| select_file(path, min_size, count.unwrap_or(1)))
            {
                Ok(_) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("Error: {}", err);
                    ExitCode::FAILURE
                }
            }
        }
    }
}

fn select_file(path: String, min_size: Option<HuByte>, count: usize) -> Result<(), Anyhow> {
    assert!(count > 0);

    if let Some(min_size) = min_size.as_ref() {
        println!("Min size: {}.", min_size);
    }

    let tree = TreeBuilder::new(
        |path| {
            read_dir(path)
                .map(|iter| iter.map(|dirent| dirent.map_err(Anyhow::new)))
                .map_err(Anyhow::new)
        },
        |err| eprintln!("Error: {}", err),
    )
    .build(PathBuf::from(path), min_size)?;

    let files_count = tree.count();
    println!("Found {} files.", files_count);

    if files_count == 0 {
        return Ok(());
    }

    let ns = if files_count <= count {
        HashSet::from_iter(0..files_count)
    } else {
        let mut ns = HashSet::new();
        let dist = Uniform::new(0, files_count);
        let mut rng = thread_rng();
        while ns.len() < count {
            let n = rng.sample(dist);
            ns.insert(n);
        }
        ns
    };

    let files = Tree::files(&tree)
        .enumerate()
        .filter(|(i, _)| ns.contains(i))
        .take(ns.len())
        .map(|(_, x)| x);

    for file in files {
        let file = file.lock().unwrap();
        let path = file.key();
        println!("{}", path.into_string());
    }

    Ok(())
}
