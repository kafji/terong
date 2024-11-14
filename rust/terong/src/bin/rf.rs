use anyhow::{anyhow, Context, Error as Anyhow};
use rand::{distributions::Uniform, thread_rng, Rng};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    collections::HashSet,
    fmt::{Display, Formatter, Result as FmtResult},
    fs::{read_dir, DirEntry},
    path::PathBuf,
    process::ExitCode,
    str::FromStr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
};

fn main() -> ExitCode {
    let mut args = std::env::args();

    let name = args.next().unwrap();

    match (args.next(), args.next(), args.next()) {
        (None, ..) => {
            eprintln!(
                "Select random file.\nUsage: {} <path> [min-size [count]]",
                name
            );
            ExitCode::FAILURE
        }
        (Some(path), min_size, count) => {
            // if only do operator exists...
            match min_size
                .map(|x| {
                    x.parse()
                        .map_err(|err| anyhow!("unexpected min size: {}", err))
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

    let tree = build_tree(path, min_size)?;

    let files_count = tree.count.load(Ordering::Relaxed);
    println!("Found {} files.", files_count);

    if files_count == 0 {
        return Ok(());
    }

    let ns = if files_count <= count {
        HashSet::from_iter(0..files_count)
    } else {
        let mut ns = HashSet::new();
        let dist = Uniform::new(0, files_count);
        while ns.len() < count {
            let n = thread_rng().sample(dist);
            ns.insert(n);
        }
        ns
    };

    let files = Tree::files(tree)
        .enumerate()
        .filter(|(i, _)| ns.contains(i))
        .take(ns.len())
        .map(|(_, x)| x);

    for file in files {
        let path = &file.lock().unwrap().path;
        println!("{}", path.display());
    }

    Ok(())
}

#[allow(unused)]
#[derive(Debug)]
struct Node {
    name: String,
    dir: bool,
    path: PathBuf,
    children: Vec<Arc<Mutex<Node>>>,
    parent: Weak<Mutex<Node>>,
    tree: Weak<Tree>,
}

impl Node {
    fn new_child(name: String, dir: bool, parent: Weak<Mutex<Node>>, tree: Weak<Tree>) -> Self {
        // this expression does two things:
        // - increment count if child is a file, and
        // - construct path for child's node struct
        //
        // ideally we split this into two separate expressions but that require upgrading parent weakref twice
        let path = if let Some(parent) = parent.upgrade() {
            if !dir {
                // increment count
                parent
                    .lock()
                    .unwrap()
                    .tree
                    .upgrade()
                    .map(|x| x.count.fetch_add(1, Ordering::Relaxed));
            }

            // construct path
            parent.lock().unwrap().path.clone().join(&name)
        } else {
            // construct path if node is actually a root node
            PathBuf::from(&name)
        };

        Self {
            name,
            dir,
            path,
            children: Vec::new(),
            parent,
            tree,
        }
    }

    fn new_child_from_dir_entry(
        parent: &Arc<Mutex<Node>>,
        entry: &DirEntry,
        min_size: Option<HuByte>,
    ) -> Result<Option<Arc<Mutex<Self>>>, Anyhow> {
        let name = entry.file_name().to_string_lossy().into_owned();

        const BLACKLIST: &[&str] = &["$RECYCLE.BIN", "System Volume Information"];
        if BLACKLIST.contains(&name.as_str()) {
            return Ok(None);
        }

        let ftype = entry.file_type()?;

        if !ftype.is_dir() && !ftype.is_file() {
            return Ok(None);
        }

        if ftype.is_file() {
            if let Some(min_size) = min_size {
                let fmeta = entry.metadata()?;
                if fmeta.len() < min_size.to_u64() {
                    return Ok(None);
                }
            }
        }

        let tree = parent.lock().unwrap().tree.clone();

        let child = Node::new_child(name, ftype.is_dir(), Arc::downgrade(&parent), tree);
        let child = Arc::new(Mutex::new(child));

        parent.lock().unwrap().children.push(child.clone());

        Ok(Some(child))
    }
}

#[derive(Debug)]
struct Tree {
    root: Option<Arc<Mutex<Node>>>,
    count: Arc<AtomicUsize>,
}

impl Tree {
    fn files(tree: impl AsRef<Tree>) -> Files {
        Files::new(tree)
    }
}

fn build_tree(root_path: String, min_size: Option<HuByte>) -> Result<Arc<Tree>, Anyhow> {
    let mut tree = Tree {
        root: None,
        count: Arc::new(AtomicUsize::new(0)),
    };

    let root = Node::new_child(root_path.clone(), true, Weak::new(), Weak::new());
    let root = Arc::new(Mutex::new(root));

    tree.root = Some(root.clone());
    let tree = Arc::new(tree);

    root.lock().unwrap().tree = Arc::downgrade(&tree);

    for entry in read_dir(&root_path)
        .with_context(|| format!("failed to read root directory {}", root_path))?
    {
        let entry = entry?;
        Node::new_child_from_dir_entry(&root, &entry, min_size)?;
    }

    let mut leaves = root.lock().unwrap().children.clone();
    while !leaves.is_empty() {
        leaves = leaves
            .into_par_iter()
            .map(|leaf| expand_leaf(leaf, min_size))
            .reduce(
                || Ok(Vec::new()),
                |acc, more| match (acc, more) {
                    (Ok(mut acc), Ok(more)) => {
                        acc.extend(more);
                        Ok(acc)
                    }
                    (Err(err), _) | (_, Err(err)) => Err(err),
                },
            )?;
    }

    Ok(tree)
}

fn expand_leaf(
    node: Arc<Mutex<Node>>,
    min_size: Option<HuByte>,
) -> Result<Vec<Arc<Mutex<Node>>>, Anyhow> {
    if !node.lock().unwrap().dir {
        return Ok(Vec::new());
    }

    assert!(node.lock().unwrap().children.is_empty());

    let path = node.lock().unwrap().path.clone();

    let dir = match read_dir(&path) {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("Error: Failed to read {}: {}", path.display(), err);
            return Ok(Vec::new());
        }
    };

    for entry in dir {
        let entry = entry?;
        Node::new_child_from_dir_entry(&node, &entry, min_size)?;
    }

    let children = node.lock().unwrap().children.clone();
    Ok(children)
}

#[derive(Debug)]
struct Files {
    trail: Vec<Arc<Mutex<Node>>>,
}

impl Files {
    fn new(tree: impl AsRef<Tree>) -> Self {
        let mut trail = Vec::new();
        if let Some(root) = tree.as_ref().root.as_ref() {
            trail.push(root.clone());
        }
        Self { trail }
    }
}

impl Iterator for Files {
    type Item = Arc<Mutex<Node>>;

    fn next(&mut self) -> Option<Self::Item> {
        let last = match self.trail.pop() {
            Some(last) => last,
            None => return None,
        };

        if !last.lock().unwrap().dir {
            return Some(last);
        }

        self.trail.extend(last.lock().unwrap().children.clone());

        self.next()
    }
}

#[derive(Copy, Clone, Debug)]
enum HuByteUnit {
    KB,
    MB,
    GB,
}

impl FromStr for HuByteUnit {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "k" => Ok(Self::KB),
            "m" => Ok(Self::MB),
            "g" => Ok(Self::GB),
            _ => Err(()),
        }
    }
}

impl Display for HuByteUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(match self {
            HuByteUnit::KB => "KB",
            HuByteUnit::MB => "MB",
            HuByteUnit::GB => "GB",
        })
    }
}

#[derive(Copy, Clone, Debug)]
struct HuByte {
    val: u64,
    unit: HuByteUnit,
}

impl HuByte {
    fn to_u64(&self) -> u64 {
        self.val
            * match self.unit {
                HuByteUnit::KB => 1024,
                HuByteUnit::MB => 1024 * 1024,
                HuByteUnit::GB => 1024 * 1024 * 1024,
            }
    }
}

impl FromStr for HuByte {
    type Err = Anyhow;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || anyhow!("expecting <number><unit> where unit is one of k, m, or g");

        let mut chars = s.chars();

        let mut digits = String::new();
        let mut unit = String::new();
        while let Some(c) = chars.next() {
            if !c.is_ascii_digit() {
                unit.push(c);
                break;
            }
            digits.push(c);
        }
        unit.extend(chars);

        let digits = digits.parse().map_err(|_| err())?;

        let unit = unit.parse().map_err(|_| err())?;

        Ok(Self { val: digits, unit })
    }
}

impl Display for HuByte {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!(
            "{} {} ({})",
            self.val,
            self.unit,
            self.to_u64()
        ))
    }
}
