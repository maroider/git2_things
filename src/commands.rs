use std::path::PathBuf;

use git2::{BranchType, ErrorCode, Oid, ReferenceType, Repository, TreeEntry};

// macro_rules! unwrap_result_or_else {
//     ($result:expr, $err:ident, $else_:expr) => {
//         match $result {
//             Ok(ok) => ok,
//             Err($err) => $else_,
//         }
//     };
// }

pub fn glcm(path: PathBuf) {
    if let Some(repository) = open_current_dir_as_repository() {
        let master_branch = repository
            .find_branch("master", BranchType::Local)
            .expect("Could not find branch 'master'");
        let reference = master_branch.get();
        assert_eq!(reference.kind(), Some(ReferenceType::Direct));
        let branch_tip_commit_id = reference.target().unwrap();
        let branch_tip_commit = repository.find_commit(branch_tip_commit_id).unwrap();
        let branch_tip_tree = branch_tip_commit.tree().unwrap();
        let tree = dbg!(Tree::new(&repository, &branch_tip_tree));
        branch_tip_commit.parent_ids();
    }
}

// TODO: !
struct DisplayTree {
    items: Vec<DisplayTreeItem>,
}

// TODO: !
struct DisplayTreeItem {
    path: PathBuf,
    name: String,
    last_commit_id: Oid,
    last_commit_message: String,
}

// TODO: !
struct DiffTree {
    changed: Tree,
    unchanged: Tree,
}

// TODO: Investigate how much should be borrowed
#[derive(Clone, Debug)]
struct Tree {
    items: Vec<TreeItem>,
}

impl Tree {
    fn new(repository: &Repository, tree: &git2::Tree) -> Self {
        let items = {
            let tree_iter = tree.iter();
            let size_hint = {
                let (lower, upper) = tree_iter.size_hint();
                upper.unwrap_or(lower)
            };
            let mut items = Vec::with_capacity(size_hint);
            for entry in tree_iter {
                let filemode = FileMode::from_i32(entry.filemode()).unwrap();
                if filemode.is_dir() {
                    let inner_tree = Tree::new(
                        repository,
                        &entry.to_object(repository).unwrap().into_tree().unwrap(),
                    );
                    items.push(TreeItem::Directory(entry.to_owned(), inner_tree));
                } else if filemode.is_file() {
                    items.push(TreeItem::File(entry.to_owned()));
                } else if filemode.is_symlink() {
                    items.push(TreeItem::Symlink(entry.to_owned()));
                } else if filemode.is_gitlink() {
                    items.push(TreeItem::Gitlink(entry.to_owned()));
                } else {
                    unreachable!()
                }
            }
            items
        };
        Self { items }
    }

    /// Returns `(changed, unchanged)`
    fn diff(&self, other: &Tree) -> DiffTree {
        let mut unchanged = self.items.clone();
        let mut changed = Vec::with_capacity(unchanged.len());

        let mut i = 0;
        while i != unchanged.len() {
            let item = &mut unchanged[i];
            let other_item = other.items.iter().find(|other_item| {
                other_item.entry().name_bytes() == item.entry().name_bytes()
                    && other_item.entry().filemode() == item.entry().filemode()
            });

            if !item.filemode().is_dir() {
                let is_same_object = other_item
                    .map(|other_item| other_item.entry().id() == item.entry().id())
                    .unwrap_or(false);
                if !is_same_object {
                    changed.push(unchanged.remove(i));
                } else {
                    i += 1;
                }
            } else {
                // TODO: !

                // item.diff()
            }
        }

        unimplemented!()
    }
}

#[derive(Clone)]
enum TreeItem {
    Directory(TreeEntry<'static>, Tree),
    File(TreeEntry<'static>),
    Symlink(TreeEntry<'static>),
    Gitlink(TreeEntry<'static>),
}

impl TreeItem {
    pub fn filemode(&self) -> FileMode {
        match self {
            Self::Directory(_, _) => FileMode::Directory,
            Self::File(entry) => FileMode::from_i32(entry.filemode()).unwrap(),
            Self::Symlink(_) => FileMode::Symlink,
            Self::Gitlink(_) => FileMode::Gitlink,
        }
    }

    pub fn entry(&self) -> &TreeEntry<'static> {
        match self {
            Self::Directory(entry, _) => entry,
            Self::File(entry) => entry,
            Self::Symlink(entry) => entry,
            Self::Gitlink(entry) => entry,
        }
    }
}

impl core::fmt::Debug for TreeItem {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::Directory(entry, tree) => f
                .debug_tuple("Directory")
                .field(&entry.name())
                .field(tree)
                .finish(),
            Self::File(entry) => f.debug_tuple("File").field(&entry.name()).finish(),
            Self::Symlink(entry) => f.debug_tuple("Symlink").field(&entry.name()).finish(),
            Self::Gitlink(entry) => f.debug_tuple("Gitlink").field(&entry.name()).finish(),
        }
    }
}

/// https://stackoverflow.com/a/8347325
#[allow(clippy::unreadable_literal)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileMode {
    Directory = 0o040000,
    File = 0o100644,
    GroupWriteableFile = 0o100664,
    Executable = 0o100755,
    Symlink = 0o120000,
    Gitlink = 0o160000,
}

impl FileMode {
    pub fn is(self, other: i32) -> bool {
        self as i32 == other
    }

    pub fn from_i32(from: i32) -> Option<Self> {
        if Self::Directory.is(from) {
            Some(Self::Directory)
        } else if Self::File.is(from) {
            Some(Self::File)
        } else if Self::GroupWriteableFile.is(from) {
            Some(Self::GroupWriteableFile)
        } else if Self::Executable.is(from) {
            Some(Self::Executable)
        } else if Self::Symlink.is(from) {
            Some(Self::Symlink)
        } else if Self::Gitlink.is(from) {
            Some(Self::Gitlink)
        } else {
            None
        }
    }

    pub fn is_file(self) -> bool {
        match self {
            Self::File | Self::GroupWriteableFile | Self::Executable => true,
            _ => false,
        }
    }

    pub fn is_dir(self) -> bool {
        Self::Directory == self
    }

    pub fn is_symlink(self) -> bool {
        Self::Symlink == self
    }

    pub fn is_gitlink(self) -> bool {
        Self::Gitlink == self
    }
}

fn open_current_dir_as_repository() -> Option<Repository> {
    Repository::open(std::env::current_dir().unwrap())
        .map(Some)
        .unwrap_or_else(|err| {
            if err.code() == ErrorCode::NotFound {
                println!("Current directory is not a git repository");
            } else {
                println!("Error! libgit2: {}", err);
            }
            None
        })
}
