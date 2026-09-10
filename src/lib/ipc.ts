import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AccountFilter,
  AccountWithCategories,
  AutoCategorizeArgs,
  AutoCategorizeResult,
  BiliFetchResult,
  BiliQrPoll,
  BiliQrStart,
  BiliStatus,
  Category,
  ImportResult,
  NormalizedAccount,
  OverviewCounts,
  ParseResult,
  UnfollowProgress,
} from "@/types";

/**
 * Tauri IPC 客户端封装。
 * 浏览器环境（vite dev 无 Tauri 时）给出明确错误提示，而不是静默失败。
 */
function ensureTauri(): void {
  const tauri = (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  if (!tauri) {
    throw new Error(
      "当前不在 Tauri 桌面环境中运行。请使用 `pnpm tauri dev` 启动应用（而非仅 `pnpm dev`）。",
    );
  }
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  ensureTauri();
  return invoke<T>(cmd, args);
}

export const api = {
  // ---- 账号 ----
  queryAccounts: (filter: AccountFilter) =>
    call<AccountWithCategories[]>("query_accounts", { filter }),
  overviewCounts: () => call<OverviewCounts>("overview_counts"),
  assignCategories: (accountIds: string[], categoryIds: string[]) =>
    call<number>("assign_categories", { accountIds, categoryIds }),
  removeCategoryFromAccounts: (accountIds: string[], categoryId: string) =>
    call<void>("remove_category_from_accounts", { accountIds, categoryId }),
  updateNote: (id: string, note: string | null) =>
    call<void>("update_note", { id, note }),

  // ---- 分类 ----
  listCategories: () => call<Category[]>("list_categories"),
  categoryCounts: () => call<Record<string, number>>("category_counts"),
  createCategory: (name: string, parentId: string | null) =>
    call<Category>("create_category", { name, parentId }),
  renameCategory: (id: string, name: string) =>
    call<void>("rename_category", { id, name }),
  deleteCategory: (id: string) => call<void>("delete_category", { id }),
  moveCategory: (id: string, newParentId: string | null, position: number) =>
    call<void>("move_category", { id, newParentId, position }),

  // ---- 导入/导出 ----
  parseImportFile: (path: string) => call<ParseResult>("parse_import_file", { path }),
  commitImport: (accounts: NormalizedAccount[]) =>
    call<ImportResult>("commit_import", { accounts }),
  writeTemplate: (path: string, format: string) =>
    call<void>("write_template", { path, format }),
  exportAccounts: (path: string, format: string, accountIds: string[] | null) =>
    call<number>("export_accounts", { path, format, accountIds }),

  // ---- B站适配器 ----
  biliQrGenerate: () => call<BiliQrStart>("bilibili_qr_generate"),
  biliQrPoll: (qrcodeKey: string) =>
    call<BiliQrPoll>("bilibili_qr_poll", { qrcodeKey }),
  biliStatus: () => call<BiliStatus>("bilibili_status"),
  biliLogout: () => call<void>("bilibili_logout"),
  biliFetchFollows: () => call<BiliFetchResult>("bilibili_fetch_follows"),
  biliUnfollowBatch: (mids: string[]) =>
    call<void>("bilibili_unfollow_batch", { mids }),

  /** 订阅取关进度事件；返回取消监听函数 */
  onUnfollowProgress: (handler: (p: UnfollowProgress) => void) =>
    listen<UnfollowProgress>("unfollow-progress", (e) => handler(e.payload)),

  // ---- 自动分类 ----
  autoCategorize: (args: AutoCategorizeArgs) =>
    call<AutoCategorizeResult>("auto_categorize", { args }),
};
