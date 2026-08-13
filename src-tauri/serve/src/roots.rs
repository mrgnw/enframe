use std::path::{Path, PathBuf};

const DENIED: &str = "path is not inside an allowed root";

#[derive(Debug)]
pub struct Roots {
    read: Vec<PathBuf>,
    write: Vec<PathBuf>,
}

impl Roots {
    // Write roots are implicitly readable: an export lands next to the photo it came from,
    // and the app reads that file straight back.
    pub fn new(read: Vec<PathBuf>, write: Vec<PathBuf>) -> Result<Self, String> {
        let write = canonical_all(write)?;
        let mut read = canonical_all(read)?;
        read.extend(write.iter().cloned());
        read.sort();
        read.dedup();
        if read.is_empty() {
            return Err("no readable roots configured".into());
        }
        Ok(Self { read, write })
    }

    pub fn read_roots(&self) -> &[PathBuf] {
        &self.read
    }

    pub fn write_roots(&self) -> &[PathBuf] {
        &self.write
    }

    pub fn check_read(&self, path: &str) -> Result<PathBuf, String> {
        let resolved = Path::new(path)
            .canonicalize()
            .map_err(|e| reject(path, &format!("cannot resolve: {e}")))?;
        confine(&self.read, resolved, path)
    }

    // The file need not exist yet, so resolve the parent and rejoin the name.
    pub fn check_write(&self, path: &str) -> Result<PathBuf, String> {
        let candidate = Path::new(path);
        let name = candidate
            .file_name()
            .ok_or_else(|| reject(path, "no file name"))?;
        let parent = match candidate.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        let parent = parent
            .canonicalize()
            .map_err(|e| reject(path, &format!("cannot resolve parent: {e}")))?;

        // If the name already exists as a symlink, a write follows it out of the root.
        let resolved = parent.join(name);
        let resolved = resolved.canonicalize().unwrap_or(resolved);
        confine(&self.write, resolved, path)
    }
}

fn canonical_all(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
    paths
        .into_iter()
        .map(|path| {
            path.canonicalize()
                .map_err(|e| format!("root {}: {e}", path.display()))
        })
        .collect()
}

// starts_with compares whole components, so /tmp/rootX is not inside /tmp/root.
fn confine(roots: &[PathBuf], resolved: PathBuf, requested: &str) -> Result<PathBuf, String> {
    if roots.iter().any(|root| resolved.starts_with(root)) {
        return Ok(resolved);
    }
    eprintln!(
        "denied {requested:?}: resolves to {}, outside every root",
        resolved.display()
    );
    Err(DENIED.to_string())
}

// The client only ever sees DENIED; the resolved path stays in the server log so a
// rejection cannot be used to probe the filesystem.
fn reject(requested: &str, reason: &str) -> String {
    eprintln!("denied {requested:?}: {reason}");
    DENIED.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sandbox {
        base: PathBuf,
    }

    impl Sandbox {
        fn new(name: &str) -> Self {
            let base = std::env::temp_dir()
                .canonicalize()
                .unwrap()
                .join(format!("enframe_roots_{}_{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            std::fs::create_dir_all(&base).unwrap();
            Self { base }
        }

        fn dir(&self, name: &str) -> PathBuf {
            let path = self.base.join(name);
            std::fs::create_dir_all(&path).unwrap();
            path
        }

        fn file(&self, name: &str) -> PathBuf {
            let path = self.base.join(name);
            std::fs::write(&path, b"x").unwrap();
            path
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn text(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn nested_and_exact_root_are_allowed() {
        let sandbox = Sandbox::new("nested");
        let root = sandbox.dir("root");
        std::fs::create_dir_all(root.join("deep")).unwrap();
        std::fs::write(root.join("deep/photo.jpg"), b"x").unwrap();
        let roots = Roots::new(vec![root.clone()], vec![]).unwrap();

        assert_eq!(roots.check_read(&text(&root)).unwrap(), root);
        assert_eq!(
            roots
                .check_read(&text(&root.join("deep/photo.jpg")))
                .unwrap(),
            root.join("deep/photo.jpg")
        );
    }

    #[test]
    fn dot_dot_traversal_is_rejected() {
        let sandbox = Sandbox::new("traversal");
        let root = sandbox.dir("root");
        let outside = sandbox.file("outside.txt");
        let roots = Roots::new(vec![root.clone()], vec![]).unwrap();

        let escape = text(&root.join("../outside.txt"));
        assert_eq!(roots.check_read(&escape), Err(DENIED.to_string()));
        assert!(roots.check_read(&text(&outside)).is_err());
    }

    #[test]
    fn symlink_out_of_root_is_rejected() {
        let sandbox = Sandbox::new("symlink");
        let root = sandbox.dir("root");
        let secret = sandbox.file("secret.txt");
        let link = root.join("link.txt");
        std::os::unix::fs::symlink(&secret, &link).unwrap();
        let roots = Roots::new(vec![root.clone()], vec![root.clone()]).unwrap();

        assert_eq!(roots.check_read(&text(&link)), Err(DENIED.to_string()));
        assert_eq!(roots.check_write(&text(&link)), Err(DENIED.to_string()));
    }

    #[test]
    fn prefix_sibling_is_not_inside_root() {
        let sandbox = Sandbox::new("prefix");
        let root = sandbox.dir("root");
        let sibling = sandbox.dir("rootX");
        std::fs::write(sibling.join("photo.jpg"), b"x").unwrap();
        let roots = Roots::new(vec![root], vec![]).unwrap();

        assert_eq!(roots.check_read(&text(&sibling)), Err(DENIED.to_string()));
        assert_eq!(
            roots.check_read(&text(&sibling.join("photo.jpg"))),
            Err(DENIED.to_string())
        );
    }

    #[test]
    fn write_allows_missing_file_but_not_missing_parent() {
        let sandbox = Sandbox::new("write");
        let root = sandbox.dir("root");
        let roots = Roots::new(vec![], vec![root.clone()]).unwrap();

        let new_file = root.join("export.avif");
        assert_eq!(roots.check_write(&text(&new_file)).unwrap(), new_file);
        assert_eq!(
            roots.check_write(&text(&root.join("nope/export.avif"))),
            Err(DENIED.to_string())
        );
    }

    #[test]
    fn write_rejects_dot_dot_and_empty_names() {
        let sandbox = Sandbox::new("names");
        let root = sandbox.dir("root");
        let roots = Roots::new(vec![], vec![root.clone()]).unwrap();

        assert_eq!(
            roots.check_write(&text(&root.join(".."))),
            Err(DENIED.to_string())
        );
        assert_eq!(roots.check_write(""), Err(DENIED.to_string()));
        assert_eq!(roots.check_write("/"), Err(DENIED.to_string()));
    }

    #[test]
    fn write_root_is_readable_but_read_root_is_not_writable() {
        let sandbox = Sandbox::new("cross");
        let readable = sandbox.dir("readable");
        let writable = sandbox.dir("writable");
        std::fs::write(writable.join("photo.jpg"), b"x").unwrap();
        let roots = Roots::new(vec![readable.clone()], vec![writable.clone()]).unwrap();

        assert_eq!(
            roots
                .check_read(&text(&writable.join("photo.jpg")))
                .unwrap(),
            writable.join("photo.jpg")
        );
        assert_eq!(
            roots.check_write(&text(&readable.join("export.avif"))),
            Err(DENIED.to_string())
        );
    }

    #[test]
    fn missing_root_fails_fast() {
        let sandbox = Sandbox::new("missing");
        let error = Roots::new(vec![sandbox.base.join("absent")], vec![]).unwrap_err();
        assert!(error.contains("absent"), "{error}");
        assert!(Roots::new(vec![], vec![]).is_err());
    }
}
