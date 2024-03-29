use std::fs;
use std::path::PathBuf;

use crate::fse;
use crate::op::{CopyOp, LinkOp, MkdirOp, RmOp};

use super::{mkdir, Resolve};

/// Action to symlink or copy from `src` to `dest`.
#[derive(Debug, Clone)]
pub struct LinkAction {
    /// Path of file to symlink/copy.
    pub src: PathBuf,
    /// Path of destination of symlink/copy.
    pub dest: PathBuf,

    /// Perform a copy instead of a symlink.
    pub copy: bool,
    /// If the `src` does not exist, emit no operations.
    pub optional: bool,
}

/// Error that occurs when resolving [`LinkAction`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// `src` was not found, and `optional` was false.
    #[error("src missing")]
    SrcMissing,
}

// Resolution of [`LinkAction`].
#[derive(Debug, Clone)]
pub enum Res {
    /// Normal procedure.
    Normal(Vec<Op>),
    /// The destination file or directory will be overwritten.
    Overwrite(Vec<Op>),
    /// The action is skipped.
    Skip(Skip),
}

/// Operation created by resolution.
#[derive(Debug, Clone)]
pub enum Op {
    /// Remove operation.
    Rm(RmOp),
    /// Link operation.
    Link(LinkOp),
    /// Copy operation.
    Copy(CopyOp),
    /// Mkdir operation.
    Mkdir(MkdirOp),
}

/// Reason for skipping [`LinkAction`].
#[derive(Debug, Clone)]
pub enum Skip {
    /// `src` and `dest` are the same path.
    SameSrcDest,
    /// Optional `src` does not exist.
    OptMissing,
    /// Destination link already exists.
    DestExists,
}

impl Resolve for LinkAction {
    type Output = Result<Res, Error>;

    #[inline]
    fn resolve(&self) -> Self::Output {
        let Self {
            src,
            dest,
            copy,
            optional,
        } = self;

        // If src and dest are the same, skip.
        if src == dest {
            return Ok(Res::Skip(Skip::SameSrcDest));
        }

        // If file does not exist and optional flag enabled, skip.
        // If optional flag disabled, error.
        match (optional, fse::symlink_exists(src)) {
            (true, false) => {
                return Ok(Res::Skip(Skip::OptMissing));
            }
            (false, false) => {
                return Err(Error::SrcMissing);
            }
            _ => {}
        };

        if *copy {
            self.resolve_copy()
        } else {
            self.resolve_link()
        }
    }
}

// TODO: Reduce code duplication
impl LinkAction {
    #[inline]
    fn resolve_link(&self) -> Result<Res, Error> {
        let Self {
            src,
            dest,
            copy: _,
            optional: _,
        } = self;

        // Check the filetype and determine if overwrite is necessary.
        let (overwrite, is_dir) = match fs::symlink_metadata(dest) {
            // For symlinks, check the target.
            // If it's the same as src, skip.
            Ok(meta) if meta.is_symlink() => {
                // SAFETY: Already determined it exists and is a symlink.
                let target = fs::read_link(dest).unwrap();
                if target == *src {
                    return Ok(Res::Skip(Skip::DestExists));
                } else {
                    (true, false)
                }
            }

            // For existing files and directories, warn about an overwrite.
            // Remove the file, and then link.
            Ok(meta) if meta.is_dir() => (true, true),
            Ok(meta) if meta.is_file() => (true, false),

            // File doesn't exist, or insufficient permissions.
            // Treat as nonexistent.
            Ok(_) | Err(_) => (false, false),
        };

        let link_op = Op::Link(LinkOp {
            src: src.clone(),
            dest: dest.clone(),
        });
        if overwrite {
            // Add op to remove existing file if exist.
            let rm_op = Op::Rm(RmOp {
                path: dest.clone(),
                dir: is_dir,
            });

            Ok(Res::Overwrite(vec![rm_op, link_op]))
        } else {
            // Check for existence of parent directories and add op to make parent directories if
            // they don't exist.
            let mut ops: Vec<_> = mkdir::mkdir_parents_ops(dest).map(Op::Mkdir).collect();

            ops.push(link_op);

            Ok(Res::Normal(ops))
        }
    }

    #[inline]
    fn resolve_copy(&self) -> Result<Res, Error> {
        let Self { src, dest, .. } = self;

        let src_is_dir = match fs::symlink_metadata(src) {
            Ok(meta) if meta.is_dir() => true,
            Ok(_) | Err(_) => false,
        };

        let (overwrite_dest, dest_is_dir) = match fs::symlink_metadata(dest) {
            // For files, check the contents. If they match, we should do nothing.
            // If not, proceed with overwrite.
            Ok(meta) if meta.is_file() => {
                let content_same = fs::read_to_string(src)
                    .map(|src_contents| match fs::read_to_string(dest) {
                        Ok(dest_contents) => src_contents == dest_contents,
                        Err(_) => false,
                    })
                    .unwrap_or(false);
                if content_same {
                    return Ok(Res::Skip(Skip::DestExists));
                }

                (true, false)
            }

            // For directories and symlinks, warn about an overwrite.
            // Remove the directory, and then link.
            Ok(meta) if meta.is_dir() => (true, true),
            Ok(meta) if meta.is_symlink() => (true, false),

            // File doesn't exist, or insufficient permissions; treat as nonexistent.
            // TODO: Treat error as error here?
            Ok(_) | Err(_) => (false, false),
        };

        let copy_op = Op::Copy(CopyOp {
            src: src.clone(),
            dest: dest.clone(),
            dir: src_is_dir,
        });
        if overwrite_dest {
            // Add op to remove existing file if exist.
            let rm_op = Op::Rm(RmOp {
                path: dest.clone(),
                dir: dest_is_dir,
            });

            Ok(Res::Overwrite(vec![rm_op, copy_op]))
        } else {
            // Check for existence of parent directories and add op to make parent directories if
            // they don't exist.
            let mut ops: Vec<_> = mkdir::mkdir_parents_ops(dest).map(Op::Mkdir).collect();

            ops.push(copy_op);
            Ok(Res::Normal(ops))
        }
    }
}
