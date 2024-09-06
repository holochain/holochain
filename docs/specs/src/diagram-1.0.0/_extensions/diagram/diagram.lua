--[[
diagram-generator – create images and figures from code blocks.

See copyright notice in file LICENSE.
]]
-- Module pandoc.system is required and was added in version 2.7.3
PANDOC_VERSION:must_be_at_least '3.0'

-- Report Lua warnings to stderr
if warn then
  warn '@on'
else
  warn = function (...) io.stderr:write(string.concat({...})) end
end

local system = require 'pandoc.system'
local utils = require 'pandoc.utils'
local stringify = utils.stringify
local with_temporary_directory = system.with_temporary_directory
local with_working_directory = system.with_working_directory

--- Returns a filter-specific directory in which cache files can be
--- stored, or nil if no such directory is available.
local function cachedir ()
  local cache_home = os.getenv 'XDG_CACHE_HOME'
  if not cache_home or cache_home == '' then
    local user_home = system.os == 'windows'
      and os.getenv 'USERPROFILE'
      or os.getenv 'HOME'

    if not user_home or user_home == '' then
      return nil
    end
    cache_home = pandoc.path.join{user_home, '.cache'} or nil
  end

  -- Create filter cache directory
  return pandoc.path.join{cache_home, 'pandoc-diagram-filter'}
end

--- Path holding the image cache, or `nil` if the cache is not used.
local image_cache = nil

local mimetype_for_extension = {
  jpeg = 'image/jpeg',
  jpg = 'image/jpeg',
  pdf = 'application/pdf',
  png = 'image/png',
  svg = 'image/svg+xml',
}

local extension_for_mimetype = {
  ['application/pdf'] = 'pdf',
  ['image/jpeg'] = 'jpg',
  ['image/png'] = 'png',
  ['image/svg+xml'] = 'svg',
}

--- Converts a list of format specifiers to a set of MIME types.
local function mime_types_set (tbl)
  local set = {}
  local mime_type
  for _, image_format_spec in ipairs(tbl) do
    mime_type = mimetype_for_extension[image_format_spec] or image_format_spec
    set[mime_type] = true
  end
  return set
end

--- Reads the contents of a file.
local function read_file (filepath)
  local fh = io.open(filepath, 'rb')
  local contents = fh:read('a')
  fh:close()
  return contents
end

--- Writes the contents into a file at the given path.
local function write_file (filepath, content)
  local fh = io.open(filepath, 'wb')
  fh:write(content)
  fh:close()
end

--
-- Diagram Engines
--

-- PlantUML engine; assumes that there's a `plantuml` binary.
local plantuml = {
  line_comment_start =  [[']],
  mime_types = mime_types_set{'pdf', 'png', 'svg'},
  compile = function (self, puml)
    local mime_type = self.mime_type or 'image/svg+xml'
    -- PlantUML format identifiers correspond to common file extensions.
    local format = extension_for_mimetype[mime_type]
    if not format then
      format, mime_type = 'svg', 'image/svg+xml'
    end
    local args = {'-t' .. format, "-pipe", "-charset", "UTF8"}
    return pandoc.pipe(self.execpath or 'plantuml', args, puml), mime_type
  end,
}

--- GraphViz engine for the dot language
local graphviz = {
  line_comment_start = '//',
  mime_types = mime_types_set{'jpg', 'pdf', 'png', 'svg'},
  mime_type = 'image/svg+xml',
  compile = function (self, code)
    local mime_type = self.mime_type
    -- GraphViz format identifiers correspond to common file extensions.
    local format = extension_for_mimetype[mime_type]
    if not format then
      format, mime_type = 'svg', 'image/svg+xml'
    end
    return pandoc.pipe(self.execpath or 'dot', {"-T"..format}, code), mime_type
  end,
}

--- Mermaid engine
local mermaid = {
  line_comment_start = '%%',
  mime_types = mime_types_set{'pdf', 'png', 'svg'},
  compile = function (self, code)
    local mime_type = self.mime_type or 'image/svg+xml'
    local file_extension = extension_for_mimetype[mime_type]
    return with_temporary_directory("diagram", function (tmpdir)
      return with_working_directory(tmpdir, function ()
        local infile = 'diagram.mmd'
        local outfile = 'diagram.' .. file_extension
        write_file(infile, code)
        pandoc.pipe(
          self.execpath or 'mmdc',
          {"--pdfFit", "--input", infile, "--output", outfile},
          ''
        )
        return read_file(outfile), mime_type
      end)
    end)
  end,
}

--- TikZ
--

--- LaTeX template used to compile TikZ images. Takes additional
--- packages as the first, and the actual TikZ code as the second
--- argument.
local tikz_template = pandoc.template.compile [[
\documentclass{standalone}
\usepackage{tikz}
$for(header-includes)$
$it$
$endfor$
$additional-packages$
\begin{document}
$body$
\end{document}
]]

--- The TikZ engine uses pdflatex to compile TikZ code to an image
local tikz = {
  line_comment_start = '%%',

  mime_types = {
    ['application/pdf'] = true,
  },

  --- Compile LaTeX with TikZ code to an image
  compile = function (self, src, user_opts)
    return with_temporary_directory("tikz", function (tmpdir)
      return with_working_directory(tmpdir, function ()
        -- Define file names:
        local file_template = "%s/tikz-image.%s"
        local tikz_file = file_template:format(tmpdir, "tex")
        local pdf_file = file_template:format(tmpdir, "pdf")

        -- Treat string values as raw LaTeX
        local meta = {
          ['header-includes'] = user_opts['header-includes'],
          ['additional-packages'] = {pandoc.RawInline(
            'latex',
            stringify(user_opts['additional-packages'] or '')
          )},
        }
        local tex_code = pandoc.write(
          pandoc.Pandoc({pandoc.RawBlock('latex', src)}, meta),
          'latex',
          {template = tikz_template}
        )
        write_file(tikz_file, tex_code)

        -- Execute the LaTeX compiler:
        pandoc.pipe(
          self.execpath or 'pdflatex',
          {'-output-directory', tmpdir, tikz_file},
          ''
        )

        return read_file(pdf_file), 'application/pdf'
      end)
    end)
  end
}

--- Asymptote diagram engine
local asymptote = {
  line_comment_start = '%%',
  mime_types = {
    ['application/pdf'] = true,
  },
  compile = function (self, code)
    return with_temporary_directory("asymptote", function(tmpdir)
      return with_working_directory(tmpdir, function ()
        local pdf_file = "pandoc_diagram.pdf"
        local args = {'-tex', 'pdflatex', "-o", "pandoc_diagram", '-'}
        pandoc.pipe(self.execpath or 'asy', args, code)
        return read_file(pdf_file), 'application/pdf'
      end)
    end)
  end,
}

local default_engines = {
  asymptote = asymptote,
  dot       = graphviz,
  mermaid   = mermaid,
  plantuml  = plantuml,
  tikz      = tikz,
}

--
-- Configuration
--

--- Options for the output format of the given name.
local function format_options (name)
  local pdf2svg = name ~= 'latex' and name ~= 'context'
  local preferred_mime_types = pandoc.List{'application/pdf', 'image/png'}
  -- Prefer SVG for non-PDF output formats
  if pdf2svg then
    preferred_mime_types:insert(1, 'image/svg+xml')
  end
  return {
    name = name,
    pdf2svg = pdf2svg,
    preferred_mime_types = preferred_mime_types,
    best_mime_type = function (self, supported_mime_types, requested)
      return self.preferred_mime_types:find_if(function (preferred)
          return supported_mime_types[preferred] and
            (not requested or
             (pandoc.utils.type(requested) == 'List' and
              requested:includes(preferred)) or
             (pandoc.utils.type(requested) == 'table' and
              requested[preferred]) or

             -- Assume string, Inlines, and Blocks values specify the only
             -- acceptable MIME type.
             stringify(requested) == preferred)
      end)
    end
  }
end

--- Returns a configured diagram engine.
local function get_engine (name, engopts, format)
  local engine = default_engines[name] or
    select(2, pcall(require, stringify(engopts.package)))

  -- Sanity check
  if not engine then
    warn(PANDOC_SCRIPT_FILE, ": No such engine '", name, "'.")
    return nil
  elseif engopts == false then
    -- engine is disabled
    return nil
  elseif engopts == true then
    -- use default options
    return engine
  end

  local execpath = engopts.execpath
    and stringify(engopts.execpath)
    or os.getenv(name:upper() .. '_BIN')

  local mime_type = format:best_mime_type(
    engine.mime_types,
    engopts['mime-type'] or engopts['mime-types']
  )
  if not mime_type then
    warn(PANDOC_SCRIPT_FILE, ": Cannot use ", name, " with ", format.name)
    return nil
  end

  return {
    execpath = execpath,
    compile = engine.compile,
    line_comment_start = engine.line_comment_start,
    mime_type = mime_type,
    opt = engopts or {},
  }
end

--- Returns the diagram engine configs.
local function configure (meta, format_name)
  local conf = meta.diagram or {}
  local format = format_options(format_name)
  meta.diagram = nil

  -- cache for image files
  if conf.cache then
    image_cache = conf['cache-dir']
      and stringify(conf['cache-dir'])
      or cachedir()
    pandoc.system.make_directory(image_cache, true)
  end

  -- engine configs
  local engine = {}
  for name, engopts in pairs(conf.engine or default_engines) do
    engine[name] = get_engine(name, engopts, format)
  end

  return {
    engine = engine,
    format = format,
    cache = image_cache and true,
    image_cache = image_cache,
  }
end


--
-- Format conversion
--

--- Converts a PDF to SVG.
local pdf2svg = function (imgdata)
  local pdf_file = os.tmpname() .. '.pdf'
  write_file(pdf_file, imgdata)
  local args = {
    '--export-type=svg',
    '--export-plain-svg',
    '--export-filename=-',
    pdf_file
  }
  return pandoc.pipe('inkscape', args, ''), os.remove(pdf_file)
end

local function properties_from_code (code, comment_start)
  local props = {}
  local pattern = comment_start:gsub('%p', '%%%1') .. '| ' ..
    '([-_%w]+): ([^\n]*)\n'
  for key, value in code:gmatch(pattern) do
    if key == 'fig-cap' then
      props['caption'] = value
    else
      props[key] = value
    end
  end
  return props
end

local function diagram_options (cb, comment_start)
  local attribs = comment_start
    and properties_from_code(cb.text, comment_start)
    or {}
  for key, value in pairs(cb.attributes) do
    attribs[key] = value
  end

  -- Read caption attribute as Markdown
  local caption = attribs.caption
    and pandoc.read(attribs.caption).blocks
    or nil
  local fig_attr = {
    id = cb.identifier ~= '' and cb.identifier or attribs.label,
    name = attribs.name,
  }
  local user_opt = {}

  for k, v in pairs(attribs) do
    local prefix, key = k:match '^(%a+)%-(%a[-%w]*)$'
    if prefix == 'fig' then
      fig_attr[key] = v
    elseif prefix == 'opt' then
      user_opt[key] = v
    end
  end

  return {
    ['alt'] = attribs.alt or
      (caption and pandoc.utils.blocks_to_inlines(caption)) or
      {},
    ['caption'] = caption,
    ['fig-attr'] = fig_attr,
    ['filename'] = attribs.filename,
    ['image-attr'] = {
      height = attribs.height,
      width = attribs.width,
      style = attribs.style,
    },
    ['opt'] = user_opt,
  }
end

local function get_cached_image (hash, mime_type)
  if not image_cache then
    return nil
  end
  local filename = hash .. '.' .. extension_for_mimetype[mime_type]
  local imgpath = pandoc.path.join{image_cache, filename}
  local success, imgdata = pcall(read_file, imgpath)
  if success then
    return imgdata, mime_type
  end
  return nil
end

local function cache_image (codeblock, imgdata, mimetype)
  -- do nothing if caching is disabled or not possible.
  if not image_cache then
    return
  end
  local ext = extension_for_mimetype[mimetype]
  local filename = pandoc.sha1(codeblock.text) .. '.' .. ext
  local imgpath = pandoc.path.join{image_cache, filename}
  write_file(imgpath, imgdata)
end

-- Executes each document's code block to find matching code blocks:
local function code_to_figure (conf)
  return function (block)
    -- Check if a converter exists for this block. If not, return the block
    -- unchanged.
    local diagram_type = block.classes[1]
    if not diagram_type then
      return nil
    end

    local engine = conf.engine[diagram_type]
    if not engine then
      return nil
    end

    -- Unified properties.
    local dgr_opt = diagram_options(block, engine.line_comment_start)
    for optname, value in pairs(engine.opt or {}) do
      dgr_opt.opt[optname] = dgr_opt.opt[optname] or value
    end

    local run_pdf2svg = engine.mime_type == 'application/pdf'
      and conf.format.pdf2svg

    -- Try to retrieve the image data from the cache.
    local imgdata, imgtype
    if conf.cache then
      imgdata, imgtype = get_cached_image(
        pandoc.sha1(block.text),
        run_pdf2svg and 'image/svg+xml' or engine.mime_type
      )
    end

    if not imgdata or not imgtype then
      -- No cached image; call the converter
      local success
      success, imgdata, imgtype =
        pcall(engine.compile, engine, block.text, dgr_opt.opt)

      -- Bail if an error occurred; imgdata contains the error message
      -- when that happens.
      if not success then
        warn(PANDOC_SCRIPT_FILE, ': ', tostring(imgdata))
        return nil
      elseif not imgdata then
        warn(PANDOC_SCRIPT_FILE, ': Diagram engine returned no image data.')
        return nil
      elseif not imgtype then
        warn(PANDOC_SCRIPT_FILE, ': Diagram engine did not return a MIME type.')
        return nil
      end

      -- Convert SVG if necessary.
      if imgtype == 'application/pdf' and conf.format.pdf2svg then
        imgdata, imgtype = pdf2svg(imgdata), 'image/svg+xml'
      end

      -- If we got here, then the transformation went ok and `img` contains
      -- the image data.
      cache_image(block, imgdata, imgtype)
    end

    -- Use the block's filename attribute or create a new name by hashing the
    -- image content.
    local basename, _extension = pandoc.path.split_extension(
      dgr_opt.filename or pandoc.sha1(imgdata)
    )
    local fname = basename .. '.' .. extension_for_mimetype[imgtype]

    -- Store the data in the media bag:
    pandoc.mediabag.insert(fname, imgtype, imgdata)

    -- Create the image object.
    local image = pandoc.Image(dgr_opt.alt, fname, "", dgr_opt['image-attr'])

    -- Create a figure if the diagram has a caption; otherwise return
    -- just the image.
    return dgr_opt.caption and
      pandoc.Figure(
        pandoc.Plain{image},
        dgr_opt.caption,
        dgr_opt['fig-attr']
      ) or
      pandoc.Plain{image}
  end
end

function Pandoc (doc)
  local conf = configure(doc.meta, FORMAT)
  return doc:walk {
    CodeBlock = code_to_figure(conf),
  }
end
