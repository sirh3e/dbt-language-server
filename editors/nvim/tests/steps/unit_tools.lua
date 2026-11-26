-- Pure helpers: nvim-cmp ordering and labels, the health check's diff of
-- commands, the selection-range arithmetic, the manager's CRUD.

return function(ctx)
  local H = ctx.H

  local completion = require("cogitator.completion")
  local function entry(name, sort_text, detail)
    return {
      source = { source = { client = { name = name } } },
      completion_item = { label = sort_text, sortText = sort_text, detail = detail },
    }
  end
  local a, b, other = entry("cogitator", "0001"), entry("cogitator", "0002"), entry("lua_ls", "0000")
  H.check(
    completion.compare(a, b) == true and completion.compare(b, a) == false,
    "completion: cogitator items follow the server's sortText"
  )
  H.check(completion.compare(a, other) == nil, "completion: other sources are left to the next comparator")
  local cmp_compare = { offset = 1, exact = 2, score = 3 }
  H.check(
    vim.deep_equal(completion.sorted(cmp_compare), { completion.compare, 1, 2, 3 }),
    "completion: sorted puts cogitator's comparator first"
  )
  local item = completion.format()(entry("cogitator", "1", "models/customers.sql\nmore"), { abbr = "x" })
  H.check(item.menu == "models/customers.sql", "completion: format shows the detail's first line", item.menu)
  local typed = entry("cogitator", "1", "Column of customers")
  typed.completion_item.labelDetails = { detail = " bigint", description = "customers" }
  local shown = completion.format()(typed, { abbr = "x" })
  H.check(shown.menu == "bigint · customers", "completion: format prefers labelDetails", shown.menu)
  local untouched = completion.format()(entry("lua_ls", "1", "detail"), { menu = "[LSP]" })
  H.check(untouched.menu == "[LSP]", "completion: format leaves other sources alone")

  local health = require("cogitator.health")
  local all = vim.tbl_keys(health.COMMANDS)
  H.check(#health.missing(all, function()
    return true
  end) == 0, "health: nothing missing when the server offers everything")
  local missing = health.missing(
    vim.tbl_filter(function(c)
      return c ~= "dbt.columnLineage"
    end, all),
    function(method)
      return method ~= "textDocument/selectionRange"
    end
  )
  H.check(
    #missing == 2
      and missing[1].name == "dbt.columnLineage"
      and missing[2].name == "textDocument/selectionRange",
    "health: names each missing command and method with its feature",
    vim.inspect(missing)
  )

  local select = require("cogitator.select")
  local function r(l1, c1, l2, c2)
    return { start = { line = l1, character = c1 }, ["end"] = { line = l2, character = c2 } }
  end
  local chain =
    { range = r(2, 4, 2, 8), parent = { range = r(2, 0, 2, 20), parent = { range = r(0, 0, 9, 0) } } }
  local ranges = select.flatten(chain)
  H.check(#ranges == 3, "select: flatten walks the parent chain")
  H.check(select.next_larger(ranges, nil) == ranges[1], "select: from nothing, the innermost range")
  H.check(select.next_larger(ranges, r(2, 4, 2, 8)) == ranges[2], "select: the next range strictly grows")
  H.check(select.next_larger(ranges, r(0, 0, 9, 0)) == nil, "select: nothing past the document")

  local syntax = require("cogitator.syntax")
  local function node(depth, l1, c1, l2, c2, extra)
    return vim.tbl_extend("force", {
      depth = depth,
      kind = "k" .. depth,
      field = vim.NIL,
      range = { start = { line = l1, character = c1 }, ["end"] = { line = l2, character = c2 } },
    }, extra or {})
  end
  local nodes = { node(0, 0, 0, 5, 0), node(1, 1, 0, 1, 10), node(2, 1, 3, 1, 6, { field = "name" }) }
  H.check(syntax.innermost(nodes, 1, 4) == 3, "syntax: the innermost node holds the cursor")
  H.check(syntax.innermost(nodes, 3, 0) == 1, "syntax: outside the inner nodes, the root")
  H.check(
    syntax.line(nodes[3]) == "    name: k2 [2:3 - 2:6]",
    "syntax: a tree line is indent, field, kind and 1-based range",
    syntax.line(nodes[3])
  )
  H.check(
    syntax.line(node(1, 0, 0, 0, 0, { missing = true, kind = ")" })) == "  MISSING ) [1:0 - 1:0]",
    "syntax: missing nodes say so"
  )

  local ts = require("cogitator.treesitter")
  local dir = vim.fs.joinpath(H.tmp, "grammar-src")
  vim.fn.mkdir(vim.fs.joinpath(dir, "src"), "p")
  vim.fn.writefile({}, vim.fs.joinpath(dir, "src", "scanner.c"))
  local cmd = ts.build_commands(dir, { "zig", "cc" }, "/out/dbt.so")[1]
  H.check(
    cmd[1] == "zig" and cmd[2] == "cc" and vim.tbl_contains(cmd, "-shared") and cmd[#cmd]:match("scanner%.c$"),
    "treesitter: builds parser.c and scanner.c as a shared object",
    vim.inspect(cmd)
  )
  local cargo = table.concat(vim.fn.readfile(vim.fs.joinpath(ctx.repo_root, "Cargo.toml")), "\n")
  local pinned = cargo:match('tree%-sitter%-dbt%s*=%s*{[^}]-rev%s*=%s*"(%x+)"')
  H.check(
    pinned == ts.REV,
    "treesitter: the grammar revision is the one Cargo.toml pins for the server",
    vim.inspect({ cargo = pinned, plugin = ts.REV })
  )
  local config = require("cogitator.config")
  H.check(
    config.build({ grammar = { rev = "abc", url = "/x" } }).grammar.rev == "abc",
    "config: grammar.url/rev are accepted though their default is nil"
  )
  H.check(not pcall(config.build, { grammar = { typo = 1 } }), "config: an unknown grammar key is an error")

  local manager = require("cogitator.manager")
  local root = vim.fs.joinpath(H.tmp, "proj-crud")
  manager.remember(root, {})
  manager.rename(root, "Warehouse")
  H.check(
    manager.describe(root):find("^Warehouse — ") ~= nil,
    "manager: rename labels the project",
    manager.describe(root)
  )
  H.check(
    manager.describe(root):find("(missing)", 1, true) ~= nil,
    "manager: a root without dbt_project.yml says so"
  )
  H.check(
    manager.forget(root) and not vim.tbl_contains(manager.roots(), root),
    "manager: forget drops the project"
  )
  H.check(not manager.forget(root), "manager: forgetting twice is a no-op")
  local labels = vim.tbl_map(function(a2)
    return a2.label
  end, manager.project_actions(root))
  H.check(
    vim.deep_equal(
      labels,
      { "open", "set profiles directory", "clear profile/target selections", "rename", "forget" }
    ),
    "manager: the project menu's actions",
    vim.inspect(labels)
  )
end
