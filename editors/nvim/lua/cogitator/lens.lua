--- Code-lens refresh wiring (spec §5.2): refresh once on attach and on the
--- configured events; clean up when the client detaches.

local M = {}

function M.attach(bufnr, config)
  if not config.lens.enabled then
    return
  end
  if vim.lsp.codelens.enable then
    -- 0.12+: managed refresh; no autocmds of our own needed
    vim.lsp.codelens.enable(true, { bufnr = bufnr })
    return
  end
  local group = vim.api.nvim_create_augroup("CogitatorLens" .. bufnr, { clear = true })
  vim.api.nvim_create_autocmd(config.lens.events, {
    group = group,
    buffer = bufnr,
    callback = function()
      vim.lsp.codelens.refresh({ bufnr = bufnr })
    end,
  })
  vim.api.nvim_create_autocmd("LspDetach", {
    group = group,
    buffer = bufnr,
    callback = function(args)
      if vim.lsp.get_client_by_id(args.data.client_id).name == "cogitator" then
        vim.api.nvim_del_augroup_by_id(group)
        vim.lsp.codelens.clear(args.data.client_id, bufnr)
      end
    end,
  })
  vim.lsp.codelens.refresh({ bufnr = bufnr })
end

return M
