use crate::hubyte::HuByte;
use anyhow::{anyhow, Context, Error as Anyhow};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    borrow::Borrow,
    collections::VecDeque,
    ffi::OsString,
    fmt::Debug,
    fs,
    marker::PhantomData,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
};

#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;

#[cfg(target_os = "windows")]
use std::os::windows::fs::MetadataExt;

pub trait Key {
    /// Concatenates key. See `Key::concat(&PathBuf, &PathBuf)` for example.
    fn concat(&self, other: &Self) -> Self;

    /// Returns string representation.
    fn into_string(&self) -> String;
}

impl Key for PathBuf {
    fn concat(&self, other: &Self) -> Self {
        self.join(other)
    }

    fn into_string(&self) -> String {
        self.display().to_string()
    }
}

pub trait NodeInfo<K> {
    /// Returns node key.
    fn key(&self) -> Result<K, Anyhow>;

    /// Denotes if node can have children.
    fn has_child(&self) -> Result<bool, Anyhow>;

    /// Denotes if node is irregular.
    ///
    /// Irregular nodes will be filtered out from the tree. See `Node::new_child_from_info`.
    fn irregular(&self) -> Result<bool, Anyhow>;

    /// Returns node size. Used for min size filter. See `Node::new_child_from_info`.
    fn size(&self) -> Result<u64, Anyhow>;
}

impl<K> NodeInfo<K> for fs::DirEntry
where
    K: From<OsString>,
{
    fn key(&self) -> Result<K, Anyhow> {
        Ok(K::from(self.file_name()))
    }

    fn has_child(&self) -> Result<bool, Anyhow> {
        let ftype = self.file_type()?;
        Ok(ftype.is_dir())
    }

    fn irregular(&self) -> Result<bool, Anyhow> {
        let ftype = self.file_type()?;
        Ok(ftype.is_symlink())
    }

    fn size(&self) -> Result<u64, Anyhow> {
        let fmeta = self.metadata()?;

        #[cfg(target_os = "linux")]
        let size = fmeta.size();

        #[cfg(target_os = "windows")]
        let size = fmeta.file_size();

        Ok(size)
    }
}

#[allow(unused)]
#[derive(Debug)]
pub struct Node<K> {
    key: K,
    has_child: bool,
    children: Vec<Arc<Mutex<Self>>>,
    parent: Weak<Mutex<Self>>,
    tree: Weak<Tree<K>>,
}

impl<K: Key> Node<K> {
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Construct new child node.
    ///
    /// Achtung!
    ///
    /// While this function takes parent as its argument, the returned node will not be added into parent's children.
    fn new_child_detached(key: K, has_child: bool, parent: Weak<Mutex<Self>>) -> Self {
        let key: K = parent
            .upgrade()
            .map(|parent| parent.lock().unwrap().key.concat(&key))
            .unwrap_or_else(|| key);

        let tree = parent
            .upgrade()
            .map(|parent| parent.lock().unwrap().tree.clone())
            .unwrap_or_else(|| Weak::new());

        // increment count
        if !has_child {
            tree.upgrade()
                .and_then(|tree| Some(tree.count.fetch_add(1, Ordering::Relaxed)));
        }

        Self {
            key,
            has_child,
            children: Vec::new(),
            parent,
            tree,
        }
    }

    /// Creates new child node and add it into parent's children.
    fn new_child_from_info<T: NodeInfo<K>>(
        parent: &Arc<Mutex<Self>>,
        info: &T,
        min_size: Option<HuByte>,
    ) -> Result<Option<Arc<Mutex<Self>>>, Anyhow> {
        let key = info.key()?;

        // filter out system directories on windows
        const BLACKLIST: &[&str] = &["$RECYCLE.BIN", "System Volume Information"];
        if BLACKLIST.contains(&key.borrow().into_string().as_str()) {
            return Ok(None);
        }

        let irregular = info.irregular()?;
        if irregular {
            return Ok(None);
        }

        let has_child = info.has_child()?;

        if !has_child {
            if let Some(min_size) = min_size {
                let size = info.size()?;
                if size < min_size.to_u64() {
                    return Ok(None);
                }
            }
        }

        let child = Node::new_child_detached(key, has_child, Arc::downgrade(&parent));
        let child = Arc::new(Mutex::new(child));

        parent.lock().unwrap().children.push(child.clone());

        Ok(Some(child))
    }
}

#[derive(Debug)]
pub struct Tree<K> {
    root: Option<Arc<Mutex<Node<K>>>>,
    count: Arc<AtomicUsize>,
}

impl<K> Tree<K> {
    /// Returns how many files were counted.
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    /// Iterates over files in the tree.
    pub fn files<'a>(tree: &'a Tree<K>) -> Files<'a, K> {
        Files::new(tree)
    }
}

#[derive(Debug)]
pub struct TreeBuilder<K, F> {
    read_info: F,
    on_error: fn(Anyhow) -> (),
    _k: PhantomData<K>,
}

impl<K, F, T, U> TreeBuilder<K, F>
where
    K: Key + Clone + Send + Sync,
    F: Fn(&K) -> Result<T, Anyhow> + Sync,
    T: Iterator<Item = Result<U, Anyhow>>,
    U: NodeInfo<K>,
{
    pub fn new(read_info: F, on_error: fn(Anyhow) -> ()) -> Self {
        Self {
            read_info,
            on_error,
            _k: PhantomData,
        }
    }

    pub fn build(&self, root_key: K, min_size: Option<HuByte>) -> Result<Arc<Tree<K>>, Anyhow> {
        let mut tree = Tree {
            root: None,
            count: Arc::new(AtomicUsize::new(0)),
        };

        // assume root has child (is a dir)
        let root = Node::new_child_detached(root_key.clone(), true, Weak::new());
        let root = Arc::new(Mutex::new(root));

        // attach root to tree
        tree.root = Some(root.clone());
        let tree = Arc::new(tree);

        // fix root's tree ref
        root.lock().unwrap().tree = Arc::downgrade(&tree);

        for entry in (self.read_info)(&root_key)
            .with_context(|| format!("failed to read root directory {}", root_key.into_string()))?
        {
            let entry = entry?;
            Node::new_child_from_info(&root, &entry, min_size)?;
        }

        let mut leaves = root.lock().unwrap().children.clone();
        while !leaves.is_empty() {
            leaves = leaves
                .into_par_iter()
                .map(|leaf| self.expand_leaf(leaf, min_size))
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
        &self,
        node: Arc<Mutex<Node<K>>>,
        min_size: Option<HuByte>,
    ) -> Result<Vec<Arc<Mutex<Node<K>>>>, Anyhow> {
        if !node.lock().unwrap().has_child {
            return Ok(Vec::new());
        }

        assert!(node.lock().unwrap().children.is_empty());

        let key = node.lock().unwrap().key().clone();

        let dir = match (self.read_info)(&key) {
            Ok(dir) => dir,
            Err(err) => {
                (self.on_error)(anyhow!("failed to read {}: {}", key.into_string(), err));
                return Ok(Vec::new());
            }
        };

        for entry in dir {
            let entry = entry?;
            Node::new_child_from_info(&node, &entry, min_size)?;
        }

        let children = node.lock().unwrap().children.clone();
        Ok(children)
    }
}

/// Unlike most (if not all) iterators from stdlib, this iterator allow concurrent modification to the data structure.
#[derive(Debug)]
pub struct Files<'a, K> {
    tree: &'a Tree<K>,
    trail: VecDeque<Weak<Mutex<Node<K>>>>,
}

impl<'a, K> Files<'a, K> {
    fn new(tree: &'a Tree<K>) -> Self {
        let mut trail = VecDeque::new();
        if let Some(root) = tree.root.as_ref() {
            trail.push_back(Arc::downgrade(root));
        }
        Self { tree, trail }
    }
}

impl<K> Iterator for Files<'_, K> {
    type Item = Arc<Mutex<Node<K>>>;

    fn next(&mut self) -> Option<Self::Item> {
        let last = match self.trail.pop_front().map(|x| x.upgrade()).flatten() {
            Some(last) => last,
            None => return None,
        };

        if !last.lock().unwrap().has_child {
            return Some(last);
        }

        self.trail
            .extend(last.lock().unwrap().children.iter().map(Arc::downgrade));

        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.tree.count()))
    }
}

#[cfg(test)]
mod tests {
    use super::{anyhow, Anyhow, Key, NodeInfo, Tree, TreeBuilder};
    use serde::{Deserialize, Serialize};
    use std::{collections::VecDeque, fs::read_dir, path::PathBuf};

    impl Key for usize {
        fn concat(&self, other: &Self) -> Self {
            *other
        }

        fn into_string(&self) -> String {
            self.to_string()
        }
    }

    #[derive(Deserialize, Serialize, Clone, Debug)]
    struct TableEntry {
        key: usize,
        has_child: bool,
        irregular: bool,
        size: u64,
        children: Vec<usize>,
    }

    impl NodeInfo<usize> for TableEntry {
        fn key(&self) -> Result<usize, Anyhow> {
            Ok(self.key)
        }

        fn has_child(&self) -> Result<bool, Anyhow> {
            Ok(self.has_child)
        }

        fn irregular(&self) -> Result<bool, Anyhow> {
            Ok(self.irregular)
        }

        fn size(&self) -> Result<u64, Anyhow> {
            Ok(self.size)
        }
    }

    fn read_table<'a>(
        index: usize,
        table: &'a [TableEntry],
    ) -> Result<impl Iterator<Item = Result<TableEntry, Anyhow>> + use<'a>, Anyhow> {
        let entries = table.get(index).ok_or_else(|| anyhow!("not found"))?;
        let iter = entries.children.iter().map(|i| Ok(table[*i].clone()));
        Ok(iter)
    }

    /// Helper function for generating JSON serialized `Vec<TableEntry>`, see `./testadata/table.json`.
    #[allow(unused)]
    fn gen_table_json() -> String {
        let mut table = Vec::new();
        table.push(TableEntry {
            key: 0,
            has_child: true,
            irregular: false,
            size: 0,
            children: Vec::new(),
        });

        let root_dir = PathBuf::from(format!("{}\\src\\", env!("CARGO_MANIFEST_DIR")));

        let mut paths = VecDeque::new();
        paths.push_back((0, root_dir));

        while let Some(path) = paths.pop_front() {
            let mut fsdir =
                read_dir(&path.1).expect(&format!("failed to read {}", path.1.display()));

            while let Some(fsent) = fsdir.next() {
                let fsent = fsent.unwrap();

                let i = table.len();

                let has_child = NodeInfo::<PathBuf>::has_child(&fsent).unwrap();

                let irregular = NodeInfo::<PathBuf>::irregular(&fsent).unwrap();

                let mut size = 0;
                if !has_child && !irregular {
                    size = NodeInfo::<PathBuf>::size(&fsent).unwrap();
                };

                let tent = TableEntry {
                    key: i,
                    has_child,
                    irregular,
                    size,
                    children: Vec::new(),
                };

                table.push(tent);
                assert_eq!(table[i].key, i);

                if has_child {
                    let path = path.1.join(NodeInfo::<PathBuf>::key(&fsent).unwrap());
                    paths.push_back((i, path));
                }

                table[path.0].children.push(i);
            }
        }

        serde_json::to_string_pretty(&table).unwrap()
    }

    #[test]
    fn test_build_tree() {
        let table: Vec<_> = serde_json::from_slice(include_bytes!("testdata/table.json")).unwrap();

        let tree = TreeBuilder::new(|index| read_table(*index, &table), |_| {})
            .build(0, None)
            .unwrap();
        assert_eq!(27, tree.count());
        assert_eq!(
            Some(3),
            Tree::files(&tree).next().map(|x| x.lock().unwrap().key)
        );
        assert_eq!(
            Some(34),
            Tree::files(&tree).last().map(|x| x.lock().unwrap().key)
        );
    }
}
