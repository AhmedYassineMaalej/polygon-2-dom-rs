use std::io;
use std::{
    env,
    path::{Path, PathBuf},
};

pub struct DirGuard {
    original: PathBuf,
}

impl DirGuard {
    pub fn change_to<P: AsRef<Path>>(new_dir: P) -> io::Result<Self> {
        let original = env::current_dir()?;
        env::set_current_dir(new_dir)?;
        Ok(Self { original })
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.original);
    }
}
