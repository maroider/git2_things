use std::{
    collections::HashMap,
    path::{self, Path, PathBuf},
};

use git2::{BranchType, ErrorCode, ObjectType, Oid, ReferenceType, Repository, TreeEntry};

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
        let display_tree = DisplayTree::new(&path, &repository, branch_tip_commit_id);
        println!("{}", path.display());
        for item in display_tree.items.iter() {
            println!(
                "    {:13}{:20}{:44}{}",
                format!("{:?}", item.filemode),
                item.name,
                item.last_commit_id,
                item.last_commit_message.trim()
            );
        }
    }
}

// TODO: !
#[derive(Debug)]
struct DisplayTree {
    items: Vec<DisplayTreeItem>,
}

impl DisplayTree {
    /// # Panics
    ///
    /// This function will panice if `path` points to something other than a directory
    pub fn new<P>(path: P, repository: &Repository, commit_id: Oid) -> Self
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let invalid_prefix = path.components().take(1).find(|component| match component {
            path::Component::Prefix(_)
            | path::Component::RootDir
            | path::Component::CurDir
            | path::Component::ParentDir => true,
            path::Component::Normal(_) => false,
        });
        let normalized_path = invalid_prefix
            .map(|invalid_prefix| path.strip_prefix(invalid_prefix).unwrap())
            .unwrap_or(path);

        let mut revwalk = repository.revwalk().unwrap();
        revwalk.push(commit_id).unwrap();

        let mut oldest_commit_for_object = HashMap::new();

        for older_commit_id in revwalk {
            let older_commit_id = older_commit_id.unwrap();
            let older_commit = repository.find_commit(older_commit_id).unwrap();
            let older_tree = older_commit.tree().unwrap();
            let older_target_tree = {
                if normalized_path == Path::new("") {
                    older_tree
                } else {
                    older_tree
                        .get_path(normalized_path)
                        .unwrap()
                        .to_object(repository)
                        .unwrap()
                        .peel_to_tree()
                        .unwrap()
                }
            };
            for entry in older_target_tree.iter() {
                if entry.kind() == Some(ObjectType::Blob) || entry.kind() == Some(ObjectType::Tree)
                {
                    oldest_commit_for_object.insert(entry.id(), older_commit_id);
                }
            }
        }

        let commit = repository.find_commit(commit_id).unwrap();
        let tree = commit.tree().unwrap();

        let target_tree = {
            if normalized_path == Path::new("") {
                tree
            } else {
                tree.get_path(normalized_path)
                    .unwrap()
                    .to_object(repository)
                    .unwrap()
                    .peel_to_tree()
                    .unwrap()
            }
        };

        let items = target_tree
            .iter()
            .filter_map(|entry| {
                if entry.kind() == Some(ObjectType::Blob) || entry.kind() == Some(ObjectType::Tree)
                {
                    Some(DisplayTreeItem::new(
                        repository,
                        *oldest_commit_for_object.get(&entry.id()).unwrap(),
                        entry,
                    ))
                } else {
                    None
                }
            })
            .collect();

        Self { items }
    }
}

// TODO: !
#[derive(Debug)]
struct DisplayTreeItem {
    name: String,
    last_commit_id: Oid,
    last_commit_message: String,
    filemode: FileMode,
}

impl DisplayTreeItem {
    fn new(repository: &Repository, last_commit_id: Oid, entry: TreeEntry<'_>) -> Self {
        let last_commit = repository.find_commit(last_commit_id).unwrap();
        Self {
            name: entry.name().unwrap().to_string(),
            last_commit_id,
            last_commit_message: last_commit.message().unwrap().to_string(),
            filemode: FileMode::from_i32(entry.filemode()).unwrap(),
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

    #[allow(dead_code)]
    pub fn is_file(self) -> bool {
        match self {
            Self::File | Self::GroupWriteableFile | Self::Executable => true,
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub fn is_dir(self) -> bool {
        Self::Directory == self
    }

    #[allow(dead_code)]
    pub fn is_symlink(self) -> bool {
        Self::Symlink == self
    }

    #[allow(dead_code)]
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
