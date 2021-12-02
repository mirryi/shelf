use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use mlua::Lua;

use self::specobject::SpecObject;
use crate::graph::PackageData;

static CONFIG_FILE: &str = "package.lua";

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("couldn't read a file")]
    Read(#[from] io::Error),
    #[error("couldn't execute Lua")]
    Lua(#[from] mlua::Error),
}

/// Loader for a package.
pub struct SpecLoader<S>
where
    S: SpecLoaderState,
{
    path: PathBuf,
    contents: String,
    lua: Lua,

    state: PhantomData<S>,
}

pub type SpecLoaderEmpty = SpecLoader<SpecLoaderStateEmpty>;
pub type SpecLoaderRead = SpecLoader<SpecLoaderStateRead>;
pub type SpecLoaderEvaled = SpecLoader<SpecLoaderStateEvaled>;

/// Typestates for [`SpecLoader`].
pub trait SpecLoaderState: specobject::SpecLoaderState {}
macro_rules! spec_loader_state {
    ($name:ident) => {
        pub struct $name;
        impl specobject::SpecLoaderState for $name {}
        impl SpecLoaderState for $name {}
    };
    ($($names:ident),* $(,)?) => {
        $(spec_loader_state!($names);)*
    }
}

spec_loader_state!(
    SpecLoaderStateEmpty,
    SpecLoaderStateRead,
    SpecLoaderStateEvaled
);

impl SpecLoaderEmpty {
    /// Create a loader for the package at the given path.
    #[inline]
    pub fn new<P>(path: P) -> Result<Self, LoadError>
    where
        P: AsRef<Path>,
    {
        let lua = Self::lua_instance()?;
        Ok(Self {
            path: path.as_ref().to_owned(),
            contents: String::new(),
            lua,
            state: PhantomData,
        })
    }

    #[inline]
    fn lua_instance() -> Result<Lua, mlua::Error> {
        #[cfg(not(feature = "unsafe"))]
        let lua = Lua::new();
        #[cfg(feature = "unsafe")]
        let lua = unsafe { Lua::unsafe_new() };

        lua.globals().set("pkg", SpecObject::new())?;
        lua.load(std::include_str!("globals.lua")).exec()?;

        Ok(lua)
    }

    /// Load the package, returning a [`PackageData`].
    #[inline]
    pub fn load<P>(path: P) -> Result<PackageData, LoadError>
    where
        P: AsRef<Path>,
    {
        let loader = Self::new(path)?;
        loader.finish()
    }

    /// Read the configuration contents.
    #[inline]
    pub fn read(mut self) -> Result<SpecLoaderRead, io::Error> {
        let config_path = self.path.join(CONFIG_FILE);
        let mut file = File::open(config_path)?;
        file.read_to_string(&mut self.contents)?;

        Ok(SpecLoader {
            path: self.path,
            contents: self.contents,
            lua: self.lua,
            state: PhantomData,
        })
    }

    /// Load the package, returning a [`PackageData`].
    #[inline]
    pub fn finish(self) -> Result<PackageData, LoadError> {
        let res = self.read()?.eval()?.to_package_data()?;
        Ok(res)
    }
}

impl SpecLoaderRead {
    #[inline]
    pub fn eval(self) -> Result<SpecLoaderEvaled, mlua::Error> {
        // FIXME propogate error
        // Save current cwd.
        let cwd = env::current_dir().unwrap();
        // Work relative to the package root.
        env::set_current_dir(&self.path).unwrap();

        // Eval lua.
        let chunk = self.lua.load(&self.contents);
        chunk.exec()?;

        // Reload cwd.
        env::set_current_dir(&cwd).unwrap();

        Ok(SpecLoader {
            path: self.path,
            contents: self.contents,
            lua: self.lua,
            state: PhantomData,
        })
    }

    /// Load the package, returning a [`PackageData`].
    #[inline]
    pub fn finish(self) -> Result<PackageData, LoadError> {
        let res = self.eval()?.to_package_data()?;
        Ok(res)
    }
}

impl SpecLoaderEvaled {
    #[inline]
    pub fn to_package_data(self) -> Result<PackageData, mlua::Error> {
        let package: SpecObject = self.lua.globals().get("pkg")?;
        Ok(PackageData {
            path: self.path,
            spec: package.spec,
            lua: self.lua,
        })
    }

    #[inline]
    pub fn finish(self) -> Result<PackageData, mlua::Error> {
        self.to_package_data()
    }
}

mod specobject {
    use std::collections::HashMap;

    use mlua::{
        Error as LuaError, FromLua, Function, UserData, UserDataMethods, Value as LuaValue,
        Variadic,
    };
    use uuid::Uuid;

    use crate::spec::{
        CmdHook, Dep, DirFile, Directive, EmptyGeneratedFile, File, FunHook, GeneratedFile,
        GeneratedFileTyp, HandlebarsTemplatedFile, Hook, JsonGeneratedFile, LinkType,
        LiquidTemplatedFile, NonZeroExitBehavior, Object, ObjectValue, Patterns, RegularFile, Spec,
        StringGeneratedFile, TemplatedFile, TemplatedFileType, TomlGeneratedFile, TreeFile,
        YamlGeneratedFile,
    };

    pub trait SpecLoaderState {}

    #[derive(Debug, Clone)]
    pub(super) struct SpecObject {
        pub(super) spec: Spec,
    }

    impl SpecObject {
        #[inline]
        pub fn new() -> Self {
            Self {
                spec: Spec {
                    name: String::new(),
                    deps: Vec::new(),
                    directives: Vec::new(),
                },
            }
        }
    }

    impl UserData for SpecObject {
        #[inline]
        fn add_fields<'lua, F: mlua::UserDataFields<'lua, Self>>(_fields: &mut F) {}

        #[inline]
        fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
            macro_rules! method {
                ($name:expr; ($($arg:ident; $ty:ty),*); $drct:expr) => {
                    #[allow(unused_parens)]
                    methods.add_method_mut($name, |_, this, arg: ($($ty),*)| {
                        let ($($arg),*) = arg;
                        this.spec.directives.push($drct);
                        Ok(())
                    });
                };
                ($name:expr; ($($arg:ident; $ty:ty),*); File; $drct:expr) => {
                    method!($name; ($($arg; $ty),*); Directive::File($drct))
                };
                ($name:expr; ($($arg:ident; $ty:ty),*); Gen; $drct:expr) => {
                    method!($name; ($($arg; $ty),*); Directive::File(File::Generated($drct)))
                };
                ($name:expr; ($($arg:ident; $ty:ty),*); Hook; $drct:expr) => {
                    method!($name; ($($arg; $ty),*); Directive::Hook($drct))
                };
            }

            methods.add_method_mut("name", |_, this, name: String| {
                this.spec.name = name;
                Ok(())
            });

            methods.add_method_mut("dep", |_, this, paths: Variadic<String>| {
                this.spec
                    .deps
                    .extend(paths.into_iter().map(|p| Dep { path: p.into() }));
                Ok(())
            });

            method!("file"; (src; String, dest; Option<String>, link_type; Option<LinkType>, optional; Option<bool>);
            File; File::Regular(RegularFile {
                src: src.into(),
                dest: dest.map(Into::into),
                link_type: link_type.unwrap_or(LinkType::Link),
                optional: optional.unwrap_or(false)
            }));

            method!("tree"; (src; String, dest; Option<String>, link_type; Option<LinkType>,
                             globs; Option<Patterns>, ignore; Option<Patterns>, optional; Option<bool>);
            File; File::Tree(TreeFile {
                src: src.into(),
                dest: dest.map(Into::into),
                globs,
                ignore,
                link_type: link_type.unwrap_or(LinkType::Link),
                optional: optional.unwrap_or(false)
            }));

            method!("hbs"; (src; String, dest; String, vars; Object, partials; HashMap<String, String>, optional; Option<bool>);
            File; {
                let partials = partials.into_iter().map(|(k, v)| (k, v.into())).collect();
                File::Templated(TemplatedFile {
                    src: src.into(),
                    dest: dest.into(),
                    vars,
                    typ: TemplatedFileType::Handlebars(HandlebarsTemplatedFile { partials }),
                    optional: optional.unwrap_or(false)
                })
            });

            method!("liquid"; (src; String, dest; String, vars; Object, optional; Option<bool>);
            File; File::Templated(TemplatedFile {
                src: src.into(),
                dest: dest.into(),
                vars,
                typ: TemplatedFileType::Liquid(LiquidTemplatedFile {}),
                optional: optional.unwrap_or(false)
            }));

            method!("empty"; (dest; String);
            Gen; GeneratedFile {
                dest: dest.into(), typ: GeneratedFileTyp::Empty(EmptyGeneratedFile)
            });
            method!("str"; (dest; String, contents; String);
            Gen; GeneratedFile {
                dest: dest.into(), typ: GeneratedFileTyp::String(StringGeneratedFile { contents })
            });
            method!("yaml"; (dest; String, values; Object, header; Option<String>);
            Gen; GeneratedFile {
                dest: dest.into(), typ: GeneratedFileTyp::Yaml(YamlGeneratedFile { values, header })
            });
            method!("toml"; (dest; String, values; Object, header; Option<String>);
            Gen; GeneratedFile {
                dest: dest.into(), typ: GeneratedFileTyp::Toml(TomlGeneratedFile { values, header })
            });
            method!("json"; (dest; String, values; Object);
            Gen; GeneratedFile {
                dest: dest.into(), typ: GeneratedFileTyp::Json(JsonGeneratedFile { values })
            });

            method!("mkdir"; (dest; String, parents; bool);
            File; File::Dir(DirFile {
                dest: dest.into(),
                parents,
            }));

            method!("cmd"; (command; String, start; Option<String>, shell; Option<String>,
                            stdout; Option<bool>, stderr; Option<bool>,
                            clean_env; Option<bool>, env; Option<HashMap<String, String>>,
                            nonzero_exit; Option<NonZeroExitBehavior>);
            Hook; Hook::Cmd(CmdHook {
                command,
                start: start.map(Into::into),
                shell,
                stdout,
                stderr,
                clean_env,
                env,
                nonzero_exit
            }));

            methods.add_method_mut(
                "fn",
                |lua, this, arg: (Function, Option<String>, Option<NonZeroExitBehavior>)| {
                    let (fun, start, nonzero_exit) = arg;

                    let name = Uuid::new_v4().to_string();
                    lua.set_named_registry_value(&name, fun)?;

                    let start = start.map(Into::into);

                    let drct = Directive::Hook(Hook::Fun(FunHook {
                        name,
                        start,
                        nonzero_exit,
                    }));
                    this.spec.directives.push(drct);
                    Ok(())
                },
            );
        }
    }

    impl<'lua> FromLua<'lua> for ObjectValue {
        #[inline]
        fn from_lua(lua_value: LuaValue<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
            let res = match lua_value {
                LuaValue::Nil => ObjectValue::Nil,
                LuaValue::Boolean(b) => ObjectValue::Bool(b),
                LuaValue::Integer(i) => ObjectValue::Int(i),
                LuaValue::Number(n) => ObjectValue::Float(n),
                LuaValue::String(s) => ObjectValue::Str(s.to_str()?.to_string()),
                LuaValue::Table(t) => {
                    ObjectValue::Object(FromLua::from_lua(LuaValue::Table(t), lua)?)
                }
                LuaValue::Function(_)
                | LuaValue::Thread(_)
                | LuaValue::LightUserData(_)
                | LuaValue::UserData(_)
                | LuaValue::Error(_) => {
                    return Err(LuaError::FromLuaConversionError {
                        from: lua_value.type_name(),
                        to: "Value",
                        message: Some(
                            "Only nil, bool, int, float, string, and table values are valid"
                                .to_string(),
                        ),
                    })
                }
            };
            Ok(res)
        }
    }

    impl<'lua> FromLua<'lua> for Object {
        #[inline]
        fn from_lua(lua_value: LuaValue<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
            match lua_value {
                LuaValue::Table(t) => Ok(Object(FromLua::from_lua(LuaValue::Table(t), lua)?)),
                _ => Err(LuaError::FromLuaConversionError {
                    from: lua_value.type_name(),
                    to: "Object",
                    message: Some("Only table values are valid".to_string()),
                }),
            }
        }
    }
}