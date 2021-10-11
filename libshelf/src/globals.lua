-- name 'test'
function name(value)
	pkg:name(value)
end

-- dep 'path'
-- dep {'path', ...}
-- dep 'path1' 'path2' ...
-- dep {'path1', 'path2', ...} { ... } ...
function dep(...)
	pkg:dep(...)
	return dep
end

-- file 'a.txt'
-- file {'b.txt'}
-- file {'c.txt', 'd.txt'}
-- file {'e.txt', 'f.txt', type = 'copy'}
-- file {'g.txt', type = 'copy'}
-- file {'h.txt', optional = true}
function file(arg)
	local src, dest, link_type, optional
	if type(arg) == "string" then
		src = arg
		dest = nil
		link_type = nil
		optional = nil
	elseif type(arg) == "table" then
		src = arg[1] or error("file src path was not provided")
		dest = arg[2]
		link_type = arg.type
		optional = arg.optional
	else
		error("invalid file directive")
	end

	pkg:file(src, dest, link_type, optional)
end

function link(arg)
	if type(arg) == "table" then
		arg.link_type = "link"
	end
	file(arg)
end

function copy(arg)
	if type(arg) == "table" then
		arg.link_type = "copy"
	end
	file(arg)
end

-- tree 'tree'
-- tree {'tree'}
-- tree {'tree', '.config'}
-- tree {'tree', type = 'copy'}
-- tree {'tree', ignore = '**/*.log'}
-- tree {'tree', '.config', type = 'copy'}
-- tree {'tree', '.config', ignore = '**/*.log'}
-- tree {'tree', '.config', type = 'copy', ignore = '**/*.log'}
-- tree {'tree', optional = true}
function tree(arg)
	local src, dest, link_type, globs, ignore, optional
	if type(arg) == "string" then
		src = arg
		dest = nil
		link_type = nil
		globs = nil
		ignore = nil
		optional = nil
	elseif type(arg) == "table" then
		src = arg[1] or error("tree src path was not provided")
		dest = arg[2]
		link_type = arg.type
		globs = arg.globs
		ignore = arg.ignore
		optional = arg.optional

		if type(globs) == "string" then
			globs = { globs }
		end
		if type(ignore) == "string" then
			ignore = { ignore }
		end
	else
		error("tree arg must be a string or table")
	end

	pkg:tree(src, dest, link_type, globs, ignore, optional)
end

-- template {'d.hbs', 'j.txt', engine = 'handlebars', vars = {}}
-- template {'d.tmpl', 'k.txt', engine = 'liquid', vars = {}}
-- template {'d.hbs', 'j.txt', engine = 'hbs', vars = {}, optional = true}
function template(arg)
	if type(arg) == "table" then
		local engine = arg.engine or error("template engine was not provided")
		if engine == "hbs" then
			hbs(arg)
		elseif engine == "liquid" then
			liquid(arg)
		else
			error("template engine must be hbs or liquid")
		end
	else
		error("template arg must be a table")
	end
end

-- hbs {'b.hbs', 'h.txt', vars = {}}
-- hbs {'b.hbs', 'h.txt', vars = {}, optional = true}
function hbs(arg)
	src = arg[1] or error("template src was not provided")
	dest = arg[2] or error("template dest was not provided")
	vars = arg.vars or error("template vars was not provided")
	partials = arg.partials or {}
	optional = arg.optional

	pkg:hbs(src, dest, vars, partials, optional)
end

-- liquid {'b.tmpl', 'i.txt', vars = {}}
-- liquid {'b.tmpl', 'i.txt', vars = {}, optional = true}
function liquid(arg)
	src = arg[1] or error("template src was not provided")
	dest = arg[2] or error("template dest was not provided")
	vars = arg.vars or error("template vars was not provided")
	optional = arg.optional

	pkg:liquid(src, dest, vars, optional)
end

-- empty 'l.txt'
-- empty {'m.txt'}
function empty(arg)
	if type(arg) == "string" then
		pkg:empty(arg)
	elseif type(arg) == "table" then
		local path = arg[1] or error("empty dest was not provided")
		pkg:empty(path)
	else
		error("empty dest must be a string or table")
	end
end

-- string {'n.txt', 'contents'}
function str(arg)
	if type(arg) == "table" then
		local dest = arg[1] or error("str dest was not provided")
		local contents = arg[2] or error("str contents was not provided")
		pkg:str(dest, contents)
	else
		error("str arg must be a table")
	end
end

-- yaml {'o.txt', {}}
-- yaml {'p.txt', {}, header = '# header'}
function yaml(arg)
	if type(arg) == "table" then
		local dest = arg[1] or error("yaml dest was not provided")
		local values = arg[2] or error("yaml values were not provided")
		local header = arg.header
		pkg:yaml(dest, values, header)
	else
		error("yaml arg must be a table")
	end
end

-- toml {'q.txt', {}}
-- toml {'r.txt', {}, header = '# header'}
function toml(arg)
	if type(arg) == "table" then
		local dest = arg[1] or error("toml dest was not provided")
		local values = arg[2] or error("toml values were not provided")
		local header = arg.header
		pkg:toml(dest, values, header)
	else
		error("toml arg must be a table")
	end
end

-- json {'s.txt', {}}
function json(arg)
	if type(arg) == "table" then
		local dest = arg[1] or error("toml dest was not provided")
		local values = arg[2] or error("toml values were not provided")
		pkg:json(dest, values)
	else
		error("json arg must be a table")
	end
end

-- mkdir 'd'
-- mkdir {'d'}
function mkdir(arg)
	if type(arg) == "table" then
		local dest = arg[1] or error("mkdir dest was not provided")
		pkg:mkdir(dest)
	else
		pkg:mkdir(arg)
	end
end

-- cmd [[echo "a"]]
-- cmd {[[echo "a"]]}
-- cmd {[[echo "a"]], quiet = true}
-- cmd {[[echo "a"]], start = "tree"}
-- cmd {[[echo "a"]], shell = "zsh"}
-- cmd {[[echo "a"]], quiet = true, start = "tree"}
-- cmd {[[echo "a"]], quiet = true, shell = "zsh"}
-- cmd {[[echo "a"]], start = "tree", shell = "zsh"}
-- cmd {[[echo "a"]], quiet = true, start = "tree", shell = "zsh"}
function cmd(arg)
	local command, start, shell, stdout, stderr, clean_env, env, nonzero_exit
	if type(arg) == "string" then
		command = arg
		start = nil
		shell = nil
		stdout = nil
		stderr = nil
		clean_env = nil
		env = nil
		nonzero_exit = nil
	elseif type(arg) == "table" then
		command = arg[1] or error("cmd command was not provided")
		start = arg.start
		shell = arg.shell
		stdout = arg.stdout
		stderr = arg.stderr
		clean_env = arg.clean_env
		env = arg.env
		nonzero_exit = arg.nonzero_exit
	else
		error("cmd arg must be a string or table")
	end

	pkg:cmd(command, start, shell, stdout, stdin, clean_env, env, nonzero_exit)
end

-- fn(function() print("a") end)
-- fn {function() print("a") end}
-- fn {function() print("a") end, error_exit = "error"}
function fn(arg)
	local fun, start, error_exit
	if type(arg) == "function" then
		fun = arg
		start = nil
		error_exit = nil
	elseif type(arg) == "table" then
		fun = arg[1] or error("fn function was not provided")
		start = arg.start
		error_exit = arg.error_exit
	else
		error("fn arg must be a function or table")
	end

	pkg:fn(fun, error_exit)
end
