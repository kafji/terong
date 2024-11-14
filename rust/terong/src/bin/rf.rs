use anyhow::{Context, Error as Anyhow};
use rand::{thread_rng, Rng};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    fs::{read_dir, DirEntry},
    path::PathBuf,
    process::ExitCode,
    str::FromStr,
    sync::{Arc, Mutex},
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

    let count = tree.lock().unwrap().count;
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
    children: Vec<Arc<Mutex<Node>>>,
    parent: Option<Arc<Mutex<Node>>>,
    tree: Arc<Mutex<Tree>>,
    path: PathBuf,
}

impl Node {
    fn new(
        name: String,
        dir: bool,
        parent: Option<Arc<Mutex<Node>>>,
        tree: Arc<Mutex<Tree>>,
    ) -> Self {
        let path = if let Some(parent) = &parent {
            parent.lock().unwrap().path.clone().join(&name)
        } else {
            PathBuf::from_str(&name).unwrap()
        };
        Self {
            name,
            dir,
            children: Vec::new(),
            parent,
            tree,
            path,
        }
    }

    fn add_child(&mut self, child: Arc<Mutex<Node>>) {
        if !child.lock().unwrap().dir {
            self.tree.lock().unwrap().count += 1;
        }
        self.children.push(child);
    }

    fn from_dir_entry(
        entry: &DirEntry,
        parent: Arc<Mutex<Node>>,
    ) -> Result<Option<Arc<Mutex<Self>>>, Anyhow> {
        let ftype = entry.file_type()?;

        if !ftype.is_dir() && !ftype.is_file() {
            return Ok(None);
        }

        let name = entry.file_name().into_string().unwrap();

        const BLACKLIST: &[&str] = &["$RECYCLE.BIN", "System Volume Information"];
        if BLACKLIST.contains(&name.as_str()) {
            return Ok(None);
        }

        let tree = parent.lock().unwrap().tree.clone();

        let node = Node::new(name, ftype.is_dir(), Some(parent.clone()), tree);
        let node = Arc::new(Mutex::new(node));
        Ok(Some(node))
    }
}

#[derive(Default, Debug)]
struct Tree {
    root: Option<Arc<Mutex<Node>>>,
    count: usize,
}

impl Tree {
    fn files(tree: Arc<Mutex<Self>>) -> Files {
        Files::new(tree)
    }
}

fn build_tree(root_path: String) -> Result<Arc<Mutex<Tree>>, Anyhow> {
    let tree = Tree::default();
    let tree = Arc::new(Mutex::new(tree));

    let root = Node::new(root_path.clone(), true, None, tree.clone());
    let root = Arc::new(Mutex::new(root));
    tree.lock().unwrap().root = Some(root.clone());

    for entry in
        read_dir(&root_path).with_context(|| format!("failed to read directory {}", root_path))?
    {
        let entry = entry?;
        if let Some(child) = Node::from_dir_entry(&entry, root.clone())? {
            root.lock().unwrap().add_child(child);
        }
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
        if let Some(child) = Node::from_dir_entry(&entry, node.clone())? {
            node.lock().unwrap().add_child(child);
        }
    }

    Ok(node.lock().unwrap().children.clone())
}

#[derive(Debug)]
struct Files {
    trail: Vec<Arc<Mutex<Node>>>,
}

impl Files {
    fn new(tree: Arc<Mutex<Tree>>) -> Self {
        let mut trail = Vec::new();
        if let Some(root) = tree.lock().unwrap().root.as_ref() {
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
