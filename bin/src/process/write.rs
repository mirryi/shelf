use std::path::{Path, };

use shelflib::action::{
    write::{Op, Res, Skip},
    Resolve, WriteAction,
};

use crate::ctxpath::CtxPath;
use crate::pretty::{
    output::{sl_info, sli_debug, sli_warn, slii_warn},
    semantic::{arrowdim, ppath, skipping, var, warning},
};

#[inline]
pub fn process(action: WriteAction, _path: &CtxPath, dest: &Path) -> Result<(), ()> {
    let dest = CtxPath::new(&action.dest, dest).unwrap();

    sl_info(format!("Writing {}", ppath(dest.rel())));
    sli_debug(arrowdim(dest.abs().display()));

    let res = action.resolve();

    match res {
        Res::Normal(ops) => process_normal(&dest, ops),
        Res::OverwriteContents(ops) => process_overwrite_contents(&dest, ops),
        Res::OverwriteFile(ops) => process_overwrite_file(&dest, ops),
        Res::Skip(skip) => process_skip(&dest, skip),
    }

    Ok(())
}

#[inline]
fn process_normal(_dest: &CtxPath, _ops: Vec<Op>) {
    // TODO: Implement
}

#[inline]
fn process_overwrite_contents(dest: &CtxPath, _ops: Vec<Op>) {
    sli_warn(warning(format!(
        "{} will be overwritten",
        ppath(dest.rel())
    )));
    slii_warn(arrowdim(dest.abs().display()));

    // TODO: Implement
}

#[inline]
fn process_overwrite_file(dest: &CtxPath, _ops: Vec<Op>) {
    sli_warn(warning(format!("{} will be replaced", ppath(dest.rel()))));
    slii_warn(arrowdim(dest.abs().display()));

    // TODO: Implement
}

#[inline]
fn process_skip(dest: &CtxPath, skip: Skip) {
    match skip {
        Skip::DestExists => {
            sli_warn(skipping(format!(
                "{} {} already exists",
                var("dest"),
                ppath(dest.rel())
            )));
            slii_warn(arrowdim(dest.abs().display()));
        }
    }
}
