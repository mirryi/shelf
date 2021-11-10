use std::fs;
use std::path::PathBuf;

use crate::op::{CreateOp, Op, RmOp, WriteOp};

use super::error::NoError;
use super::{link, Done, Notice, Resolution, Resolve, ResolveOpts, WarnNotice};

#[derive(Debug, Clone)]
pub struct WriteAction {
    pub dest: PathBuf,
    pub contents: String,
}

pub type WriteActionError = NoError;

impl<'lua> Resolve<'lua> for WriteAction {
    type Error = WriteActionError;

    #[inline]
    fn resolve(&self, _opts: &ResolveOpts) -> Result<Resolution<'lua>, Self::Error> {
        let Self { dest, contents } = self;

        let mut output = Done::empty();

        // If the destination file already exists, check the filetype.
        match fs::symlink_metadata(dest) {
            Ok(meta) if meta.is_file() => {
                output
                    .notices
                    .push(Notice::Warn(WarnNotice::Overwrite { path: dest.clone() }));
                output.ops.push(Op::Rm(RmOp {
                    path: dest.clone(),
                    dir: false,
                }));

                // FIXME: For files, check the contents. If they match, we should do nothing.
                // let content_same = match fs::read_to_string(dest) {
                // Ok(dest_contents) => dest_contents == contents,
                // // If error, just assume content is different
                // Err(_) => false,
                // };

                // if content_same {
                // return Ok(Resolution::Skip(SkipReason::DestinationExists {
                // path: path.clone(),
                // }));
                // }
            }
            // For directories, warn about an overwrite, remove the directory, and then
            // link.
            //
            // FIXME: https://github.com/rust-lang/rust/pull/89677
            Ok(meta) if meta.is_dir() | meta.is_symlink() => {
                output
                    .notices
                    .push(Notice::Warn(WarnNotice::Overwrite { path: dest.clone() }));

                let dir = meta.is_dir();
                output.ops.push(Op::Rm(RmOp {
                    path: dest.clone(),
                    dir,
                }));
            }
            // File doesn't exist, or insufficient permissions; treat as nonexistent.
            Ok(_) | Err(_) => {
                // We need to first create a file before writing to it.
                output.ops.push(Op::Create(CreateOp { path: dest.clone() }));
            }
        };

        // Check for existence of parent directories and add op to make parent directories if
        // they don't exist.
        if let Some(mkparents_op) = link::mkparents_op(dest) {
            output.ops.push(Op::Mkdir(mkparents_op));
        }

        output.ops.push(Op::Write(WriteOp {
            path: dest.clone(),
            contents: contents.clone().into_bytes(),
        }));

        Ok(Resolution::Done(output))
    }
}
