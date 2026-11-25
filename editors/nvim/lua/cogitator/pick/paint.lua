--- Colour in the pickers' lines. An item carries `hls`, byte spans of its
--- `text` with a highlight group each, which every backend draws: the
--- built-in picker as extmarks, telescope as its display highlights,
--- snacks as segments, fzf-lua as ANSI. A line is built from parts, so
--- its text and its spans cannot drift apart. Pure but for nothing.

local M = {}

--- The kind of a node or symbol → the group its name is drawn in: the
--- dbt-docs colours of the DAG (as text), and colours of their own for
--- what dbt-docs leaves uncoloured (cogitator.highlight defines them).
M.KIND_GROUP = {
  model = "DbtDagModelFg",
  python = "DbtDagPythonFg",
  source = "DbtDagSourceFg",
  exposure = "DbtDagExposureFg",
  export = "DbtDagExposureFg",
  metric = "DbtDagMetricFg",
  semantic_model = "DbtDagSemanticModelFg",
  semantic = "DbtDagSemanticModelFg",
  test = "DbtKindTestFg",
  generic_test = "DbtKindTestFg",
  macro = "DbtKindMacroFg",
  seed = "DbtKindSeedFg",
  snapshot = "DbtKindSnapshotFg",
  selector = "DbtKindSelectorFg",
  analysis = "DbtKindAnalysisFg",
}

--- The group of `kind` (a Python model by its file), or nil.
---@param kind string?
---@param uri any?
---@return string?
function M.kind_group(kind, uri)
  if kind == "model" and type(uri) == "string" and uri:sub(-3) == ".py" then
    return M.KIND_GROUP.python
  end
  return kind and M.KIND_GROUP[kind] or nil
end

---@alias cogitator.pick.Span {[1]: integer, [2]: integer, [3]: string} 0-based start, end (exclusive), group

--- A line of parts, `{text, group?}` each: its text and its spans.
---@param parts {[1]: string, [2]: string?}[]
---@return string text
---@return cogitator.pick.Span[] hls
function M.line(parts)
  local text, hls = "", {}
  for _, part in ipairs(parts) do
    local piece = part[1] or ""
    if part[2] and piece ~= "" then
      hls[#hls + 1] = { #text, #text + #piece, part[2] }
    end
    text = text .. piece
  end
  return text, hls
end

--- `item` with its text and spans set from `parts`.
---@param item table
---@param parts {[1]: string, [2]: string?}[]
---@return table item
function M.paint(item, parts)
  item.text, item.hls = M.line(parts)
  return item
end

--- The parts of a node's line: its label in its kind's colour, the kind
--- in brackets, its tags.
---@param label string
---@param kind string
---@param tags string[]?
---@param uri any?
---@return {[1]: string, [2]: string?}[]
function M.node_parts(label, kind, tags, uri, kind_label)
  local group = M.kind_group(kind, uri)
  local parts = {
    { label, group },
    { "  " },
    { "[" .. (kind_label or kind) .. "]", group },
  }
  if tags and #tags > 0 then
    parts[#parts + 1] = { "  " }
    parts[#parts + 1] = { "#" .. table.concat(tags, " #"), "DbtPickTag" }
  end
  return parts
end

--- An item's text cut into `{text, group?}` segments at its spans (for
--- the backends that draw segments). Overlapping spans: the later wins.
---@param text string
---@param hls cogitator.pick.Span[]?
---@return {[1]: string, [2]: string?}[]
function M.segments(text, hls)
  if not hls or #hls == 0 then
    return { { text } }
  end
  local group_at = {}
  for _, span in ipairs(hls) do
    for i = span[1] + 1, math.min(span[2], #text) do
      group_at[i] = span[3]
    end
  end
  local out, start = {}, 1
  for i = 2, #text + 1 do
    if i > #text or group_at[i] ~= group_at[start] then
      out[#out + 1] = { text:sub(start, i - 1), group_at[start] }
      start = i
    end
  end
  return out
end

return M
