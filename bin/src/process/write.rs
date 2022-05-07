use shelflib::{
    action::{
        write::{self, Res},
        Resolve, WriteAction,
    },
    op::Op,
};

use super::GraphProcessor;
use crate::ctxpath::CtxPath;

impl<'p, 'g> GraphProcessor<'p, 'g> {
    #[inline]
    pub fn resolve_write(
        &self,
        action: WriteAction,
        _path: &CtxPath,
    ) -> Result<Vec<Op<'static>>, ()> {
        let res = action.resolve();
        match res {
            Res::Normal(ops) => {
                // TODO: Output
                Ok(map_ops(ops))
            }
            Res::OverwriteContents(ops) => {
                // TODO: Output
                Ok(map_ops(ops))
            }
            Res::OverwriteFile(ops) => {
                // TODO: Output
                Ok(map_ops(ops))
            }
            Res::Skip(_skip) => {
                // TODO: Output
                Ok(vec![])
            }
        }
    }
}

#[inline]
fn map_ops(ops: Vec<write::Op>) -> Vec<Op<'static>> {
    ops.into_iter()
        .map(|op| match op {
            write::Op::Rm(op) => Op::Rm(op),
            write::Op::Create(op) => Op::Create(op),
            write::Op::Write(op) => Op::Write(op),
            write::Op::Mkdir(op) => Op::Mkdir(op),
        })
        .collect()
}
