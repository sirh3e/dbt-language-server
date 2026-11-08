--- Graph colours, taken from dbt-docs (src/app/services/graph.service.js).
--- Groups are defined with default=true so colorschemes can override them.

local M = {}

local WHITE, WHITE_CTERM = "#ffffff", 15

--- Filled node groups: name → {background, 256-colour background}.
local FILLS = {
  DbtDagModel = { "#0094b3", 31 },
  DbtDagSource = { "#5fb825", 70 },
  DbtDagExposure = { "#ff694b", 203 },
  DbtDagMetric = { "#ff5688", 204 },
  DbtDagSemanticModel = { "#ffa8c2", 218 },
  DbtDagPython = { "#6a5acd", 62 },
  DbtDagSelected = { "#88447d", 96 },
}

--- Line groups: name → {foreground, 256-colour foreground}.
local LINES = {
  DbtDagEdge = { "#006f8a", 24 },
  DbtDagSelectedEdge = { "#bd6bb6", 133 },
  DbtDagDim = { "#919599", 246 },
}

local KIND_GROUPS = {
  source = "DbtDagSource",
  exposure = "DbtDagExposure",
  metric = "DbtDagMetric",
  semantic_model = "DbtDagSemanticModel",
}

local function define()
  local set = vim.api.nvim_set_hl
  for name, c in pairs(FILLS) do
    set(0, name, { bg = c[1], fg = WHITE, ctermbg = c[2], ctermfg = WHITE_CTERM, default = true })
    -- Same colour as text, for list panes where a fill would be too loud.
    set(0, name .. "Fg", { fg = c[1], ctermfg = c[2], default = true })
  end
  for name, c in pairs(LINES) do
    set(0, name, { fg = c[1], ctermfg = c[2], default = true })
  end
end

local installed = false

--- Define the groups; idempotent. :colorscheme clears them, so also
--- re-define them whenever it runs.
function M.setup()
  define()
  if not installed then
    installed = true
    vim.api.nvim_create_autocmd("ColorScheme", {
      group = vim.api.nvim_create_augroup("CogitatorHighlight", { clear = true }),
      callback = define,
    })
  end
end

--- The filled group for a graph node or entry. dbt-docs has no colour of
--- its own for tests and analyses, so they get the default node colour.
---@param node {kind: string, uri?: string|vim.NIL}
---@return string
function M.group_for(node)
  local group = KIND_GROUPS[node.kind]
  if group then
    return group
  end
  if type(node.uri) == "string" and node.uri:sub(-3) == ".py" then
    return "DbtDagPython"
  end
  return "DbtDagModel"
end

return M
