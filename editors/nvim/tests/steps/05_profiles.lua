-- Profiles, status and the DAG — the dir-switch mutates session state,
-- so this step runs LAST. Every former hand-rolled retry loop is a
-- single labeled wait with a deadline.

return function(ctx)
  local H, bufnr = ctx.H, ctx.bufnr

  local profiles = require("cogitator.profiles")
  local cands = profiles.candidates({ profiles_dir = "/override" }, "/work")
  H.check(
    cands[1] == vim.fs.joinpath("/override", "profiles.yml")
      and vim.tbl_contains(cands, vim.fs.joinpath("/work", ".dbt", "profiles.yml")),
    "profiles: candidates start with the override",
    vim.inspect(cands)
  )

  -- switch to a genuinely different directory (inside the sandbox)
  local other_dir = vim.fs.joinpath(H.tmp, "profiles-switch")
  vim.fn.mkdir(other_dir, "p")
  vim.fn.writefile(
    { "jaffle_shop:", "  target: dev", "  outputs:", "    dev:", "      type: snowflake" },
    vim.fs.joinpath(other_dir, "profiles.yml")
  )
  profiles.set(other_dir)

  -- live switch: the server advertises dbt.setProfilesDir, so the SAME
  -- client stays up — dbt.status converges on the new dirOverride
  local client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
  ctx.client = client
  local last_status
  H.wait("profiles: :DbtProfiles live-switches via dbt.setProfilesDir", 30000, function()
    local status = H.exec_cmd(client, bufnr, "dbt.status", {
      uri = vim.uri_from_bufnr(bufnr),
    }, "profiles: dbt.status during switch")
    last_status = status
    return type(status) == "table" and status.profiles and status.profiles.dirOverride == other_dir
  end, function()
    return "wanted " .. other_dir .. " got " .. vim.inspect(last_status):sub(1, 300)
  end)
  print("ok   profiles: :DbtProfiles live-switches via dbt.setProfilesDir (no restart)")

  -- and the selection was persisted for this project root
  local persisted = H.wait("manager: live switch persists the per-root profiles dir", 10000, function()
    return require("cogitator.manager").for_root(client.root_dir).profiles_dir == other_dir
  end)
  if persisted then
    print("ok   manager: live switch persists the per-root profiles dir")
  end

  -- the auth manager over the wire: the new dir holds a snowflake
  -- jaffle_shop, reported with auth issues once the refresh landed
  local report
  H.wait("profiles: dbt.profiles reports the snowflake profile", 30000, function()
    local pending = H.exec_cmd(client, bufnr, "dbt.profiles", {
      uri = vim.uri_from_bufnr(bufnr),
    }, "profiles: dbt.profiles answers")
    if type(pending) == "table" and pending.activeProfile == "jaffle_shop" then
      report = pending
      return true
    end
    return false
  end)
  local target0 = type(report) == "table"
    and report.profiles
    and report.profiles[1]
    and report.profiles[1].targets
    and report.profiles[1].targets[1]
  H.check(
    type(report) == "table"
      and report.activeProfile == "jaffle_shop"
      and target0
      and target0.type == "snowflake"
      and #target0.auth.issues > 0,
    "profiles: dbt.profiles reports targets with auth issues",
    vim.inspect(report):sub(1, 300)
  )

  local set_result = H.exec_cmd(client, bufnr, "dbt.setTarget", { target = "dev" }, "profiles: dbt.setTarget")
  H.check(
    type(set_result) == "table" and set_result.target == "dev",
    "profiles: dbt.setTarget switches the active target",
    vim.inspect(set_result)
  )

  -- tests navigation: dbt.modelTests wire shape
  local tests_result = H.exec_cmd(client, bufnr, "dbt.modelTests", {
    uri = vim.uri_from_bufnr(bufnr),
  }, "yaml: dbt.modelTests")
  local has_unique = type(tests_result) == "table"
    and tests_result.model == "customers"
    and vim.iter(tests_result.tests or {}):any(function(t)
      return t.name == "unique" and t.column == "customer_id"
    end)
  H.check(has_unique, "yaml: dbt.modelTests lists unique on customer_id", vim.inspect(tests_result):sub(1, 200))

  -- extract-to-version: applyEdit lands in the schema BUFFER (never saved)
  client:exec_cmd({
    command = "dbt.extractModelYaml",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr })
  local schema_buf
  H.wait("yaml: extract applies versions: to the schema buffer", 15000, function()
    for _, b in ipairs(vim.api.nvim_list_bufs()) do
      if vim.api.nvim_buf_is_loaded(b) and vim.api.nvim_buf_get_name(b):find("models/schema%.yml$") then
        local text = table.concat(vim.api.nvim_buf_get_lines(b, 0, -1, false), "\n")
        if text:find("versions:") then
          schema_buf = b
          return true
        end
      end
    end
    return false
  end)
  H.check(schema_buf ~= nil, "yaml: extract applies versions: to the schema buffer")
  if schema_buf then
    local text = table.concat(vim.api.nvim_buf_get_lines(schema_buf, 0, -1, false), "\n")
    H.check(text:find("latest_version: 1") ~= nil, "yaml: latest_version added", text:sub(1, 200))
    H.check(vim.bo[schema_buf].modified, "yaml: schema buffer modified, not written")
    vim.api.nvim_buf_delete(schema_buf, { force = true })
  end

  -- server status over the wire
  local status_result = H.exec_cmd(client, bufnr, "dbt.status", {
    uri = vim.uri_from_bufnr(bufnr),
  }, "status: dbt.status")
  H.check(
    type(status_result) == "table"
      and status_result.server.backend == "tree-sitter"
      and status_result.server.progressSupported == true
      and status_result.project.models > 0
      and status_result.refresh.count >= 1,
    "status: dbt.status reports backend, progress support and project",
    vim.inspect(status_result):sub(1, 300)
  )
  H.check(
    type(status_result) == "table" and type(status_result.config) == "table" and status_result.config.loaded == false,
    "status: dbt.status carries the config section (no cogitator.yml here)",
    type(status_result) == "table" and vim.inspect(status_result.config) or status_result
  )
  H.check(
    type(status_result) == "table"
      and type(status_result.caches) == "table"
      and type(status_result.caches.artifacts) == "table",
    "status: the caches section reports the artifacts surface",
    type(status_result) == "table" and vim.inspect(status_result.caches) or status_result
  )

  -- the project DAG (Neovim-only): wire shape, boxes, edges, columns
  local dag_result = H.exec_cmd(client, bufnr, "dbt.projectGraph", {
    uri = vim.uri_from_bufnr(bufnr),
  }, "dag: dbt.projectGraph")
  H.check(
    type(dag_result) == "table" and #dag_result.nodes > 5 and #dag_result.edges > 0,
    "dag: dbt.projectGraph lists nodes and edges",
    type(dag_result) == "table" and #dag_result.nodes or dag_result
  )

  local dag = require("cogitator.dag")
  dag.open()
  H.wait("dag: :DbtDag renders boxes", 15000, function()
    return dag._state ~= nil and dag._state.boxes ~= nil and next(dag._state.boxes) ~= nil
  end)
  H.check(dag._state ~= nil, "dag: :DbtDag opened")
  if dag._state then
    local text = table.concat(vim.api.nvim_buf_get_lines(dag._state.buf, 0, -1, false), "\n")
    H.check(text:find("╭") ~= nil and text:find("▸") ~= nil, "dag: boxes and edge arrows drawn")
    H.check(
      text:find("stg_customers", 1, true) ~= nil and text:find("customers", 1, true) ~= nil,
      "dag: model names present"
    )
    H.check(text:find("#nightly", 1, true) ~= nil, "dag: tags shown on tagged nodes", text:sub(1, 200))
    -- C toggles columns everywhere (docglow-style expansion)
    dag._state.all_expanded = true
    vim.api.nvim_feedkeys("C", "x", false) -- flips it back off…
    vim.api.nvim_feedkeys("C", "x", false) -- …and on again, exercising the mapping
    local expanded = table.concat(vim.api.nvim_buf_get_lines(dag._state.buf, 0, -1, false), "\n")
    H.check(
      expanded:find("• customer_id", 1, true) ~= nil,
      "dag: expanded nodes list columns",
      expanded:sub(1, 300)
    )
    dag.close()
    H.check(dag._state == nil, "dag: closes cleanly")
  end
end
