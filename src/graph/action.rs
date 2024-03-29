use std::fmt;
use std::path::{Path, PathBuf};
use std::slice;

use mlua::{Function, Lua};

use crate::action::{
    Action, CommandAction, FunctionAction, HandlebarsAction, JsonAction, LinkAction, LiquidAction,
    MkdirAction, TomlAction, TreeAction, WriteAction, YamlAction,
};
use crate::fse;
use crate::graph::PackageData;
use crate::spec::{
    CmdHook, DirFile, Directive, File, FunHook, GeneratedFile, GeneratedFileTyp, Hook, LinkType,
    RegularFile, TemplatedFile, TemplatedFileType, TreeFile,
};

impl PackageData {
    #[inline]
    pub fn action_iter<P>(&self, dest: P) -> ActionIter<'_>
    where
        P: AsRef<Path>,
    {
        ActionIter {
            dest: dest.as_ref().to_path_buf(),
            path: &self.path,
            lua: &self.lua,
            directives: self.spec.directives.iter(),
        }
    }
}

pub struct ActionIter<'g> {
    dest: PathBuf,
    path: &'g Path,
    lua: &'g Lua,

    directives: slice::Iter<'g, Directive>,
}

impl<'p> fmt::Debug for ActionIter<'p> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActionIter")
            .field("dest", &self.dest)
            .field("path", &self.path)
            .field("lua", &"<lua>")
            .field("directives", &self.directives)
            .finish()
    }
}

impl<'g> Iterator for ActionIter<'g> {
    type Item = Action<'g>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let drct = self.directives.next()?;
        Some(self.get_directive(drct))
    }
}

impl<'g> ActionIter<'g> {
    #[inline]
    fn get_directive(&self, drct: &Directive) -> Action<'g> {
        match drct {
            Directive::File(f) => self.get_file(f),
            Directive::Hook(h) => self.get_hook(h),
        }
    }

    #[inline]
    fn get_file(&self, f: &File) -> Action<'g> {
        match f {
            File::Regular(rf) => self.get_file_regular(rf),
            File::Templated(tf) => self.get_file_template(tf),
            File::Tree(tf) => self.get_file_tree(tf),
            File::Generated(gf) => self.get_file_generated(gf),
            File::Dir(df) => self.get_file_dir(df),
        }
    }

    #[inline]
    fn get_file_regular(&self, rf: &RegularFile) -> Action<'g> {
        let RegularFile {
            src,
            dest,
            link_type,
            optional,
        } = rf;

        // Normalize src.
        let src_w = self.join_package(src);
        // Normalize dest (or use src if absent).
        let dest_w = self.join_dest(dest.as_ref().unwrap_or(src));

        // Determine copy flag.
        let copy = match link_type {
            LinkType::Link => false,
            LinkType::Copy => true,
        };

        Action::Link(LinkAction {
            src: src_w,
            dest: dest_w,
            copy,
            optional: *optional,
        })
    }

    #[inline]
    fn get_file_template(&self, tf: &TemplatedFile) -> Action<'g> {
        let TemplatedFile {
            src,
            dest,
            vars,
            typ,
            optional,
        } = tf;

        // Normalize src.
        let src_w = self.join_package(src);
        // Normalize dest.
        let dest_w = self.join_dest(dest);

        match typ {
            TemplatedFileType::Handlebars(hbs) => Action::Handlebars(HandlebarsAction {
                src: src_w,
                dest: dest_w,
                vars: vars.clone(),
                optional: *optional,
                partials: hbs.partials.clone(),
            }),
            TemplatedFileType::Liquid(_) => Action::Liquid(LiquidAction {
                src: src_w,
                dest: dest_w,
                vars: vars.clone(),
                optional: *optional,
            }),
        }
    }

    #[inline]
    fn get_file_tree(&self, tf: &TreeFile) -> Action<'g> {
        let TreeFile {
            src,
            dest,
            globs,
            ignore,
            link_type,
            optional,
        } = tf;

        // Normalize src.
        let src_w = self.join_package(src);
        // Normalize dest.
        let dest_w = dest
            .as_ref()
            .map(|dest| self.join_dest(dest))
            .unwrap_or_else(|| self.dest.clone());

        // FIXME no clone
        let globs = globs.clone().unwrap_or_else(|| vec!["**/*".to_string()]);
        let ignore = ignore.clone().unwrap_or_default();

        // Determine copy flag.
        let copy = match link_type {
            LinkType::Link => false,
            LinkType::Copy => true,
        };

        Action::Tree(TreeAction {
            src: src_w,
            dest: dest_w,
            globs,
            ignore,
            copy,
            optional: *optional,
        })
    }

    #[inline]
    fn get_file_generated(&self, gf: &GeneratedFile) -> Action<'g> {
        let GeneratedFile { dest, typ } = gf;

        // Normalize dest.
        let dest_w = self.join_dest(dest);

        match typ {
            GeneratedFileTyp::Empty(_) => Action::Write(WriteAction {
                dest: dest_w,
                contents: "".to_string().into_bytes(),
            }),
            GeneratedFileTyp::String(s) => Action::Write(WriteAction {
                dest: dest_w,
                contents: s.contents.clone().into_bytes(),
            }),
            // FIXME error context
            GeneratedFileTyp::Yaml(y) => Action::Yaml(YamlAction {
                dest: dest_w,
                values: y.values.clone(),
                header: y.header.clone(),
            }),
            GeneratedFileTyp::Toml(t) => Action::Toml(TomlAction {
                dest: dest_w,
                values: t.values.clone(),
                header: t.header.clone(),
            }),
            GeneratedFileTyp::Json(j) => Action::Json(JsonAction {
                dest: dest_w,
                values: j.values.clone(),
            }),
        }
    }

    #[inline]
    fn get_file_dir(&self, df: &DirFile) -> Action<'g> {
        let DirFile { dest, parents } = df;

        let path = self.join_dest(dest);
        Action::Mkdir(MkdirAction {
            path,
            parents: *parents,
        })
    }

    #[inline]
    fn get_hook(&self, h: &Hook) -> Action<'g> {
        match h {
            Hook::Cmd(cmd) => self.get_hook_cmd(cmd),
            Hook::Fun(fun) => self.get_hook_fun(fun),
        }
    }

    #[inline]
    fn get_hook_cmd(&self, cmd: &CmdHook) -> Action<'g> {
        let CmdHook {
            command,
            start,
            shell,
            clean_env,
            env,

            // TODO: How to use these?
            stdout: _,
            stderr: _,
            nonzero_exit: _,
        } = cmd;

        // Normalize start path.
        let start = start
            .as_ref()
            .map(|start| self.join_package(start))
            .unwrap_or_else(|| self.path.to_path_buf());

        let command = command.clone();
        // Use sh as default shell.
        let shell = shell.clone().unwrap_or_else(|| "sh".to_string());
        let clean_env = *clean_env.as_ref().unwrap_or(&false);
        let env = env.clone().unwrap_or_default();

        Action::Command(CommandAction {
            command,
            start,
            shell,
            clean_env,
            env,
        })
    }

    #[inline]
    fn get_hook_fun(&self, fun: &FunHook) -> Action<'g> {
        let FunHook {
            name,
            start,
            nonzero_exit: _,
        } = fun;

        // Load function from Lua registry.
        let function: Function = self.lua.named_registry_value(name).unwrap();
        let start = start
            .as_ref()
            .map(|start| self.join_package(start))
            .unwrap_or_else(|| self.path.to_path_buf());

        Action::Function(FunctionAction { function, start })
    }

    #[inline]
    fn join_package<P>(&self, path: P) -> PathBuf
    where
        P: AsRef<Path>,
    {
        self.normalize_path(path, &self.path)
    }

    #[inline]
    fn join_dest<P>(&self, path: P) -> PathBuf
    where
        P: AsRef<Path>,
    {
        self.normalize_path(path, &self.dest)
    }

    #[inline]
    fn normalize_path<P, S>(&self, path: P, start: S) -> PathBuf
    where
        P: AsRef<Path>,
        S: AsRef<Path>,
    {
        fse::clean(start.as_ref().join(path))
    }
}
