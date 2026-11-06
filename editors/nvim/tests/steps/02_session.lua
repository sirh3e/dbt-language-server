-- Session bring-up: attach inside the fixture COPY, the dbt.status
-- refresh barrier (the sanctioned readiness signal), then the basic
-- wire shapes — hover, lenses, goToSchema, rename.

return function(ctx)
  local H = ctx.H

  local model = vim.fs.joinpath(ctx.fixture, "models", "customers.sql")
  vim.cmd.edit(model)
  local bufnr = vim.api.nvim_get_current_buf()

  local attached = H.wait("integration: client attaches inside the dbt project", 15000, function()
    return #vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" }) == 1
  end)
  if not attached then
    H.finish() -- nothing else can run without a client
  end
  print("ok   integration: client attaches inside the dbt project")
  local client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
  ctx.client, ctx.bufnr = client, bufnr

  -- THE readiness barrier: refresh.count >= 1 means the first project
  -- scan completed — not a lens/hover proxy (spec §8)
  if not H.wait_refresh(client, bufnr, 1, 30000) then
    H.finish()
  end
  print("ok   integration: first context refresh completed (dbt.status barrier)")

  -- position inside the first ref('…') argument
  local text = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
  local ref_name = text:match("ref%('([%w_]+)'%)")
  H.check(ref_name ~= nil, "integration: fixture model contains a ref()")
  local before = text:sub(1, (text:find("ref%('" .. ref_name .. "'%)")) - 1)
  local _, newlines = before:gsub("\n", "")
  local line_start = before:match("[^\n]*$")
  ctx.doc = {
    textDocument = { uri = vim.uri_from_bufnr(bufnr) },
    position = { line = newlines, character = #line_start + 5 },
  }

  -- after the barrier these are single requests, no retries
  local lenses = H.request(client, bufnr, "textDocument/codeLens", {
    textDocument = { uri = vim.uri_from_bufnr(bufnr) },
  }, "integration: code lenses after the barrier") or {}
  H.check(#lenses > 0, "integration: context refresh produced code lenses")
  local titles = vim.tbl_map(function(l)
    return l.command.title
  end, lenses)
  local function has_title(pattern)
    return vim.iter(titles):any(function(t)
      return t:find(pattern) ~= nil
    end)
  end
  H.check(has_title("downstream") and has_title("upstream"), "integration: downstream/upstream lenses", vim.inspect(titles))
  H.check(has_title("dbt run"), "integration: dbt run lens present", vim.inspect(titles))

  local hover = H.request(client, bufnr, "textDocument/hover", ctx.doc, "integration: hover answers")
  H.check(
    hover ~= nil and hover ~= vim.NIL and type(hover.contents) == "string",
    "integration: hover over ref('" .. tostring(ref_name) .. "') answers",
    vim.inspect(hover)
  )

  local schema_result = H.exec_cmd(client, bufnr, "dbt.goToSchema", {
    uri = vim.uri_from_bufnr(bufnr),
    position = { line = 0, character = 0 },
  }, "integration: dbt.goToSchema")
  H.check(
    type(schema_result) == "table" and type(schema_result.uri) == "string" and schema_result.uri:find("%.yml$"),
    "integration: dbt.goToSchema returns a schema-YAML Location",
    vim.inspect(schema_result)
  )

  local edit = H.request(
    client,
    bufnr,
    "textDocument/rename",
    vim.tbl_extend("force", ctx.doc, { newName = "renamed_model" }),
    "integration: rename answers"
  )
  local changes = edit and edit.documentChanges or {}
  local last = changes[#changes]
  H.check(
    last ~= nil and last.kind == "rename" and last.newUri:find("renamed_model"),
    "integration: rename ends in a RenameFile documentChange",
    vim.inspect(last)
  )
end
