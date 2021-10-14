use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{Finish, Op, Rollback};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinkOp {
    pub src: PathBuf,
    pub dest: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum LinkOpError {
    #[error("i/o error")]
    Io(#[from] io::Error),
}

impl Finish for LinkOp {
    type Output = ();
    type Error = LinkOpError;

    #[inline]
    fn finish(&self) -> Result<Self::Output, Self::Error> {
        let res = self.symlink()?;
        Ok(res)
    }
}

impl LinkOp {
    #[cfg(unix)]
    #[inline]
    fn symlink(&self) -> io::Result<()> {
        use std::os::unix;

        let Self { src, dest } = self;
        unix::fs::symlink(src, dest)
    }

    #[cfg(windows)]
    #[inline]
    fn symlink(&self) -> io::Result<()> {
        // FIXME: implement
        unimplemented!()
    }
}

impl<'lua> Rollback<Op<'lua>> for LinkOp {
    #[inline]
    fn rollback(&self) -> Op<'lua> {
        todo!()
    }
}