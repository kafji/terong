use anyhow::{Context, Error as Anyhow};
use rand::{thread_rng, Rng};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    fs::{read_dir, DirEntry},
    path::PathBuf,
    process::ExitCode,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
};

fn main() -> ExitCode {
    let mut args = std::env::args();

    let name = args.next().unwrap();

    match args.next() {
        None => {
            eprintln!("Select random file.\nUsage: {} <path>", name);
            ExitCode::FAILURE
        }
        Some(path) => match select_file(path) {
            Ok(file_path) => {
                println!("{}", file_path.display());
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("Error: {}", err);
                ExitCode::FAILURE
            }
        },
    }
}

fn select_file(path: String) -> Result<PathBuf, Anyhow> {
    let tree = build_tree(path)?;

    let count = tree.count.load(Ordering::Relaxed);
    println!("Found {} files.", count);

    let mut files = Tree::files(tree);

    let n = thread_rng().gen_range(0..count);
    let file = files.nth(n).unwrap();

    let path = file.lock().unwrap().path.clone();
    Ok(path)
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
    ) -> Result<Option<Arc<Mutex<Self>>>, Anyhow> {
        let ftype = entry.file_type()?;

        if !ftype.is_dir() && !ftype.is_file() {
            return Ok(None);
        }

        let name = entry.file_name().to_string_lossy().into_owned();

        const BLACKLIST: &[&str] = &["$RECYCLE.BIN", "System Volume Information"];
        if BLACKLIST.contains(&name.as_str()) {
            return Ok(None);
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

fn build_tree(root_path: String) -> Result<Arc<Tree>, Anyhow> {
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
        Node::new_child_from_dir_entry(&root, &entry)?;
    }

    let mut leaves = root.lock().unwrap().children.clone();
    while !leaves.is_empty() {
        leaves = leaves
            .into_par_iter()
            .map(|leaf| expand_leaf(leaf))
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

fn expand_leaf(node: Arc<Mutex<Node>>) -> Result<Vec<Arc<Mutex<Node>>>, Anyhow> {
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
        Node::new_child_from_dir_entry(&node, &entry)?;
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
